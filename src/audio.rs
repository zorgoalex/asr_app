use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, StreamConfig};
use std::io::Cursor;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AudioRecorder {
    device: cpal::Device,
    config: StreamConfig,
    sample_format: SampleFormat,
    channels: u16,
}

pub struct RecordingSession {
    samples: Arc<Mutex<Vec<i16>>>,
    stream: cpal::Stream,
    sample_rate: u32,
}

pub struct AudioBuffer {
    pub wav_data: Vec<u8>,
    pub sample_rate: u32,
    pub duration_ms: u64,
}

pub fn list_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let mut devices = Vec::new();
    for device in host.input_devices()? {
        devices.push(device.name()?);
    }
    Ok(devices)
}

impl AudioRecorder {
    pub fn new(device_name: Option<String>) -> Result<Self> {
        let host = cpal::default_host();
        let device = if let Some(name) = device_name {
            host.input_devices()?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .ok_or_else(|| anyhow!("input device not found"))?
        } else {
            host.default_input_device()
                .ok_or_else(|| anyhow!("no input device available"))?
        };

        let mut config = device.default_input_config()?;
        if let Ok(mut supported) = device.supported_input_configs() {
            for range in supported {
                if range.min_sample_rate().0 <= 16_000 && range.max_sample_rate().0 >= 16_000 {
                    let with_rate = range.with_sample_rate(cpal::SampleRate(16_000));
                    config = with_rate;
                    break;
                }
            }
        }
        Ok(Self {
            device,
            config: config.clone().into(),
            sample_format: config.sample_format(),
            channels: config.channels(),
        })
    }

    pub fn start(&self) -> Result<RecordingSession> {
        let samples = Arc::new(Mutex::new(Vec::<i16>::new()));
        let samples_clone = samples.clone();
        let err_fn = |err| {
            log::error!("audio stream error: {}", err);
        };

        let channels = self.channels as usize;
        let stream = match self.sample_format {
            SampleFormat::I16 => self.device.build_input_stream(
                &self.config,
                move |data: &[i16], _| {
                    let mut guard = samples_clone.lock().unwrap();
                    downmix_i16(data, channels, &mut guard);
                },
                err_fn,
                None,
            )?,
            SampleFormat::U16 => self.device.build_input_stream(
                &self.config,
                move |data: &[u16], _| {
                    let mut guard = samples_clone.lock().unwrap();
                    let mut tmp: Vec<i16> = data.iter().map(|s| Sample::to_i16(s)).collect();
                    downmix_i16(&tmp, channels, &mut guard);
                },
                err_fn,
                None,
            )?,
            SampleFormat::F32 => self.device.build_input_stream(
                &self.config,
                move |data: &[f32], _| {
                    let mut guard = samples_clone.lock().unwrap();
                    let mut tmp: Vec<i16> = data.iter().map(|s| Sample::to_i16(s)).collect();
                    downmix_i16(&tmp, channels, &mut guard);
                },
                err_fn,
                None,
            )?,
            _ => return Err(anyhow!("unsupported sample format")),
        };

        stream.play()?;

        Ok(RecordingSession {
            samples,
            stream,
            sample_rate: self.config.sample_rate.0,
        })
    }
}

impl RecordingSession {
    pub fn stop(self) -> Result<AudioBuffer> {
        drop(self.stream);
        let guard = self.samples.lock().unwrap();
        let samples = guard.clone();
        let wav_data = samples_to_wav(&samples, self.sample_rate)?;
        let duration_ms = if self.sample_rate > 0 {
            (samples.len() as u64 * 1000) / self.sample_rate as u64
        } else {
            0
        };
        Ok(AudioBuffer {
            wav_data,
            sample_rate: self.sample_rate,
            duration_ms,
        })
    }
}

fn samples_to_wav(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .context("failed to create wav writer")?;
        for s in samples {
            writer.write_sample(*s)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

fn downmix_i16(input: &[i16], channels: usize, out: &mut Vec<i16>) {
    if channels <= 1 {
        out.extend_from_slice(input);
        return;
    }
    for frame in input.chunks(channels) {
        let mut sum: i32 = 0;
        for s in frame {
            sum += *s as i32;
        }
        let avg = (sum / channels as i32) as i16;
        out.push(avg);
    }
}
