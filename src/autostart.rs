use crate::win::to_wide_null;
use anyhow::{anyhow, Result};
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_SZ,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "VoiceASRClient";

pub fn set_enabled(enabled: bool) -> Result<()> {
    let exe = std::env::current_exe().map_err(|e| anyhow!("current_exe failed: {e}"))?;
    set_enabled_for_path(enabled, &exe)
}

pub fn set_enabled_for_path(enabled: bool, exe_path: &Path) -> Result<()> {
    let key = open_run_key()?;
    if enabled {
        let value = build_run_value(exe_path);
        let mut wide = value.encode_utf16().collect::<Vec<u16>>();
        wide.push(0);
        let bytes = unsafe {
            std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2)
        };
        unsafe {
            let name = to_wide_null(VALUE_NAME);
            RegSetValueExW(
                key,
                PCWSTR(name.as_ptr()),
                0,
                REG_SZ,
                Some(bytes),
            )
            .ok()
            .map_err(|e| anyhow!("RegSetValueExW failed: {e}"))?;
            let _ = RegCloseKey(key);
        }
    } else {
        unsafe {
            let _ = RegDeleteValueW(key, PCWSTR(to_wide_null(VALUE_NAME).as_ptr()));
            let _ = RegCloseKey(key);
        }
    }
    Ok(())
}

pub fn build_run_value(exe_path: &Path) -> String {
    let path = exe_path.to_string_lossy();
    if path.contains(' ') {
        format!("\"{}\"", path)
    } else {
        path.to_string()
    }
}

fn open_run_key() -> Result<HKEY> {
    let key_path = to_wide_null(RUN_KEY);
    unsafe {
        let mut handle = HKEY::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut handle,
        )
        .ok()
        .map_err(|e| anyhow!("RegOpenKeyExW failed: {e}"))?;
        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::build_run_value;
    use std::path::Path;

    #[test]
    fn run_value_quotes_spaces() {
        let p = Path::new("C:\\Program Files\\Voice ASR\\app.exe");
        assert_eq!(
            build_run_value(p),
            "\"C:\\Program Files\\Voice ASR\\app.exe\""
        );
    }

    #[test]
    fn run_value_no_quotes() {
        let p = Path::new("C:\\Apps\\voice_asr_client.exe");
        assert_eq!(build_run_value(p), "C:\\Apps\\voice_asr_client.exe");
    }
}
