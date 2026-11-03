//! GPU statistics on Windows without vendor SDKs or admin rights — the same
//! sources Task Manager uses:
//! * DXGI: adapter name and dedicated VRAM size (picks the discrete GPU).
//! * D3DKMT adapter perf data: temperature and fan speed (WDDM 2.9+ drivers).
//! * PDH "GPU Engine" / "GPU Adapter Memory" counters: utilisation and VRAM in use.

use serde_json::{json, Value};
use std::ffi::c_void;
use windows::core::{w, PCWSTR};
use windows::Wdk::Graphics::Direct3D::{
    D3DKMTCloseAdapter, D3DKMTOpenAdapterFromLuid, D3DKMTQueryAdapterInfo, D3DKMT_ADAPTER_PERFDATA, D3DKMT_CLOSEADAPTER,
    D3DKMT_OPENADAPTERFROMLUID, D3DKMT_QUERYADAPTERINFO, KMTQAITYPE_ADAPTERPERFDATA,
};
use windows::Win32::Foundation::LUID;
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W,
    PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};

const PDH_MORE_DATA: u32 = 0x8000_07D2;

struct Adapter {
    name: String,
    luid: LUID,
    vram_total: u64,
    kmt: Option<u32>,
}

pub struct GpuProbe {
    adapter: Option<Adapter>,
    query: Option<(PDH_HQUERY, PDH_HCOUNTER, PDH_HCOUNTER)>,
}

// PDH/D3DKMT handles are plain values used from a single thread.
unsafe impl Send for GpuProbe {}

impl Drop for GpuProbe {
    fn drop(&mut self) {
        unsafe {
            if let Some((q, _, _)) = self.query.take() {
                let _ = PdhCloseQuery(q);
            }
            if let Some(h) = self.adapter.as_ref().and_then(|a| a.kmt) {
                let _ = D3DKMTCloseAdapter(&D3DKMT_CLOSEADAPTER { hAdapter: h });
            }
        }
    }
}

fn find_adapter() -> Option<Adapter> {
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut best: Option<Adapter> = None;
        let mut i = 0;
        while let Ok(a) = factory.EnumAdapters1(i) {
            i += 1;
            let Ok(d) = a.GetDesc1() else { continue };
            if d.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                continue;
            }
            let len = d.Description.iter().position(|c| *c == 0).unwrap_or(d.Description.len());
            let name = String::from_utf16_lossy(&d.Description[..len]);
            let vram = d.DedicatedVideoMemory as u64;
            if best.as_ref().is_none_or(|b| vram > b.vram_total) {
                best = Some(Adapter { name, luid: d.AdapterLuid, vram_total: vram, kmt: None });
            }
        }
        let mut adapter = best?;
        let mut open = D3DKMT_OPENADAPTERFROMLUID { AdapterLuid: adapter.luid, hAdapter: 0 };
        if D3DKMTOpenAdapterFromLuid(&mut open).is_ok() {
            adapter.kmt = Some(open.hAdapter);
        }
        Some(adapter)
    }
}

fn open_query() -> Option<(PDH_HQUERY, PDH_HCOUNTER, PDH_HCOUNTER)> {
    unsafe {
        let mut q = PDH_HQUERY::default();
        if PdhOpenQueryW(PCWSTR::null(), 0, &mut q) != 0 {
            return None;
        }
        let mut engine = PDH_HCOUNTER::default();
        let mut memory = PDH_HCOUNTER::default();
        if PdhAddEnglishCounterW(q, w!("\\GPU Engine(*)\\Utilization Percentage"), 0, &mut engine) != 0
            || PdhAddEnglishCounterW(q, w!("\\GPU Adapter Memory(*)\\Dedicated Usage"), 0, &mut memory) != 0
        {
            let _ = PdhCloseQuery(q);
            return None;
        }
        let _ = PdhCollectQueryData(q); // prime rate counters
        Some((q, engine, memory))
    }
}

/// (instance name, value) pairs of a wildcard counter.
unsafe fn read_array(counter: PDH_HCOUNTER) -> Vec<(String, f64)> {
    let mut size = 0u32;
    let mut count = 0u32;
    if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None) != PDH_MORE_DATA || size == 0 {
        return Vec::new();
    }
    // Buffer of items followed by their name strings; u64 alignment for the f64 values.
    let mut buf = vec![0u64; (size as usize).div_ceil(8) + 1];
    let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
    if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items)) != 0 {
        return Vec::new();
    }
    (0..count as usize)
        .map(|i| {
            let item = &*items.add(i);
            let name = item.szName.to_string().unwrap_or_default();
            (name, item.FmtValue.Anonymous.doubleValue)
        })
        .collect()
}

impl GpuProbe {
    pub fn new() -> Self {
        Self { adapter: find_adapter(), query: open_query() }
    }

    pub fn sample(&mut self) -> Option<Value> {
        let adapter = self.adapter.as_ref()?;
        let luid_tag = format!("luid_0x{:08x}_0x{:08x}", adapter.luid.HighPart as u32, adapter.luid.LowPart).to_ascii_lowercase();
        let mut usage: Option<f64> = None;
        let mut vram_used: Option<u64> = None;
        if let Some((q, engine, memory)) = self.query {
            unsafe {
                if PdhCollectQueryData(q) == 0 {
                    // Task Manager style: busiest engine type (3D, Copy, VideoDecode, ...).
                    let mut by_type: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
                    for (name, v) in read_array(engine) {
                        let lower = name.to_ascii_lowercase();
                        if !lower.contains(&luid_tag) {
                            continue;
                        }
                        let kind = lower.rsplit("engtype_").next().unwrap_or("other").to_string();
                        *by_type.entry(kind).or_insert(0.0) += v;
                    }
                    usage = by_type.values().cloned().fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
                    let used: f64 = read_array(memory)
                        .into_iter()
                        .filter(|(n, _)| n.to_ascii_lowercase().contains(&luid_tag))
                        .map(|(_, v)| v)
                        .sum();
                    if used > 0.0 {
                        vram_used = Some(used as u64);
                    }
                }
            }
        }

        let (mut temp, mut fan) = (None, None);
        if let Some(h) = adapter.kmt {
            unsafe {
                let mut perf = D3DKMT_ADAPTER_PERFDATA { PhysicalAdapterIndex: 0, ..Default::default() };
                let mut q = D3DKMT_QUERYADAPTERINFO {
                    hAdapter: h,
                    Type: KMTQAITYPE_ADAPTERPERFDATA,
                    pPrivateDriverData: &mut perf as *mut _ as *mut c_void,
                    PrivateDriverDataSize: std::mem::size_of::<D3DKMT_ADAPTER_PERFDATA>() as u32,
                };
                if D3DKMTQueryAdapterInfo(&mut q).is_ok() {
                    if perf.Temperature > 0 && perf.Temperature < 1500 {
                        temp = Some(perf.Temperature as f64 / 10.0); // deci-Celsius
                    }
                    if perf.FanRPM > 0 {
                        fan = Some(perf.FanRPM);
                    }
                }
            }
        }

        Some(json!({
            "name": adapter.name,
            "usage": usage.map(|u| (u.min(100.0) * 10.0).round() / 10.0),
            "vramUsed": vram_used,
            "vramTotal": adapter.vram_total,
            "temp": temp,
            "fan": fan,
        }))
    }
}
