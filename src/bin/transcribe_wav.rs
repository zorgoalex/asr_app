use anyhow::{Context, Result};
use std::path::PathBuf;
use voice_asr_client::audio::AudioBuffer;
use voice_asr_client::config::AppConfig;
use voice_asr_client::stt;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let wav_path = match args.next() {
        Some(v) => PathBuf::from(v),
        None => {
            eprintln!("Usage: transcribe_wav <path-to-wav>");
            std::process::exit(2);
        }
    };

    let api_key = std::env::var("GROQ_API_KEY")
        .context("GROQ_API_KEY is not set in the environment")?;

    let (wav_data, sample_rate, duration_ms) = load_wav(&wav_path)?;
    let audio = AudioBuffer {
        wav_data,
        sample_rate,
        duration_ms,
    };

    let cfg = AppConfig::default();
    let text = stt::transcribe(&audio, &cfg, &api_key)?;
    println!("{}", text);
    Ok(())
}

fn load_wav(path: &PathBuf) -> Result<(Vec<u8>, u32, u64)> {
    let wav_data = std::fs::read(path).context("failed to read wav file")?;
    let reader = hound::WavReader::new(std::io::Cursor::new(&wav_data))
        .context("invalid wav file")?;
    let spec = reader.spec();
    let duration_samples = reader.duration() as u64;
    let sample_rate = spec.sample_rate;
    let duration_ms = if sample_rate > 0 {
        duration_samples.saturating_mul(1000) / sample_rate as u64
    } else {
        0
    };
    Ok((wav_data, sample_rate, duration_ms))
}
