//! Read sensors from HWiNFO's shared memory (Windows), when HWiNFO is running
//! with "Shared Memory Support" enabled. This is the standard, read-only way
//! third-party gadgets get CPU temperatures and every other sensor without
//! installing a kernel driver.

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::Foundation::CloseHandle;
#[cfg(windows)]
use windows::Win32::System::Memory::{MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ};

const SIGNATURE: u32 = 0x5369_5748; // "HWiS"
const HEADER_SIZE: usize = 48;
const READING_MIN_SIZE: usize = 316;

#[cfg_attr(not(windows), allow(dead_code))]
pub struct Reading {
    pub kind: &'static str,
    pub label: String,
    pub unit: String,
    pub value: f64,
}

fn kind_of(t: u32) -> &'static str {
    match t {
        1 => "temperature",
        2 => "voltage",
        3 => "fan",
        4 => "current",
        5 => "power",
        6 => "clock",
        7 => "usage",
        _ => "other",
    }
}

/// HWiNFO strings are fixed-size, NUL-terminated, Windows-1252: map bytes to
/// Latin-1 (covers the "°" sign).
fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    bytes[..end].iter().map(|b| *b as char).collect::<String>().trim().to_string()
}

fn u32_at(base: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(base[off..off + 4].try_into().unwrap())
}

fn f64_at(base: &[u8], off: usize) -> f64 {
    f64::from_le_bytes(base[off..off + 8].try_into().unwrap())
}

/// Parse a copy of the shared memory block (pure function, unit-tested).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse(mem: &[u8]) -> Option<Vec<Reading>> {
    if mem.len() < HEADER_SIZE || u32_at(mem, 0) != SIGNATURE {
        return None;
    }
    // Header (packed): sig, version, revision, poll_time (i64), sensor off/size/count, reading off/size/count, period
    let reading_off = u32_at(mem, 32) as usize;
    let reading_size = u32_at(mem, 36) as usize;
    let reading_count = u32_at(mem, 40) as usize;
    if reading_size < READING_MIN_SIZE || reading_count > 4000 {
        return None;
    }
    let mut out = Vec::with_capacity(reading_count);
    for i in 0..reading_count {
        let start = reading_off + i * reading_size;
        let Some(r) = mem.get(start..start + READING_MIN_SIZE) else { break };
        let label_user = cstr(&r[140..268]);
        let label = if label_user.is_empty() { cstr(&r[12..140]) } else { label_user };
        let value = f64_at(r, 284);
        if !value.is_finite() || label.is_empty() {
            continue;
        }
        out.push(Reading { kind: kind_of(u32_at(r, 0)), label, unit: cstr(&r[268..284]), value });
    }
    Some(out)
}

#[cfg(windows)]
pub fn read() -> Option<Vec<Reading>> {
    unsafe {
        let handle = OpenFileMappingW(FILE_MAP_READ.0, false, w!("Global\\HWiNFO_SENS_SM2")).ok()?;
        let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0);
        if view.Value.is_null() {
            let _ = CloseHandle(handle);
            return None;
        }
        let base = view.Value as *const u8;
        let header = std::slice::from_raw_parts(base, HEADER_SIZE);
        let result = if u32_at(header, 0) == SIGNATURE {
            let total = u32_at(header, 32) as usize + u32_at(header, 36) as usize * u32_at(header, 40) as usize;
            if total < 64 * 1024 * 1024 {
                let copy = std::slice::from_raw_parts(base, total).to_vec();
                parse(&copy)
            } else {
                None
            }
        } else {
            None
        };
        let _ = UnmapViewOfFile(view);
        let _ = CloseHandle(handle);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(buf: &mut [u8], off: usize, bytes: &[u8]) {
        buf[off..off + bytes.len()].copy_from_slice(bytes);
    }

    #[test]
    fn parses_shared_memory_layout() {
        let size = 460; // newer HWiNFO versions append UTF-8 labels after the 316 legacy bytes
        let mut mem = vec![0u8; HEADER_SIZE + 2 * size];
        put(&mut mem, 0, &SIGNATURE.to_le_bytes());
        put(&mut mem, 32, &(HEADER_SIZE as u32).to_le_bytes());
        put(&mut mem, 36, &(size as u32).to_le_bytes());
        put(&mut mem, 40, &2u32.to_le_bytes());
        let r0 = HEADER_SIZE;
        put(&mut mem, r0, &1u32.to_le_bytes());
        put(&mut mem, r0 + 12, b"CPU (Tctl/Tdie)");
        put(&mut mem, r0 + 268, &[0xB0, b'C']);
        put(&mut mem, r0 + 284, &63.25f64.to_le_bytes());
        let r1 = HEADER_SIZE + size;
        put(&mut mem, r1, &7u32.to_le_bytes());
        put(&mut mem, r1 + 12, b"Total CPU Usage");
        put(&mut mem, r1 + 140, b"CPU load");
        put(&mut mem, r1 + 268, b"%");
        put(&mut mem, r1 + 284, &12.5f64.to_le_bytes());
        let r = parse(&mem).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].kind, r[0].label.as_str(), r[0].unit.as_str(), r[0].value), ("temperature", "CPU (Tctl/Tdie)", "\u{b0}C", 63.25));
        assert_eq!(r[1].label, "CPU load"); // user label wins
        assert!(parse(&[0u8; 10]).is_none());
    }
}
