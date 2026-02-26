use crate::audio::list_input_devices;
use crate::config::{AppConfig, ConfigStore, InjectMode, LogLevel, RecordMode};
use crate::events::{post_event, AppEvent};
use crate::hotkey::parse_hotkey;
use crate::win::to_wide_null;
use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::sync::OnceLock;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, DEFAULT_GUI_FONT, HFONT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowTextLengthW, GetWindowTextW,
    LoadCursorW, MessageBoxW, RegisterClassW, SendMessageW, SetWindowLongPtrW, SetWindowTextW,
    ShowWindow, CB_ADDSTRING, CB_GETCOUNT, CB_GETCURSEL, CB_GETLBTEXT, CB_SETCURSEL,
    CW_USEDEFAULT, ES_LEFT, ES_PASSWORD, GWLP_USERDATA, HMENU, MB_ICONERROR, MB_OK, SW_SHOW,
    WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_NCCREATE, WM_SETFONT, WNDCLASSW, WS_BORDER, WS_CHILD,
    WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_WINDOWEDGE, WS_OVERLAPPEDWINDOW, WS_TABSTOP,
    WS_VISIBLE, WS_VSCROLL, BS_PUSHBUTTON, CBS_DROPDOWNLIST,
};

static SETTINGS_HWND: OnceLock<std::sync::Mutex<Option<HWND>>> = OnceLock::new();

const ID_SAVE: usize = 2001;
const ID_CANCEL: usize = 2002;
const ID_API_KEY: usize = 2100;
const ID_MODEL: usize = 2101;
const ID_LANG: usize = 2102;
const ID_HOTKEY: usize = 2103;
const ID_MODE: usize = 2104;
const ID_MIC: usize = 2105;
const ID_TIMEOUT: usize = 2106;
const ID_MAXREC: usize = 2107;
const ID_INJECT: usize = 2108;
const ID_LOGLEVEL: usize = 2109;

struct SettingsState {
    cfg: AppConfig,
    store: ConfigStore,
    parent: HWND,
    device_names: Vec<String>,
    font: HFONT,
    api_key_edit: HWND,
    model_combo: HWND,
    lang_combo: HWND,
    hotkey_edit: HWND,
    mode_combo: HWND,
    mic_combo: HWND,
    timeout_edit: HWND,
    maxrec_edit: HWND,
    inject_combo: HWND,
    loglevel_combo: HWND,
}

pub fn open(parent: HWND, cfg: AppConfig, config_path: PathBuf) {
    let guard = SETTINGS_HWND
        .get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(current) = guard.lock() {
        if let Some(hwnd) = *current {
            unsafe { let _ = ShowWindow(hwnd, SW_SHOW); };
            return;
        }
    }

    let device_names = list_input_devices().unwrap_or_default();
    let store = ConfigStore::from_path(config_path);

    let hwnd = match create_window(parent) {
        Ok(h) => h,
        Err(err) => {
            show_error(parent, &format!("Не удалось открыть настройки: {}", err));
            return;
        }
    };

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
    }

    let font = unsafe { HFONT(GetStockObject(DEFAULT_GUI_FONT).0) };
    let mut state = SettingsState {
        cfg,
        store,
        parent,
        device_names,
        font,
        api_key_edit: HWND(0),
        model_combo: HWND(0),
        lang_combo: HWND(0),
        hotkey_edit: HWND(0),
        mode_combo: HWND(0),
        mic_combo: HWND(0),
        timeout_edit: HWND(0),
        maxrec_edit: HWND(0),
        inject_combo: HWND(0),
        loglevel_combo: HWND(0),
    };

    build_ui(hwnd, &mut state);
    if let Ok(mut current) = guard.lock() {
        *current = Some(hwnd);
    }
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(Box::new(state)) as isize);
    }
}

fn create_window(parent: HWND) -> Result<HWND> {
    unsafe {
        let class_name = to_wide_null("VoiceASRSettings");
        let class_ptr = PCWSTR(class_name.as_ptr());
        let title = to_wide_null("Настройки Voice ASR");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(settings_wndproc),
            hCursor: LoadCursorW(None, windows::Win32::UI::WindowsAndMessaging::IDC_ARROW)?,
            lpszClassName: class_ptr,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE,
            class_ptr,
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            520,
            parent,
            None,
            None,
            None,
        );
        if hwnd.0 == 0 {
            return Err(anyhow!("CreateWindowExW failed"));
        }
        Ok(hwnd)
    }
}

unsafe extern "system" fn settings_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            LRESULT(1)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as usize;
            if let Some(state) = get_state(hwnd) {
                if id == ID_SAVE {
                    if let Err(err) = unsafe { save_settings(hwnd, &mut *state) } {
                        show_error(hwnd, &format!("{}", err));
                    }
                    return LRESULT(0);
                }
                if id == ID_CANCEL {
                    let _ = DestroyWindow(hwnd);
                    return LRESULT(0);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            if let Some(state) = get_state(hwnd) {
                let _ = Box::from_raw(state as *mut SettingsState);
            }
            if let Some(lock) = SETTINGS_HWND.get() {
                if let Ok(mut current) = lock.lock() {
                    *current = None;
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn build_ui(hwnd: HWND, state: &mut SettingsState) {
    let mut y = 16;
    let label_w = 160;
    let field_w = 300;
    let h = 22;
    let gap = 8;

    state.api_key_edit = add_edit(hwnd, "Groq API key", ID_API_KEY, y, true, label_w, field_w, h, state.font);
    y += h + gap;
    state.model_combo = add_combo(hwnd, "STT модель", ID_MODEL, y, label_w, field_w, h, state.font);
    y += h + gap;
    state.lang_combo = add_combo(hwnd, "Язык", ID_LANG, y, label_w, field_w, h, state.font);
    y += h + gap;
    state.hotkey_edit = add_edit(hwnd, "Hotkey", ID_HOTKEY, y, false, label_w, field_w, h, state.font);
    y += h + gap;
    state.mode_combo = add_combo(hwnd, "Режим записи", ID_MODE, y, label_w, field_w, h, state.font);
    y += h + gap;
    state.mic_combo = add_combo(hwnd, "Микрофон", ID_MIC, y, label_w, field_w, h, state.font);
    y += h + gap;
    state.timeout_edit = add_edit(hwnd, "Timeout (сек)", ID_TIMEOUT, y, false, label_w, field_w, h, state.font);
    y += h + gap;
    state.maxrec_edit = add_edit(hwnd, "Лимит записи (сек)", ID_MAXREC, y, false, label_w, field_w, h, state.font);
    y += h + gap;
    state.inject_combo = add_combo(hwnd, "Вставка текста", ID_INJECT, y, label_w, field_w, h, state.font);
    y += h + gap;
    state.loglevel_combo = add_combo(hwnd, "Логирование", ID_LOGLEVEL, y, label_w, field_w, h, state.font);
    y += h + gap + 10;

    add_button(hwnd, "Сохранить", ID_SAVE, 160, y, 120, 28, state.font);
    add_button(hwnd, "Отмена", ID_CANCEL, 300, y, 120, 28, state.font);

    populate_controls(state);
}

fn populate_controls(state: &SettingsState) {
    set_combo_items(state.model_combo, &["whisper-large-v3", "whisper-large-v3-turbo"]);
    set_combo_items(state.lang_combo, &["auto", "ru", "en"]);
    set_combo_items(state.mode_combo, &["hold", "toggle"]);
    set_combo_items(state.inject_combo, &["direct", "clipboard", "clipboard_only"]);
    set_combo_items(state.loglevel_combo, &["info", "debug"]);

    combo_add(state.mic_combo, "default");
    for dev in &state.device_names {
        combo_add(state.mic_combo, dev);
    }

    set_combo_value(state.model_combo, &state.cfg.stt_model);
    set_combo_value(state.lang_combo, &state.cfg.language);
    set_combo_value(
        state.mode_combo,
        match state.cfg.record_mode {
            RecordMode::Hold => "hold",
            RecordMode::Toggle => "toggle",
        },
    );
    set_combo_value(
        state.inject_combo,
        match state.cfg.inject_mode {
            InjectMode::Direct => "direct",
            InjectMode::Clipboard => "clipboard",
            InjectMode::ClipboardOnly => "clipboard_only",
        },
    );
    set_combo_value(
        state.loglevel_combo,
        match state.cfg.log_level {
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
        },
    );
    if let Some(dev) = &state.cfg.input_device {
        set_combo_value(state.mic_combo, dev);
    } else {
        set_combo_value(state.mic_combo, "default");
    }

    set_edit_text(state.hotkey_edit, &state.cfg.hotkey);
    set_edit_text(state.timeout_edit, &state.cfg.timeout_secs.to_string());
    set_edit_text(state.maxrec_edit, &state.cfg.max_record_secs.to_string());
}

fn save_settings(hwnd: HWND, state: &mut SettingsState) -> Result<()> {
    let hotkey = get_edit_text(state.hotkey_edit);
    parse_hotkey(&hotkey).context("Некорректный hotkey")?;

    let model = get_combo_text(state.model_combo);
    let lang = get_combo_text(state.lang_combo);
    let mode = match get_combo_text(state.mode_combo).as_str() {
        "hold" => RecordMode::Hold,
        "toggle" => RecordMode::Toggle,
        _ => RecordMode::Hold,
    };
    let inject = match get_combo_text(state.inject_combo).as_str() {
        "clipboard" => InjectMode::Clipboard,
        "clipboard_only" => InjectMode::ClipboardOnly,
        _ => InjectMode::Direct,
    };
    let log_level = match get_combo_text(state.loglevel_combo).as_str() {
        "debug" => LogLevel::Debug,
        _ => LogLevel::Info,
    };
    let mic = get_combo_text(state.mic_combo);
    let mic = if mic.is_empty() || mic == "default" {
        None
    } else {
        Some(mic)
    };

    let timeout: u64 = get_edit_text(state.timeout_edit)
        .parse()
        .context("Некорректный timeout")?;
    let maxrec: u64 = get_edit_text(state.maxrec_edit)
        .parse()
        .context("Некорректный лимит записи")?;
    if timeout == 0 {
        return Err(anyhow!("Timeout должен быть больше 0"));
    }
    if maxrec == 0 {
        return Err(anyhow!("Лимит записи должен быть больше 0"));
    }

    let mut cfg = state.cfg.clone();
    cfg.hotkey = hotkey;
    cfg.stt_model = model;
    cfg.language = lang;
    cfg.record_mode = mode;
    cfg.inject_mode = inject;
    cfg.log_level = log_level;
    cfg.input_device = mic;
    cfg.timeout_secs = timeout;
    cfg.max_record_secs = maxrec;

    let api_key = get_edit_text(state.api_key_edit);
    if !api_key.trim().is_empty() {
        state.store.set_api_key(&mut cfg, &api_key)?;
    } else {
        state.store.save(&cfg)?;
    }

    state.cfg = cfg;
    post_event(state.parent, AppEvent::SettingsUpdated);
    unsafe { let _ = DestroyWindow(hwnd); };
    Ok(())
}

fn add_label(hwnd: HWND, text: &str, x: i32, y: i32, w: i32, h: i32, font: HFONT) {
    unsafe {
        let label_text = to_wide_null(text);
        let label = CreateWindowExW(
            WS_EX_WINDOWEDGE,
            PCWSTR(to_wide_null("STATIC").as_ptr()),
            PCWSTR(label_text.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            hwnd,
            HMENU(0),
            None,
            None,
        );
        SendMessageW(label, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }
}

fn add_edit(
    hwnd: HWND,
    label: &str,
    id: usize,
    y: i32,
    password: bool,
    label_w: i32,
    field_w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    add_label(hwnd, label, 16, y, label_w, h, font);
    let mut style = windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
        WS_CHILD.0 | WS_VISIBLE.0 | WS_BORDER.0 | WS_TABSTOP.0 | ES_LEFT as u32,
    );
    if password {
        style.0 |= ES_PASSWORD as u32;
    }
    unsafe {
        let edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            PCWSTR(to_wide_null("EDIT").as_ptr()),
            PCWSTR(to_wide_null("").as_ptr()),
            style,
            16 + label_w,
            y,
            field_w,
            h,
            hwnd,
            HMENU(id as isize),
            None,
            None,
        );
        SendMessageW(edit, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        edit
    }
}

fn add_combo(
    hwnd: HWND,
    label: &str,
    id: usize,
    y: i32,
    label_w: i32,
    field_w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    add_label(hwnd, label, 16, y, label_w, h, font);
    unsafe {
        let combo = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            PCWSTR(to_wide_null("COMBOBOX").as_ptr()),
            PCWSTR(to_wide_null("").as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | WS_VSCROLL.0 | CBS_DROPDOWNLIST as u32,
            ),
            16 + label_w,
            y,
            field_w,
            h + 200,
            hwnd,
            HMENU(id as isize),
            None,
            None,
        );
        SendMessageW(combo, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        combo
    }
}

fn add_button(hwnd: HWND, text: &str, id: usize, x: i32, y: i32, w: i32, h: i32, font: HFONT) {
    unsafe {
        let btn = CreateWindowExW(
            WS_EX_WINDOWEDGE,
            PCWSTR(to_wide_null("BUTTON").as_ptr()),
            PCWSTR(to_wide_null(text).as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_PUSHBUTTON as u32,
            ),
            x,
            y,
            w,
            h,
            hwnd,
            HMENU(id as isize),
            None,
            None,
        );
        SendMessageW(btn, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }
}

fn combo_add(combo: HWND, text: &str) {
    unsafe {
        let wide = to_wide_null(text);
        SendMessageW(combo, CB_ADDSTRING, WPARAM(0), LPARAM(wide.as_ptr() as isize));
    }
}

fn set_combo_items(combo: HWND, items: &[&str]) {
    for item in items {
        combo_add(combo, item);
    }
}

fn set_combo_value(combo: HWND, value: &str) {
    unsafe {
        let count = SendMessageW(combo, CB_GETCOUNT, WPARAM(0), LPARAM(0)).0 as i32;
        for idx in 0..count {
            let mut buf = vec![0u16; 256];
            let len = SendMessageW(
                combo,
                CB_GETLBTEXT,
                WPARAM(idx as usize),
                LPARAM(buf.as_mut_ptr() as isize),
            )
            .0;
            if len > 0 {
                let item = String::from_utf16_lossy(&buf[..len as usize]);
                if item == value {
                    let _ = SendMessageW(combo, CB_SETCURSEL, WPARAM(idx as usize), LPARAM(0));
                    return;
                }
            }
        }
        if count > 0 {
            let _ = SendMessageW(combo, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        }
    }
}

fn get_combo_text(combo: HWND) -> String {
    unsafe {
        let idx = SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
        if idx < 0 {
            return String::new();
        }
        let mut buf = vec![0u16; 256];
        let len = SendMessageW(combo, CB_GETLBTEXT, WPARAM(idx as usize), LPARAM(buf.as_mut_ptr() as isize)).0;
        if len <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

fn set_edit_text(hwnd: HWND, text: &str) {
    unsafe {
        let wide = to_wide_null(text);
        let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_ptr()));
    }
}

fn get_edit_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let read = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..read as usize])
    }
}

fn get_state(hwnd: HWND) -> Option<*mut SettingsState> {
    unsafe {
        let ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if ptr == 0 {
            None
        } else {
            Some(ptr as *mut SettingsState)
        }
    }
}

fn show_error(parent: HWND, msg: &str) {
    unsafe {
        let msg_w = to_wide_null(msg);
        let title_w = to_wide_null("Ошибка");
        MessageBoxW(parent, PCWSTR(msg_w.as_ptr()), PCWSTR(title_w.as_ptr()), MB_OK | MB_ICONERROR);
    }
}
