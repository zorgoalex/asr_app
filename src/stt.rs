use crate::audio::AudioBuffer;
use crate::config::AppConfig;
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::{multipart::Form, multipart::Part, Client, Response};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
}

pub fn transcribe(audio: &AudioBuffer, cfg: &AppConfig, api_key: &str) -> Result<String> {
    let url = format!("{}/audio/transcriptions", cfg.api_base_url.trim_end_matches('/'));
    let mut builder = Client::builder().timeout(Duration::from_secs(cfg.timeout_secs));
    if !use_proxy_from_env() {
        builder = builder.no_proxy();
    }
    let client = builder.build().context("failed to build http client")?;

    let mut form = Form::new()
        .part(
            "file",
            Part::bytes(audio.wav_data.clone())
                .file_name("audio.wav")
                .mime_str("audio/wav")?,
        )
        .text("model", cfg.stt_model.clone())
        .text("response_format", "json");

    if !cfg.language.trim().is_empty() && cfg.language != "auto" {
        form = form.text("language", cfg.language.clone());
    }

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .context("failed to send transcription request")?;

    handle_response(response)
}

fn handle_response(response: Response) -> Result<String> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(anyhow!("api error {}: {}", status, body));
    }
    let resp: TranscriptionResponse = response.json().context("invalid response")?;
    Ok(resp.text)
}

fn use_proxy_from_env() -> bool {
    std::env::var("VOICE_ASR_USE_PROXY")
        .map(|v| parse_bool_env(&v))
        .unwrap_or(false)
}

fn parse_bool_env(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::transcribe;
    use super::parse_bool_env;
    use crate::audio::AudioBuffer;
    use crate::config::AppConfig;
    use hound::WavSpec;
    use std::f32::consts::PI;
    use std::io::Cursor;

    fn make_test_audio() -> AudioBuffer {
        let sample_rate = 16_000u32;
        let duration_ms = 1_000u64;
        let samples_len = (sample_rate as u64 * duration_ms / 1000) as usize;
        let freq_hz = 440.0f32;
        let amp = i16::MAX as f32 * 0.2;
        let mut samples = Vec::with_capacity(samples_len);
        for i in 0..samples_len {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * PI * freq_hz * t).sin() * amp;
            samples.push(sample as i16);
        }

        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec)
                .expect("failed to create wav writer");
            for s in samples {
                writer.write_sample(s).expect("failed to write sample");
            }
            writer.finalize().expect("failed to finalize wav");
        }

        AudioBuffer {
            wav_data: cursor.into_inner(),
            sample_rate,
            duration_ms,
        }
    }

    #[test]
    #[ignore = "requires GROQ_API_KEY and network access"]
    fn groq_transcribe_smoke() {
        let api_key = std::env::var("GROQ_API_KEY")
            .expect("set GROQ_API_KEY to run the Groq integration test");
        let mut cfg = AppConfig::default();
        cfg.timeout_secs = 60;
        let audio = make_test_audio();
        let _ = transcribe(&audio, &cfg, &api_key).expect("transcribe failed");
    }

    #[test]
    fn parse_proxy_env_values() {
        assert!(parse_bool_env("1"));
        assert!(parse_bool_env("true"));
        assert!(parse_bool_env("YES"));
        assert!(parse_bool_env("On"));
        assert!(!parse_bool_env("0"));
        assert!(!parse_bool_env("false"));
        assert!(!parse_bool_env("off"));
        assert!(!parse_bool_env(""));
    }
}
