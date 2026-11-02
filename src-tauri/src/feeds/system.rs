//! CPU and memory usage for dashboard wallpapers.

use serde_json::{json, Value};
use sysinfo::System;

pub struct SystemFeed {
    sys: System,
}

impl Default for SystemFeed {
    fn default() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        Self { sys }
    }
}

impl SystemFeed {
    pub fn sample(&mut self) -> Value {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        let total = self.sys.total_memory();
        let used = self.sys.used_memory();
        json!({
            "cpu": (self.sys.global_cpu_usage() as f64 * 10.0).round() / 10.0,
            "memoryUsed": used,
            "memoryTotal": total,
            "memory": if total > 0 { ((used as f64 / total as f64) * 1000.0).round() / 10.0 } else { 0.0 },
            "uptime": System::uptime(),
        })
    }
}
