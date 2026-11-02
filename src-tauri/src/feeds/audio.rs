//! System audio spectrum via WASAPI loopback (what you hear, not the microphone).
//!
//! A capture thread reads the default output device in loopback mode, an FFT
//! turns the latest 2048 samples into 64 log-spaced bands (30 Hz – 16 kHz),
//! normalised to 0..1 with fast attack / slow decay.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{num_complex::Complex, FftPlanner};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const BANDS: usize = 64;
const FFT_SIZE: usize = 2048;

struct Shared {
    samples: Mutex<VecDeque<f32>>,
    sample_rate: Mutex<f32>,
    bands: Mutex<(Vec<f32>, f32)>,
    stop: AtomicBool,
}

pub struct AudioCapture {
    shared: Arc<Shared>,
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

impl AudioCapture {
    pub fn start() -> Option<Self> {
        let shared = Arc::new(Shared {
            samples: Mutex::new(VecDeque::with_capacity(FFT_SIZE * 2)),
            sample_rate: Mutex::new(48_000.0),
            bands: Mutex::new((vec![0.0; BANDS], 0.0)),
            stop: AtomicBool::new(false),
        });
        let s = shared.clone();
        std::thread::Builder::new()
            .name("wallgates-audio".into())
            .spawn(move || run(s))
            .ok()?;
        Some(Self { shared })
    }

    /// Latest (bands, overall level).
    pub fn snapshot(&self) -> (Vec<f32>, f32) {
        self.shared.bands.lock().map(|b| b.clone()).unwrap_or_else(|_| (vec![0.0; BANDS], 0.0))
    }
}

fn push_samples(shared: &Shared, data: &[f32], channels: usize) {
    let Ok(mut q) = shared.samples.lock() else { return };
    for frame in data.chunks(channels.max(1)) {
        q.push_back(frame.iter().sum::<f32>() / frame.len() as f32);
    }
    while q.len() > FFT_SIZE * 2 {
        q.pop_front();
    }
}

fn open_stream(shared: &Arc<Shared>, failed: &Arc<AtomicBool>) -> Option<cpal::Stream> {
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let config = device.default_output_config().ok()?;
    let channels = config.channels() as usize;
    if let Ok(mut sr) = shared.sample_rate.lock() {
        *sr = config.sample_rate().0 as f32;
    }
    let err_flag = failed.clone();
    let on_err = move |_e: cpal::StreamError| err_flag.store(true, Ordering::Relaxed);
    let stream_config: cpal::StreamConfig = config.clone().into();
    // On WASAPI, an input stream on an *output* device captures in loopback mode.
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let s = shared.clone();
            device.build_input_stream(&stream_config, move |d: &[f32], _| push_samples(&s, d, channels), on_err, None)
        }
        cpal::SampleFormat::I16 => {
            let s = shared.clone();
            device.build_input_stream(
                &stream_config,
                move |d: &[i16], _| {
                    let f: Vec<f32> = d.iter().map(|v| *v as f32 / i16::MAX as f32).collect();
                    push_samples(&s, &f, channels)
                },
                on_err,
                None,
            )
        }
        cpal::SampleFormat::I32 => {
            let s = shared.clone();
            device.build_input_stream(
                &stream_config,
                move |d: &[i32], _| {
                    let f: Vec<f32> = d.iter().map(|v| *v as f32 / i32::MAX as f32).collect();
                    push_samples(&s, &f, channels)
                },
                on_err,
                None,
            )
        }
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some(stream)
}

fn run(shared: Arc<Shared>) {
    let failed = Arc::new(AtomicBool::new(false));
    let mut stream = open_stream(&shared, &failed);
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos())
        .collect();
    let mut smooth = vec![0.0f32; BANDS];
    let mut level_smooth = 0.0f32;
    let mut retry_in = 0u32;

    while !shared.stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(33));

        // Device changed / unplugged: reopen every ~2 s.
        if stream.is_none() || failed.load(Ordering::Relaxed) {
            if retry_in == 0 {
                drop(stream.take());
                failed.store(false, Ordering::Relaxed);
                stream = open_stream(&shared, &failed);
                retry_in = 60;
            } else {
                retry_in -= 1;
            }
        }

        let samples: Vec<f32> = match shared.samples.lock() {
            Ok(q) if q.len() >= FFT_SIZE => q.iter().skip(q.len() - FFT_SIZE).copied().collect(),
            _ => {
                // Silence (nothing playing: WASAPI loopback delivers no packets).
                for v in &mut smooth {
                    *v *= 0.85;
                }
                level_smooth *= 0.85;
                if let Ok(mut b) = shared.bands.lock() {
                    *b = (smooth.clone(), level_smooth);
                }
                continue;
            }
        };
        if let Ok(mut q) = shared.samples.lock() {
            // Consume so silence decays instead of repeating the last buffer.
            let len = q.len();
            let keep = len.saturating_sub(FFT_SIZE / 2);
            q.drain(..keep.min(len));
        }
        let sr = shared.sample_rate.lock().map(|s| *s).unwrap_or(48_000.0);

        let rms = (samples.iter().map(|s| s * s).sum::<f32>() / FFT_SIZE as f32).sqrt();
        let mut buf: Vec<Complex<f32>> = samples.iter().zip(&window).map(|(s, w)| Complex::new(s * w, 0.0)).collect();
        fft.process(&mut buf);

        let bins = FFT_SIZE / 2;
        let f_min = 30.0f32;
        let f_max = 16_000.0f32.min(sr / 2.0);
        for (b, out) in smooth.iter_mut().enumerate() {
            let f0 = f_min * (f_max / f_min).powf(b as f32 / BANDS as f32);
            let f1 = f_min * (f_max / f_min).powf((b + 1) as f32 / BANDS as f32);
            let i0 = ((f0 * FFT_SIZE as f32 / sr) as usize).clamp(1, bins - 1);
            let i1 = ((f1 * FFT_SIZE as f32 / sr) as usize).clamp(i0 + 1, bins);
            let mag = buf[i0..i1].iter().map(|c| c.norm()).fold(0.0f32, f32::max) * 2.0 / FFT_SIZE as f32;
            let db = 20.0 * (mag + 1e-9).log10();
            let v = ((db + 72.0) / 62.0).clamp(0.0, 1.0);
            *out = if v > *out { v } else { *out * 0.86 + v * 0.14 };
        }
        let lvl = (rms * 3.0).clamp(0.0, 1.0);
        level_smooth = if lvl > level_smooth { lvl } else { level_smooth * 0.9 + lvl * 0.1 };
        if let Ok(mut b) = shared.bands.lock() {
            *b = (smooth.clone(), level_smooth);
        }
    }
}
