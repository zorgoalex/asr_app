use crate::config::InjectMode;
use anyhow::{anyhow, Result};
use windows::Win32::Foundation::{HGLOBAL, HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_CONTROL, VK_V,
};

pub fn inject_text(text: &str, mode: InjectMode) -> Result<()> {
    if text.trim().is_empty() {
        return Ok(());
    }

    match mode {
        InjectMode::Direct => {
            if send_unicode(text).is_ok() {
                return Ok(());
            }
            clipboard_paste(text)?;
        }
        InjectMode::Clipboard => {
            if send_unicode(text).is_ok() {
                return Ok(());
            }
            clipboard_paste(text)?;
        }
        InjectMode::ClipboardOnly => {
            set_clipboard_text(text)?;
        }
    }
    Ok(())
}

const CF_UNICODETEXT: u32 = 13;

fn send_unicode(text: &str) -> Result<()> {
    let mut inputs: Vec<INPUT> = Vec::with_capacity(text.len() * 2);
    for unit in text.encode_utf16() {
        let down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unit,
                    dwFlags: KEYEVENTF_UNICODE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let up = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unit,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        inputs.push(down);
        inputs.push(up);
    }
    unsafe {
        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if sent != inputs.len() as u32 {
            return Err(anyhow!("SendInput failed"));
        }
    }
    Ok(())
}

fn clipboard_paste(text: &str) -> Result<()> {
    let previous = get_clipboard_text().ok();
    set_clipboard_text(text)?;
    send_ctrl_v()?;
    if let Some(prev) = previous {
        let _ = set_clipboard_text(&prev);
    }
    Ok(())
}

fn send_ctrl_v() -> Result<()> {
    let mut inputs: Vec<INPUT> = Vec::with_capacity(4);
    inputs.push(key_input(VK_CONTROL.0 as u16, false));
    inputs.push(key_input(VK_V.0 as u16, false));
    inputs.push(key_input(VK_V.0 as u16, true));
    inputs.push(key_input(VK_CONTROL.0 as u16, true));
    unsafe {
        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if sent != inputs.len() as u32 {
            return Err(anyhow!("SendInput ctrl+v failed"));
        }
    }
    Ok(())
}

fn key_input(vk: u16, key_up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn set_clipboard_text(text: &str) -> Result<()> {
    unsafe {
        OpenClipboard(HWND(0)).map_err(|e| anyhow!("OpenClipboard failed: {e}"))?;
        let _ = EmptyClipboard();
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let size = wide.len() * 2;
        let hmem: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, size).map_err(|e| anyhow!("GlobalAlloc failed: {e}"))?;
        let ptr = GlobalLock(hmem) as *mut u8;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err(anyhow!("GlobalLock failed"));
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr, size);
        let _ = GlobalUnlock(hmem);
        let handle = HANDLE(hmem.0 as isize);
        SetClipboardData(CF_UNICODETEXT, handle)
            .map_err(|e| anyhow!("SetClipboardData failed: {e}"))?;
        let _ = CloseClipboard();
    }
    Ok(())
}

fn get_clipboard_text() -> Result<String> {
    unsafe {
        OpenClipboard(HWND(0)).map_err(|e| anyhow!("OpenClipboard failed: {e}"))?;
        let handle = GetClipboardData(CF_UNICODETEXT)
            .map_err(|e| anyhow!("GetClipboardData failed: {e}"))?;
        let hglobal = HGLOBAL(handle.0 as *mut std::ffi::c_void);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err(anyhow!("GlobalLock failed"));
        }
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let text = String::from_utf16_lossy(slice);
        let _ = GlobalUnlock(hglobal);
        let _ = CloseClipboard();
        Ok(text)
    }
}
