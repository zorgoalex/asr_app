use anyhow::Result;
use anyhow::Result;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

#[derive(Debug)]
pub enum AppEvent {
    HotkeyStart,
    HotkeyStop,
    HotkeyToggle,
    AutoStop(u64),
    TranscriptionDone(Result<String>),
    TrayOpenSettings,
    TrayExit,
    SettingsUpdated,
}

pub const WM_APP_EVENT: u32 = WM_APP + 10;

pub fn post_event(hwnd: HWND, event: AppEvent) {
    let boxed = Box::new(event);
    let ptr = Box::into_raw(boxed) as isize;
    unsafe {
        let _ = PostMessageW(hwnd, WM_APP_EVENT, WPARAM(0), LPARAM(ptr));
    }
}

pub unsafe fn take_event(lparam: LPARAM) -> Box<AppEvent> {
    Box::from_raw(lparam.0 as *mut AppEvent)
}
