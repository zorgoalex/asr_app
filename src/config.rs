use crate::secret;
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordMode {
    Hold,
    Toggle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InjectMode {
    Direct,
    Clipboard,
    ClipboardOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Info,
    Debug,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub hotkey: String,
    pub record_mode: RecordMode,
    pub input_device: Option<String>,
    pub stt_model: String,
    pub language: String,
    pub timeout_secs: u64,
    pub max_record_secs: u64,
    pub inject_mode: InjectMode,
    pub log_level: LogLevel,
    pub autostart: bool,
    pub api_base_url: String,
    pub api_key_encrypted: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Alt+Space".to_string(),
            record_mode: RecordMode::Hold,
            input_device: None,
            stt_model: "whisper-large-v3".to_string(),
            language: "auto".to_string(),
            timeout_secs: 30,
            max_record_secs: 30,
            inject_mode: InjectMode::Direct,
            log_level: LogLevel::Info,
            autostart: false,
            api_base_url: "https://api.groq.com/openai/v1".to_string(),
            api_key_encrypted: None,
        }
    }
}

pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "zorgoalex", "VoiceASRClient")
            .context("failed to resolve config directory")?;
        let dir = proj_dirs.config_dir();
        fs::create_dir_all(dir).context("failed to create config directory")?;
        Ok(Self {
            path: dir.join("config.json"),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn from_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load_or_default(&self) -> Result<AppConfig> {
        if !self.path.exists() {
            let cfg = AppConfig::default();
            self.save(&cfg)?;
            return Ok(cfg);
        }
        let data = fs::read_to_string(&self.path).context("failed to read config file")?;
        let mut cfg: AppConfig =
            serde_json::from_str(&data).context("failed to parse config file")?;

        // Keep forward/backward compatibility.
        if cfg.api_base_url.trim().is_empty() {
            cfg.api_base_url = "https://api.groq.com/openai/v1".to_string();
        }
        if cfg.timeout_secs == 0 {
            cfg.timeout_secs = 30;
        }
        if cfg.max_record_secs == 0 {
            cfg.max_record_secs = 30;
        }
        Ok(cfg)
    }

    pub fn save(&self, cfg: &AppConfig) -> Result<()> {
        let data = serde_json::to_string_pretty(cfg).context("failed to serialize config")?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, data).context("failed to write config temp")?;
        if self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
        fs::rename(&tmp, &self.path).context("failed to write config")?;
        Ok(())
    }

    pub fn set_api_key(&self, cfg: &mut AppConfig, api_key: &str) -> Result<()> {
        let enc = secret::protect(api_key.as_bytes())?;
        cfg.api_key_encrypted = Some(general_purpose::STANDARD.encode(enc));
        self.save(cfg)?;
        Ok(())
    }

    pub fn get_api_key(&self, cfg: &AppConfig) -> Result<Option<String>> {
        let Some(enc) = &cfg.api_key_encrypted else { return Ok(None) };
        let bytes = general_purpose::STANDARD
            .decode(enc)
            .context("failed to decode api key")?;
        let plain = secret::unprotect(&bytes)?;
        Ok(Some(String::from_utf8_lossy(&plain).to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_config_path() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir()
            .join("voice_asr_client_tests")
            .join(format!("config-{}", nanos));
        let _ = fs::create_dir_all(&dir);
        dir.join("config.json")
    }

    #[test]
    fn config_roundtrip() {
        let path = temp_config_path();
        let store = ConfigStore::from_path(path.clone());
        let cfg = AppConfig::default();
        store.save(&cfg).unwrap();
        let loaded = store.load_or_default().unwrap();
        assert_eq!(cfg, loaded);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn api_key_roundtrip() {
        let path = temp_config_path();
        let store = ConfigStore::from_path(path.clone());
        let mut cfg = AppConfig::default();
        store.save(&cfg).unwrap();
        store.set_api_key(&mut cfg, "test-key-123").unwrap();
        let key = store.get_api_key(&cfg).unwrap().unwrap();
        assert_eq!(key, "test-key-123");
        let _ = fs::remove_file(path);
    }
}
