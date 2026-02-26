use crate::config::LogLevel;
use anyhow::{Context, Result};
use chrono::Local;
use directories::ProjectDirs;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
const LOG_FILE_NAME: &str = "voice-asr-client.log";

static LOGGER: OnceLock<FileLogger> = OnceLock::new();

pub fn init() -> Result<()> {
    let logger = FileLogger::new(MAX_LOG_BYTES)?;
    log::set_max_level(LevelFilter::Info);
    log::set_logger(LOGGER.get_or_init(|| logger))
        .map_err(|e| anyhow::anyhow!("failed to set logger: {e:?}"))?;
    Ok(())
}

pub fn set_level(level: LogLevel) {
    let filter = match level {
        LogLevel::Info => LevelFilter::Info,
        LogLevel::Debug => LevelFilter::Debug,
    };
    log::set_max_level(filter);
}

struct FileLogger {
    file: Mutex<File>,
    path: PathBuf,
    max_size: u64,
}

impl FileLogger {
    fn new(max_size: u64) -> Result<Self> {
        let dir = log_dir()?;
        fs::create_dir_all(&dir).context("failed to create log dir")?;
        let path = dir.join(LOG_FILE_NAME);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .context("failed to open log file")?;
        Ok(Self {
            file: Mutex::new(file),
            path,
            max_size,
        })
    }

    fn rotate_if_needed(&self) -> Result<()> {
        let len = fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        if len < self.max_size {
            return Ok(());
        }
        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let rotated = self
            .path
            .with_file_name(format!("voice-asr-client-{}.log", timestamp));

        let placeholder = File::create("NUL").context("failed to open placeholder")?;
        {
            let mut guard = self.file.lock().unwrap();
            let _old = std::mem::replace(&mut *guard, placeholder);
        }

        fs::rename(&self.path, rotated).context("failed to rotate log file")?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .context("failed to reopen log file")?;
        let mut guard = self.file.lock().unwrap();
        let _old = std::mem::replace(&mut *guard, file);
        Ok(())
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let max = log::max_level().to_level().unwrap_or(Level::Error);
        metadata.level() <= max
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if self.rotate_if_needed().is_err() {
            return;
        }
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        let line = format!(
            "{} [{:>5}] {} - {}\n",
            ts,
            record.level(),
            record.target(),
            record.args()
        );
        if let Ok(mut guard) = self.file.lock() {
            let _ = guard.write_all(line.as_bytes());
            let _ = guard.flush();
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.file.lock() {
            let _ = guard.flush();
        }
    }
}

fn log_dir() -> Result<PathBuf> {
    let proj_dirs =
        ProjectDirs::from("com", "zorgoalex", "VoiceASRClient").context("log dir error")?;
    Ok(proj_dirs.data_local_dir().join("logs"))
}
