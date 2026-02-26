use crate::config::RecordMode;
use crate::events::{post_event, AppEvent};
use anyhow::{anyhow, Result};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    CallNextHookEx, SetWindowsHookExW, HC_ACTION, KBDLLHOOKSTRUCT, VK_LCONTROL, VK_LMENU,
    VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

#[derive(Debug, Clone, Copy)]
pub struct Hotkey {
    pub modifiers: Modifiers,
    pub vk: u16,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct KeyState {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    main: bool,
    hotkey_down: bool,
}

static HOTKEY_STATE: OnceLock<Mutex<(Hotkey, RecordMode, KeyState)>> = OnceLock::new();
static EVENT_HWND: OnceLock<HWND> = OnceLock::new();

pub fn install(hwnd: HWND, hotkey: Hotkey, mode: RecordMode) -> Result<()> {
    EVENT_HWND.get_or_init(|| hwnd);
    HOTKEY_STATE.get_or_init(|| Mutex::new((hotkey, mode, KeyState::default())));
    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), HINSTANCE(0), 0);
        if hook.0 == 0 {
            return Err(anyhow!("failed to install keyboard hook"));
        }
    }
    Ok(())
}

pub fn update(hotkey: Hotkey, mode: RecordMode) {
    if let Some(state) = HOTKEY_STATE.get() {
        if let Ok(mut guard) = state.lock() {
            guard.0 = hotkey;
            guard.1 = mode;
        }
    }
}

pub fn parse_hotkey(input: &str) -> Result<Hotkey> {
    let mut mods = Modifiers::default();
    let mut key: Option<u16> = None;

    for part in input.split('+') {
        let token = part.trim().to_lowercase();
        match token.as_str() {
            "ctrl" | "control" => mods.ctrl = true,
            "alt" => mods.alt = true,
            "shift" => mods.shift = true,
            "win" | "meta" | "cmd" => mods.win = true,
            "" => {}
            _ => {
                if key.is_some() {
                    return Err(anyhow!("multiple main keys in hotkey"));
                }
                key = Some(parse_vk(&token)?);
            }
        }
    }

    let Some(vk) = key else {
        return Err(anyhow!("missing main key in hotkey"));
    };

    Ok(Hotkey { modifiers: mods, vk })
}

fn parse_vk(token: &str) -> Result<u16> {
    if token.len() == 1 {
        let ch = token.chars().next().unwrap();
        if ch.is_ascii_alphabetic() {
            return Ok(ch.to_ascii_uppercase() as u16);
        }
        if ch.is_ascii_digit() {
            return Ok(ch as u16);
        }
    }
    if token.starts_with('f') {
        if let Ok(num) = token[1..].parse::<u8>() {
            if (1..=24).contains(&num) {
                return Ok(0x70 + (num as u16 - 1));
            }
        }
    }

    match token {
        "space" => Ok(0x20),
        "tab" => Ok(0x09),
        "enter" | "return" => Ok(0x0D),
        "esc" | "escape" => Ok(0x1B),
        "backspace" => Ok(0x08),
        "insert" => Ok(0x2D),
        "delete" => Ok(0x2E),
        "home" => Ok(0x24),
        "end" => Ok(0x23),
        "pageup" => Ok(0x21),
        "pagedown" => Ok(0x22),
        _ => Err(anyhow!("unsupported key: {}", token)),
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION {
        let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kb.vkCode as u16;
        let is_down = wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN;
        let is_up = wparam.0 as u32 == WM_KEYUP || wparam.0 as u32 == WM_SYSKEYUP;

        if let Some(state) = HOTKEY_STATE.get() {
            if let Ok(mut guard) = state.lock() {
                let (hotkey, mode, ks) = &mut *guard;
                update_key_state(ks, vk, is_down, is_up, hotkey.vk);

                let combo_active = ks.main
                    && (!hotkey.modifiers.ctrl || ks.ctrl)
                    && (!hotkey.modifiers.alt || ks.alt)
                    && (!hotkey.modifiers.shift || ks.shift)
                    && (!hotkey.modifiers.win || ks.win);

                if combo_active && !ks.hotkey_down {
                    ks.hotkey_down = true;
                    if let Some(hwnd) = EVENT_HWND.get() {
                        match mode {
                            RecordMode::Hold => post_event(*hwnd, AppEvent::HotkeyStart),
                            RecordMode::Toggle => post_event(*hwnd, AppEvent::HotkeyToggle),
                        }
                    }
                }

                if ks.hotkey_down && !combo_active {
                    ks.hotkey_down = false;
                    if let Some(hwnd) = EVENT_HWND.get() {
                        if matches!(mode, RecordMode::Hold) {
                            post_event(*hwnd, AppEvent::HotkeyStop);
                        }
                    }
                }
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

fn update_key_state(state: &mut KeyState, vk: u16, is_down: bool, is_up: bool, main_vk: u16) {
    if is_down || is_up {
        match vk {
            VK_LCONTROL.0 | VK_RCONTROL.0 => state.ctrl = is_down,
            VK_LMENU.0 | VK_RMENU.0 => state.alt = is_down,
            VK_LSHIFT.0 | VK_RSHIFT.0 | VK_SHIFT.0 => state.shift = is_down,
            VK_LWIN.0 | VK_RWIN.0 => state.win = is_down,
            _ => {}
        }
        if vk == main_vk {
            state.main = is_down;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_hotkey() {
        let hk = parse_hotkey("Ctrl+Alt+Space").unwrap();
        assert!(hk.modifiers.ctrl);
        assert!(hk.modifiers.alt);
        assert!(!hk.modifiers.shift);
        assert_eq!(hk.vk, 0x20);
    }

    #[test]
    fn parse_function_key() {
        let hk = parse_hotkey("Shift+F5").unwrap();
        assert!(hk.modifiers.shift);
        assert_eq!(hk.vk, 0x70 + 4);
    }

    #[test]
    fn missing_main_key_fails() {
        assert!(parse_hotkey("Ctrl+Alt").is_err());
    }

    #[test]
    fn multiple_main_keys_fail() {
        assert!(parse_hotkey("Ctrl+A+B").is_err());
    }
}
