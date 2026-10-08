//! Sound cards, through cpal: what there is, and opening one for output.
//!
//! The card's own thread is the audio thread while it is open: its callback
//! runs `Shared::callback` and nothing else. Its timestamp is the host time
//! the clock is stamped with, on the same clock as `shm::monotonic_ns`.

use crate::audio::{DeviceInfo, Output, Shared};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Failed = (&'static str, String);

/// An open output. Dropping it closes the card.
pub struct Stream {
    _out: cpal::Stream,
}

pub struct Found {
    device: cpal::Device,
    name: String,
}

pub struct Listed {
    pub name: String,
    pub inputs: u32,
    pub outputs: u32,
    pub rates: Vec<u32>,
}

/// The rates a session is likely to be at, for `device.list`.
const RATES: [u32; 6] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000];

fn name_of(d: &cpal::Device) -> Option<String> {
    d.description().ok().map(|d| d.name().to_string())
}

/// Every device with an output, and how many inputs the device of the same
/// name has.
pub fn list() -> Vec<Listed> {
    let host = cpal::default_host();
    let Ok(outputs) = host.output_devices() else {
        return Vec::new();
    };
    let mut listed = Vec::new();
    for d in outputs {
        let (Some(name), Ok(configs)) = (name_of(&d), d.supported_output_configs()) else {
            continue;
        };
        let configs: Vec<_> = configs.collect();
        let inputs = d
            .supported_input_configs()
            .map(|c| c.map(|c| c.channels() as u32).max().unwrap_or(0))
            .unwrap_or(0);
        listed.push(Listed {
            name,
            inputs,
            outputs: configs
                .iter()
                .map(|c| c.channels() as u32)
                .max()
                .unwrap_or(0),
            rates: RATES
                .iter()
                .copied()
                .filter(|r| configs.iter().any(|c| c.contains_rate(*r)))
                .collect(),
        });
    }
    listed
}

/// A device by its name, or the system's default output for `None`.
pub fn find(name: Option<&str>) -> Result<Found, Failed> {
    let host = cpal::default_host();
    let device = match name {
        None => host.default_output_device(),
        Some(name) => host
            .output_devices()
            .ok()
            .and_then(|mut all| all.find(|d| name_of(d).as_deref() == Some(name))),
    };
    let found = device.and_then(|device| name_of(&device).map(|name| Found { device, name }));
    found.ok_or_else(|| {
        (
            "no_such_device",
            match name {
                Some(name) => format!("No audio device named \"{name}\"."),
                None => "There is no audio output device.".into(),
            },
        )
    })
}

/// Opens `found` at `rate` in blocks of `block`. Its callbacks carry
/// `token`, and play silence until the worker makes that token the active one.
pub fn open(
    found: Found,
    rate: u32,
    block: u32,
    shared: Arc<Shared>,
    token: u64,
) -> Result<(Stream, DeviceInfo), Failed> {
    let Found { device, name } = found;
    let failed = |why: String| {
        (
            "device_failed",
            format!("The audio device \"{name}\" didn't open: {why}."),
        )
    };
    let configs: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| failed(e.to_string()))?
        .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
        .collect();
    // Two channels when the card has them, else the fewest above, else one.
    let config = configs
        .iter()
        .filter(|c| c.contains_rate(rate))
        .min_by_key(|c| match c.channels() {
            2 => (0, 0),
            n if n > 2 => (1, n),
            n => (2, n),
        })
        .ok_or_else(|| {
            let has: Vec<String> = RATES
                .iter()
                .filter(|r| configs.iter().any(|c| c.contains_rate(**r)))
                .map(|r| r.to_string())
                .collect();
            failed(format!(
                "it doesn't run at {rate} Hz (it runs at {})",
                has.join(", ")
            ))
        })?;
    let channels = config.channels() as usize;
    let config = cpal::StreamConfig {
        channels: config.channels(),
        sample_rate: rate,
        buffer_size: cpal::BufferSize::Fixed(block),
    };
    // Frames from a callback to the speaker, as the card's first callback says.
    let latency = Arc::new(AtomicI64::new(-1));
    let out = device
        .build_output_stream(
            config,
            {
                let (latency, shared) = (latency.clone(), shared.clone());
                move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    let ts = info.timestamp();
                    let ahead = ts.playback.saturating_duration_since(ts.callback);
                    latency.store(
                        (ahead.as_nanos() as u64 * rate as u64 / 1_000_000_000) as i64,
                        Ordering::Relaxed,
                    );
                    let n = data.len() / channels;
                    let host = ts.callback.as_nanos() as u64;
                    shared.callback(token, n, Output::Interleaved { data, channels }, Some(host));
                }
            },
            {
                move |e: cpal::Error| match e.kind() {
                    // Said on the card's own thread: counted, not logged.
                    cpal::ErrorKind::Xrun => shared.count_xrun(),
                    _ => eprintln!("wwav-engine: the audio device says: {e}"),
                }
            },
            Some(Duration::from_secs(5)),
        )
        .map_err(|e| failed(e.to_string()))?;
    out.play().map_err(|e| failed(e.to_string()))?;
    let until = Instant::now() + Duration::from_millis(500);
    while latency.load(Ordering::Relaxed) < 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    let block = out.buffer_size().unwrap_or(block);
    let info = DeviceInfo {
        name: Some(name),
        rate,
        block,
        output_latency: latency.load(Ordering::Relaxed).max(0),
        input_latency: 0,
    };
    Ok((Stream { _out: out }, info))
}

/// An open input, feeding the capture ring. Dropping it closes the card.
pub struct Input {
    _stream: cpal::Stream,
    pub name: String,
    /// Samples a frame the take keeps: 1 or 2.
    pub channels: usize,
    /// Frames from a sound reaching the card to its callback.
    pub latency: i64,
}

/// What the input named `name`, or the system's default input, would record.
fn find_input(name: Option<&str>) -> Result<(cpal::Device, String), Failed> {
    let host = cpal::default_host();
    let device = match name {
        None => host.default_input_device(),
        Some(name) => host
            .input_devices()
            .ok()
            .and_then(|mut all| all.find(|d| name_of(d).as_deref() == Some(name))),
    };
    device
        .and_then(|d| name_of(&d).map(|name| (d, name)))
        .ok_or_else(|| {
            (
                "no_such_device",
                match name {
                    Some(name) => format!("No audio input named \"{name}\"."),
                    None => "There is no audio input.".into(),
                },
            )
        })
}

/// Opens an input at `rate` and starts it pushing into the capture ring,
/// each callback's frames stamped with when they were captured.
pub fn open_input(name: Option<&str>, rate: u32, shared: Arc<Shared>) -> Result<Input, Failed> {
    let (device, name) = find_input(name)?;
    let failed = |why: String| {
        (
            "device_failed",
            format!("The audio input \"{name}\" didn't open: {why}."),
        )
    };
    let config = device
        .supported_input_configs()
        .map_err(|e| failed(e.to_string()))?
        .filter(|c| c.sample_format() == cpal::SampleFormat::F32 && c.contains_rate(rate))
        .min_by_key(|c| c.channels())
        .ok_or_else(|| failed(format!("it doesn't run at {rate} Hz")))?;
    let of = config.channels() as usize;
    let channels = of.min(2);
    shared.capture().begin(channels, false);
    let config = cpal::StreamConfig {
        channels: config.channels(),
        sample_rate: rate,
        buffer_size: cpal::BufferSize::Default,
    };
    let latency = Arc::new(AtomicI64::new(-1));
    let stream = device
        .build_input_stream(
            config,
            {
                let latency = latency.clone();
                move |data: &[f32], info: &cpal::InputCallbackInfo| {
                    let ts = info.timestamp();
                    let behind = ts.callback.saturating_duration_since(ts.capture);
                    latency.store(
                        (behind.as_nanos() as u64 * rate as u64 / 1_000_000_000) as i64,
                        Ordering::Relaxed,
                    );
                    shared
                        .capture()
                        .push_input(data, of, ts.capture.as_nanos() as u64);
                }
            },
            |e: cpal::Error| {
                if e.kind() != cpal::ErrorKind::Xrun {
                    eprintln!("wwav-engine: the audio input says: {e}");
                }
            },
            Some(Duration::from_secs(5)),
        )
        .map_err(|e| failed(e.to_string()))?;
    stream.play().map_err(|e| failed(e.to_string()))?;
    let until = Instant::now() + Duration::from_millis(1000);
    while latency.load(Ordering::Relaxed) < 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    if latency.load(Ordering::Relaxed) < 0 {
        return Err(failed("it gave no audio within a second".into()));
    }
    Ok(Input {
        _stream: stream,
        name,
        channels,
        latency: latency.load(Ordering::Relaxed),
    })
}
