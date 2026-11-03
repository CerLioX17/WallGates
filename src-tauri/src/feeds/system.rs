//! Hardware & OS statistics for dashboard / terminal wallpapers ("system" capability).
//!
//! Sources, best first:
//! * HWiNFO shared memory (Windows, if HWiNFO runs with "Shared Memory Support"): every sensor.
//! * GPU driver counters (Windows: DXGI + D3DKMT + PDH, the same data Task Manager shows).
//! * sysinfo: CPU, per-core load, memory, swap, disks, network, processes, components.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Instant;
use sysinfo::{Components, Disks, Networks, ProcessesToUpdate, System};

pub struct SystemFeed {
    sys: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    last_net: Instant,
    tick: u64,
    #[cfg(windows)]
    gpu: super::gpu_windows::GpuProbe,
}

impl Default for SystemFeed {
    fn default() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        Self {
            sys,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            last_net: Instant::now(),
            tick: 0,
            #[cfg(windows)]
            gpu: super::gpu_windows::GpuProbe::new(),
        }
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Pick the most relevant CPU temperature from generic sensor labels.
fn cpu_temp_from(labels: &[(String, f64)]) -> Option<f64> {
    const PREFERRED: &[&str] = &["cpu (tctl/tdie)", "cpu package", "tctl", "tdie", "package id 0", "cpu die", "core max", "cpu"];
    for p in PREFERRED {
        if let Some((_, v)) = labels.iter().find(|(l, v)| l.to_ascii_lowercase().contains(p) && *v > 0.0 && *v < 130.0) {
            return Some(*v);
        }
    }
    None
}

impl SystemFeed {
    pub fn sample(&mut self) -> Value {
        self.tick += 1;
        let sys = &mut self.sys;
        sys.refresh_cpu_all();
        sys.refresh_memory();
        // Processes are the expensive part: every sample (2 s) is fine for a visible dashboard.
        sys.refresh_processes(ProcessesToUpdate::All, true);
        if self.tick % 15 == 1 {
            self.disks.refresh(true);
        }
        self.networks.refresh(true);
        self.components.refresh(true);

        let cores: Vec<f64> = sys.cpus().iter().map(|c| round1(c.cpu_usage() as f64)).collect();
        let n = cores.len().max(1) as f64;
        let freq = sys.cpus().iter().map(|c| c.frequency() as f64).sum::<f64>() / n;
        let cpu_name = sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();

        // Top processes, grouped by executable name (browsers spawn dozens).
        let mut groups: HashMap<String, (f64, u64, u32)> = HashMap::new();
        for p in sys.processes().values() {
            let name = p.name().to_string_lossy().to_string();
            if name.is_empty() || name == "System Idle Process" || name == "Idle" {
                continue;
            }
            let e = groups.entry(name).or_insert((0.0, 0, 0));
            e.0 += p.cpu_usage() as f64 / n;
            e.1 += p.memory();
            e.2 += 1;
        }
        let mut top: Vec<(String, (f64, u64, u32))> = groups.into_iter().collect();
        top.sort_by(|a, b| b.1 .0.partial_cmp(&a.1 .0).unwrap_or(std::cmp::Ordering::Equal).then(b.1 .1.cmp(&a.1 .1)));
        let processes: Vec<Value> = top
            .iter()
            .take(10)
            .map(|(name, (cpu, mem, count))| json!({ "name": name, "cpu": round1(*cpu), "memory": mem, "count": count }))
            .collect();
        let mut by_mem = top.clone();
        by_mem.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
        let memory_hogs: Vec<Value> = by_mem
            .iter()
            .take(5)
            .map(|(name, (cpu, mem, count))| json!({ "name": name, "cpu": round1(*cpu), "memory": mem, "count": count }))
            .collect();

        let disks: Vec<Value> = self
            .disks
            .list()
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| {
                json!({
                    "name": d.name().to_string_lossy(),
                    "mount": d.mount_point().to_string_lossy(),
                    "total": d.total_space(),
                    "used": d.total_space().saturating_sub(d.available_space()),
                    "removable": d.is_removable(),
                })
            })
            .collect();

        let secs = self.last_net.elapsed().as_secs_f64().max(0.5);
        self.last_net = Instant::now();
        let (rx, tx) = self
            .networks
            .iter()
            .fold((0u64, 0u64), |(r, t), (_, d)| (r + d.received(), t + d.transmitted()));

        let comp: Vec<(String, f64)> = self
            .components
            .list()
            .iter()
            .filter_map(|c| c.temperature().map(|t| (c.label().to_string(), t as f64)))
            .collect();

        let total = sys.total_memory();
        let used = sys.used_memory();
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut out = json!({
            "cpu": round1(sys.global_cpu_usage() as f64),
            "cpuName": cpu_name,
            "cores": cores,
            "cpuFreq": freq.round(),
            "cpuTemp": cpu_temp_from(&comp),
            "memoryUsed": used,
            "memoryTotal": total,
            "memory": if total > 0 { round1(used as f64 / total as f64 * 100.0) } else { 0.0 },
            "swapUsed": sys.used_swap(),
            "swapTotal": sys.total_swap(),
            "disks": disks,
            "net": { "rx": (rx as f64 / secs).round(), "tx": (tx as f64 / secs).round() },
            "processes": processes,
            "memoryHogs": memory_hogs,
            "processCount": sys.processes().len(),
            "uptime": System::uptime(),
            "host": System::host_name().unwrap_or_default(),
            "os": System::long_os_version().unwrap_or_default(),
            "gpu": Value::Null,
            "sensors": comp.iter().map(|(l, v)| json!({ "label": l, "unit": "°C", "value": round1(*v), "type": "temperature" })).collect::<Vec<_>>(),
            "sources": { "temperatures": if comp.is_empty() { Value::Null } else { json!("system") } },
        });

        #[cfg(windows)]
        {
            if let Some(gpu) = self.gpu.sample() {
                out["gpu"] = gpu;
            }
            if let Some(hw) = super::hwinfo::read() {
                let pairs: Vec<(String, f64)> = hw
                    .iter()
                    .filter(|r| r.kind == "temperature")
                    .map(|r| (r.label.clone(), r.value))
                    .collect();
                if let Some(t) = cpu_temp_from(&pairs) {
                    out["cpuTemp"] = json!(round1(t));
                }
                let find = |names: &[&str]| {
                    hw.iter()
                        .find(|r| names.iter().any(|n| r.label.eq_ignore_ascii_case(n)))
                        .map(|r| round1(r.value))
                };
                if let Some(g) = out.get_mut("gpu").filter(|g| g.is_object()) {
                    if let Some(t) = find(&["GPU Temperature"]) {
                        g["temp"] = json!(t);
                    }
                    if let Some(p) = find(&["GPU Power", "GPU Package Power", "Total Board Power"]) {
                        g["power"] = json!(p);
                    }
                    if let Some(t) = find(&["GPU Memory Junction Temperature", "GPU Hot Spot Temperature"]) {
                        g["hotspot"] = json!(t);
                    }
                }
                if let Some(p) = find(&["CPU Package Power", "CPU PPT"]) {
                    out["cpuPower"] = json!(p);
                }
                out["sensors"] = json!(hw
                    .iter()
                    .take(400)
                    .map(|r| json!({ "label": r.label, "unit": r.unit, "value": round1(r.value), "type": r.kind }))
                    .collect::<Vec<_>>());
                out["sources"]["temperatures"] = json!("hwinfo");
            } else if out["gpu"].get("temp").is_some_and(|t| !t.is_null()) && out["sources"]["temperatures"].is_null() {
                out["sources"]["temperatures"] = json!("gpu-driver");
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_cpu_temperature() {
        let l = vec![("GPU Temperature".to_string(), 55.0), ("CPU (Tctl/Tdie)".to_string(), 61.5), ("Core Max".into(), 58.0)];
        assert_eq!(cpu_temp_from(&l), Some(61.5));
        assert_eq!(cpu_temp_from(&[("acpitz temp1".into(), 40.0)]), None);
        assert_eq!(cpu_temp_from(&[("coretemp Package id 0".into(), 47.0)]), Some(47.0));
    }

    #[test]
    fn samples_real_system() {
        let mut f = SystemFeed::default();
        let v = f.sample();
        assert!(v["memoryTotal"].as_u64().unwrap() > 0);
        assert!(!v["cores"].as_array().unwrap().is_empty());
        assert!(v["processes"].as_array().unwrap().len() > 0);
    }
}
