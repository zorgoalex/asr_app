use crate::events::{post_event, AppEvent};
use crate::win::to_wide_null;
use anyhow::{anyhow, Result};
use std::sync::OnceLock;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, GetCursorPos, LoadIconW, ModifyMenuW, SetForegroundWindow,
    TrackPopupMenu, HMENU, MF_BYCOMMAND, MF_GRAYED, MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    TPM_RIGHTBUTTON, WM_LBUTTONUP, WM_RBUTTONUP,
};

pub const WM_TRAYICON: u32 = 0x8001;
const TRAY_ID: u32 = 1;
const ID_STATUS: usize = 1000;
const ID_SETTINGS: usize = 1001;
const ID_CHECK: usize = 1003;
const ID_EXIT: usize = 1002;

static TRAY_MENU: OnceLock<HMENU> = OnceLock::new();

pub fn init(hwnd: HWND) -> Result<()> {
    create_menu()?;
    add_tray_icon(hwnd)?;
    Ok(())
}

pub fn destroy(hwnd: HWND) {
    unsafe {
        if let Ok(mut nid) = base_nid(hwnd) {
            let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
        }
    }
}

pub fn update_status(hwnd: HWND, status: &str) {
    unsafe {
        let Ok(mut nid) = base_nid(hwnd) else { return };
        let tip = to_wide_null(&format!("Voice ASR Client - {}", status));
        let len = tip.len().min(nid.szTip.len());
        nid.szTip[..len].copy_from_slice(&tip[..len]);
        nid.uFlags = NIF_TIP;
        let _ = Shell_NotifyIconW(NIM_MODIFY, &mut nid);
    }
    update_menu_status(status);
}

pub fn show_notification(hwnd: HWND, title: &str, message: &str) {
    unsafe {
        let Ok(mut nid) = base_nid(hwnd) else { return };
        let title_w = to_wide_null(title);
        let msg_w = to_wide_null(message);
        nid.uFlags = NIF_INFO;
        let tlen = title_w.len().min(nid.szInfoTitle.len());
        let mlen = msg_w.len().min(nid.szInfo.len());
        nid.szInfoTitle[..tlen].copy_from_slice(&title_w[..tlen]);
        nid.szInfo[..mlen].copy_from_slice(&msg_w[..mlen]);
        let _ = Shell_NotifyIconW(NIM_MODIFY, &mut nid);
    }
}

pub fn handle_tray_message(hwnd: HWND, lparam: LPARAM) -> bool {
    let msg = lparam.0 as u32;
    if msg == WM_RBUTTONUP {
        show_menu(hwnd);
        return true;
    }
    if msg == WM_LBUTTONUP {
        post_event(hwnd, AppEvent::TrayOpenSettings);
        return true;
    }
    false
}

pub fn handle_command(hwnd: HWND, wparam: usize) -> bool {
    match wparam {
        ID_STATUS => true,
        ID_SETTINGS => {
            post_event(hwnd, AppEvent::TrayOpenSettings);
            true
        }
        ID_CHECK => {
            post_event(hwnd, AppEvent::TrayCheckConnection);
            true
        }
        ID_EXIT => {
            post_event(hwnd, AppEvent::TrayExit);
            true
        }
        _ => false,
    }
}

fn create_menu() -> Result<()> {
    let menu = unsafe { CreatePopupMenu()? };
    unsafe {
        let status = to_wide_null("Статус: Idle");
        let check = to_wide_null("Проверка соединения");
        let settings = to_wide_null("Настройки");
        let exit = to_wide_null("Выход");
        let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, ID_STATUS, PCWSTR(status.as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, ID_CHECK, PCWSTR(check.as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, ID_SETTINGS, PCWSTR(settings.as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, PCWSTR(exit.as_ptr()));
    }
    TRAY_MENU.get_or_init(|| menu);
    Ok(())
}

fn show_menu(hwnd: HWND) {
    let Some(menu) = TRAY_MENU.get() else { return };
    unsafe {
        let mut point = Default::default();
        if GetCursorPos(&mut point).is_err() {
            return;
        }
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            *menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            None,
        );
    }
}

fn update_menu_status(status: &str) {
    let Some(menu) = TRAY_MENU.get() else { return };
    unsafe {
        let text = to_wide_null(&format!("Статус: {}", status));
        let _ = ModifyMenuW(
            *menu,
            ID_STATUS as u32,
            MF_BYCOMMAND | MF_STRING | MF_GRAYED,
            ID_STATUS,
            PCWSTR(text.as_ptr()),
        );
    }
}

fn add_tray_icon(hwnd: HWND) -> Result<()> {
    unsafe {
        let mut nid = base_nid(hwnd)?;
        let tip = to_wide_null("Voice ASR Client - Idle");
        nid.szTip[..tip.len()].copy_from_slice(&tip);
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        let ok = Shell_NotifyIconW(NIM_ADD, &mut nid).as_bool();
        if !ok {
            return Err(anyhow!("Shell_NotifyIconW NIM_ADD failed"));
        }
        nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let _ = Shell_NotifyIconW(NIM_MODIFY, &mut nid);
    }
    Ok(())
}

fn base_nid(hwnd: HWND) -> Result<NOTIFYICONDATAW> {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ID;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = unsafe {
        LoadIconW(None, windows::Win32::UI::WindowsAndMessaging::IDI_APPLICATION)?
    };
    Ok(nid)
}
