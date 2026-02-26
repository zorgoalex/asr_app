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
    let client = Client::builder()
        .timeout(Duration::from_secs(cfg.timeout_secs))
        .build()
        .context("failed to build http client")?;

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
