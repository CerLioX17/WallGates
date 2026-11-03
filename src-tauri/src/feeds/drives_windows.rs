//! Physical drives on Windows without admin rights:
//! * model name and temperature via IOCTL_STORAGE_QUERY_PROPERTY on
//!   `\\.\PhysicalDriveN` opened with no access rights (the same source the
//!   Windows 11 Settings "Disks & volumes" page uses for NVMe temperature);
//! * activity (% active time) and read/write throughput via PDH PhysicalDisk counters.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::ffi::c_void;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceProperty, StorageDeviceTemperatureProperty, IOCTL_STORAGE_QUERY_PROPERTY,
    STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY, STORAGE_TEMPERATURE_DATA_DESCRIPTOR,
};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};
use windows::Win32::System::IO::DeviceIoControl;

const PDH_MORE_DATA: u32 = 0x8000_07D2;

struct Drive {
    index: u32,
    model: String,
}

pub struct DriveProbe {
    drives: Vec<Drive>,
    query: Option<(PDH_HQUERY, [PDH_HCOUNTER; 3])>,
}

unsafe impl Send for DriveProbe {}

impl Drop for DriveProbe {
    fn drop(&mut self) {
        if let Some((q, _)) = self.query.take() {
            unsafe {
                let _ = PdhCloseQuery(q);
            }
        }
    }
}

unsafe fn open_drive(index: u32) -> Option<HANDLE> {
    let path = HSTRING::from(format!("\\\\.\\PhysicalDrive{index}"));
    // Access 0: enough for property queries, no admin needed.
    CreateFileW(&path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE, None, OPEN_EXISTING, FILE_FLAGS_AND_ATTRIBUTES(0), None).ok()
}

unsafe fn query(handle: HANDLE, id: windows::Win32::System::Ioctl::STORAGE_PROPERTY_ID, buf: &mut [u8]) -> bool {
    let q = STORAGE_PROPERTY_QUERY { PropertyId: id, QueryType: PropertyStandardQuery, AdditionalParameters: [0] };
    let mut returned = 0u32;
    DeviceIoControl(
        handle,
        IOCTL_STORAGE_QUERY_PROPERTY,
        Some(&q as *const _ as *const c_void),
        std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
        Some(buf.as_mut_ptr() as *mut c_void),
        buf.len() as u32,
        Some(&mut returned),
        None,
    )
    .is_ok()
        && returned > 0
}

unsafe fn model_of(handle: HANDLE) -> Option<String> {
    let mut buf = vec![0u8; 1024];
    if !query(handle, StorageDeviceProperty, &mut buf) {
        return None;
    }
    let d = &*(buf.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR);
    let off = d.ProductIdOffset as usize;
    if off == 0 || off >= buf.len() {
        return None;
    }
    let end = buf[off..].iter().position(|b| *b == 0).map(|p| off + p).unwrap_or(buf.len());
    let s = String::from_utf8_lossy(&buf[off..end]).trim().to_string();
    (!s.is_empty()).then_some(s)
}

unsafe fn temperature_of(handle: HANDLE) -> Option<f64> {
    let mut buf = vec![0u8; 512];
    if !query(handle, StorageDeviceTemperatureProperty, &mut buf) {
        return None;
    }
    let d = &*(buf.as_ptr() as *const STORAGE_TEMPERATURE_DATA_DESCRIPTOR);
    if d.InfoCount == 0 {
        return None;
    }
    let t = d.TemperatureInfo[0].Temperature as f64;
    (t > -40.0 && t < 150.0).then_some(t)
}

fn open_query() -> Option<(PDH_HQUERY, [PDH_HCOUNTER; 3])> {
    unsafe {
        let mut q = PDH_HQUERY::default();
        if PdhOpenQueryW(PCWSTR::null(), 0, &mut q) != 0 {
            return None;
        }
        let mut c = [PDH_HCOUNTER::default(); 3];
        let paths = [
            w!("\\PhysicalDisk(*)\\% Idle Time"),
            w!("\\PhysicalDisk(*)\\Disk Read Bytes/sec"),
            w!("\\PhysicalDisk(*)\\Disk Write Bytes/sec"),
        ];
        for (i, p) in paths.iter().enumerate() {
            if PdhAddEnglishCounterW(q, *p, 0, &mut c[i]) != 0 {
                let _ = PdhCloseQuery(q);
                return None;
            }
        }
        let _ = PdhCollectQueryData(q);
        Some((q, c))
    }
}

/// Instance name ("0 C: D:") -> value, keyed by disk index.
unsafe fn read_by_disk(counter: PDH_HCOUNTER) -> HashMap<u32, (String, f64)> {
    let mut size = 0u32;
    let mut count = 0u32;
    let mut out = HashMap::new();
    if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None) != PDH_MORE_DATA || size == 0 {
        return out;
    }
    let mut buf = vec![0u64; (size as usize).div_ceil(8) + 1];
    let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
    if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items)) != 0 {
        return out;
    }
    for i in 0..count as usize {
        let item = &*items.add(i);
        let name = item.szName.to_string().unwrap_or_default();
        let mut parts = name.splitn(2, ' ');
        if let Some(Ok(idx)) = parts.next().map(str::parse::<u32>) {
            out.insert(idx, (parts.next().unwrap_or("").trim().to_string(), item.FmtValue.Anonymous.doubleValue));
        }
    }
    out
}

impl DriveProbe {
    pub fn new() -> Self {
        let mut drives = Vec::new();
        unsafe {
            for index in 0..16 {
                let Some(h) = open_drive(index) else { continue };
                let model = model_of(h).unwrap_or_else(|| format!("Disk {index}"));
                let _ = CloseHandle(h);
                drives.push(Drive { index, model });
            }
        }
        Self { drives, query: open_query() }
    }

    pub fn sample(&mut self) -> Vec<Value> {
        let (idle, read, write) = match self.query {
            Some((q, c)) => unsafe {
                if PdhCollectQueryData(q) == 0 {
                    (read_by_disk(c[0]), read_by_disk(c[1]), read_by_disk(c[2]))
                } else {
                    Default::default()
                }
            },
            None => Default::default(),
        };
        self.drives
            .iter()
            .map(|d| {
                let temp = unsafe { open_drive(d.index).and_then(|h| {
                    let t = temperature_of(h);
                    let _ = CloseHandle(h);
                    t
                }) };
                let letters = idle.get(&d.index).map(|(n, _)| n.clone()).unwrap_or_default();
                json!({
                    "index": d.index,
                    "model": d.model,
                    "letters": letters,
                    "temp": temp,
                    "active": idle.get(&d.index).map(|(_, v)| ((100.0 - v).clamp(0.0, 100.0) * 10.0).round() / 10.0),
                    "read": read.get(&d.index).map(|(_, v)| v.round()),
                    "write": write.get(&d.index).map(|(_, v)| v.round()),
                })
            })
            .collect()
    }
}
