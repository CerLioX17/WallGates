//! Live data for interactive wallpapers. Every feed runs only while a visible,
//! unpaused wallpaper declares the matching capability.

pub mod hwinfo;
pub mod markets;
pub mod sun;
pub mod system;
pub mod weather;

#[cfg(windows)]
pub mod gpu_windows;

#[cfg(windows)]
pub mod audio;
#[cfg(windows)]
pub mod media;

#[cfg(not(windows))]
pub mod audio {
    //! System audio capture is Windows-only for now (WASAPI loopback).
    pub struct AudioCapture;
    impl AudioCapture {
        pub fn start() -> Option<Self> {
            None
        }
        pub fn snapshot(&self) -> (Vec<f32>, f32) {
            (Vec::new(), 0.0)
        }
    }
}

#[cfg(not(windows))]
pub mod media {
    use serde_json::Value;
    #[derive(Default)]
    pub struct MediaFeed;
    impl MediaFeed {
        pub fn sample(&mut self) -> Option<Value> {
            None
        }
    }
}
