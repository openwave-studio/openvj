//! Live capture from an audio input device.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

use crate::Analyzer;

/// At most this many seconds of samples wait between frames; older ones are dropped.
const MAX_PENDING_SECS: usize = 1;

/// Names of the available input devices, default first.
pub fn input_device_names() -> Vec<String> {
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.description().ok()).map(|d| d.name().to_owned());
    let mut names: Vec<String> = host
        .input_devices()
        .map(|devs| devs.filter_map(|d| d.description().ok().map(|d| d.name().to_owned())).collect())
        .unwrap_or_default();
    if let Some(default) = default {
        names.retain(|n| *n != default);
        names.insert(0, default);
    }
    names
}

/// A running input stream. Dropping it stops capture.
pub struct AudioInput {
    _stream: cpal::Stream,
    pending: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    name: String,
}

impl AudioInput {
    /// Opens the named input device, or the default one.
    pub fn open(name: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();
        let device = match name {
            Some(name) => host
                .input_devices()?
                .find(|d| d.description().is_ok_and(|desc| desc.name() == name))
                .ok_or_else(|| anyhow!("no input device called {name}"))?,
            None => host.default_input_device().context("no audio input device")?,
        };
        let name = device.description().map(|d| d.name().to_owned()).unwrap_or_else(|_| "input".into());
        let config = device.default_input_config().context("input device has no usable config")?;
        let sample_rate = config.sample_rate();
        let channels = config.channels().max(1) as usize;
        let pending = Arc::new(Mutex::new(Vec::new()));
        let cap = sample_rate as usize * MAX_PENDING_SECS;

        let stream = match config.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, config.into(), channels, cap, pending.clone()),
            SampleFormat::I16 => build::<i16>(&device, config.into(), channels, cap, pending.clone()),
            SampleFormat::I32 => build::<i32>(&device, config.into(), channels, cap, pending.clone()),
            SampleFormat::U16 => build::<u16>(&device, config.into(), channels, cap, pending.clone()),
            other => bail!("unsupported sample format {other}"),
        }
        .with_context(|| format!("could not open {name}"))?;
        stream.play().with_context(|| format!("could not start {name}"))?;
        Ok(Self { _stream: stream, pending, sample_rate, name })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Moves captured samples into the analyser.
    pub fn drain_into(&self, analyzer: &mut Analyzer) {
        let samples = match self.pending.lock() {
            Ok(mut p) => std::mem::take(&mut *p),
            Err(_) => return,
        };
        analyzer.push(&samples);
    }
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    cap: usize,
    pending: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let stream = device.build_input_stream::<T, _, _>(
        config,
        move |data: &[T], _: &_| {
            // Never block the audio thread; drop this buffer if the UI holds the lock.
            let Ok(mut p) = pending.try_lock() else { return };
            for frame in data.chunks(channels) {
                let sum: f32 = frame.iter().map(|s| f32::from_sample_(*s)).sum();
                p.push(sum / channels as f32);
            }
            if p.len() > cap {
                let excess = p.len() - cap;
                p.drain(..excess);
            }
        },
        |e| eprintln!("openVJ: audio input error: {e}"),
        None,
    )?;
    Ok(stream)
}
