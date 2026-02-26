use crate::audio::{AudioRecorder, RecordingSession};
use crate::config::{AppConfig, ConfigStore, RecordMode};
use crate::events::{post_event, take_event, AppEvent, WM_APP_EVENT};
use crate::hotkey::parse_hotkey;
use crate::inject::inject_text;
use crate::logger;
use crate::stt;
use crate::tray;
use anyhow::{anyhow, Context, Result};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage, RegisterClassW,
    TranslateMessage, CW_USEDEFAULT, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND, WM_DESTROY,
    WM_NCCREATE,
};
use windows::Win32::UI::WindowsAndMessaging::WNDCLASSW;
use windows::core::PCWSTR;

static APP: OnceLock<Mutex<App>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
enum AppState {
    Idle,
    Recording,
    Transcribing,
    Injecting,
    Error,
}

pub struct App {
    hwnd: HWND,
    cfg: AppConfig,
    store: ConfigStore,
    recorder: Option<AudioRecorder>,
    recording: Option<RecordingSession>,
    state: AppState,
    record_token: u64,
}

pub fn run() -> Result<()> {
    let store = ConfigStore::new()?;
    let cfg = store.load_or_default()?;
    logger::set_level(cfg.log_level);

    let hwnd = create_hidden_window()?;

    tray::init(hwnd)?;

    let hotkey = parse_hotkey(&cfg.hotkey).context("invalid hotkey")?;
    crate::hotkey::install(hwnd, hotkey, cfg.record_mode)?;

    let app = App {
        hwnd,
        cfg,
        store,
        recorder: None,
        recording: None,
        state: AppState::Idle,
        record_token: 0,
    };
    APP.get_or_init(|| Mutex::new(app));

    tray::update_status(hwnd, "Idle");
    message_loop()?;
    tray::destroy(hwnd);
    Ok(())
}

fn message_loop() -> Result<()> {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND(0), 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

fn create_hidden_window() -> Result<HWND> {
    unsafe {
        let class_name = crate::win::to_wide_null("VoiceASRClientHiddenWindow");
        let class_ptr = PCWSTR(class_name.as_ptr());
        let hinstance = GetModuleHandleW(None)?;
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: class_ptr,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(anyhow!("RegisterClassW failed"));
        }
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_ptr,
            class_ptr,
            WINDOW_STYLE(0),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND(0),
            None,
            hinstance,
            None,
        );
        if hwnd.0 == 0 {
            return Err(anyhow!("CreateWindowExW failed"));
        }
        Ok(hwnd)
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            LRESULT(1)
        }
        WM_COMMAND => {
            if tray::handle_command(hwnd, (wparam.0 & 0xffff) as usize) {
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        m if m == tray::WM_TRAYICON => {
            if tray::handle_tray_message(hwnd, lparam) {
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        m if m == WM_APP_EVENT => {
            if lparam.0 != 0 {
                let event = take_event(lparam);
                if let Some(app) = APP.get() {
                    if let Ok(mut guard) = app.lock() {
                        guard.handle_event(*event);
                    }
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

impl App {
    fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::HotkeyStart => self.start_recording(),
            AppEvent::HotkeyStop => self.stop_recording(),
            AppEvent::HotkeyToggle => {
                if matches!(self.state, AppState::Recording) {
                    self.stop_recording();
                } else {
                    self.start_recording();
                }
            }
            AppEvent::AutoStop(token) => {
                if matches!(self.state, AppState::Recording) && self.record_token == token {
                    self.stop_recording();
                }
            }
            AppEvent::TranscriptionDone(result) => self.finish_transcription(result),
            AppEvent::TrayOpenSettings => {
                crate::settings::open(self.hwnd, self.cfg.clone(), self.store.path().to_path_buf());
            }
            AppEvent::TrayExit => unsafe {
                PostQuitMessage(0);
            },
            AppEvent::SettingsUpdated => {
                if let Ok(cfg) = self.store.load_or_default() {
                    self.cfg = cfg;
                    logger::set_level(self.cfg.log_level);
                    if let Ok(hotkey) = parse_hotkey(&self.cfg.hotkey) {
                        crate::hotkey::update(hotkey, self.cfg.record_mode);
                    }
                }
            }
        }
    }

    fn start_recording(&mut self) {
        if !matches!(self.state, AppState::Idle) {
            return;
        }
        let recorder = match AudioRecorder::new(self.cfg.input_device.clone()) {
            Ok(r) => r,
            Err(err) => {
                tray::show_notification(self.hwnd, "Ошибка", "Микрофон недоступен");
                log::error!("audio device error: {}", err);
                return;
            }
        };
        let session = match recorder.start() {
            Ok(s) => s,
            Err(err) => {
                tray::show_notification(self.hwnd, "Ошибка", "Не удалось начать запись");
                log::error!("audio start error: {}", err);
                return;
            }
        };
        self.recorder = Some(recorder);
        self.recording = Some(session);
        self.state = AppState::Recording;
        self.record_token = self.record_token.wrapping_add(1);
        let token = self.record_token;
        let hwnd = self.hwnd;
        let max_secs = self.cfg.max_record_secs;
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(max_secs));
            post_event(hwnd, AppEvent::AutoStop(token));
        });
        tray::update_status(self.hwnd, "Recording");
        tray::show_notification(self.hwnd, "Запись", "Запись началась");
        log::info!("recording started");
    }

    fn stop_recording(&mut self) {
        if !matches!(self.state, AppState::Recording) {
            return;
        }
        let Some(session) = self.recording.take() else { return };
        let buffer = match session.stop() {
            Ok(b) => b,
            Err(err) => {
                tray::show_notification(self.hwnd, "Ошибка", "Не удалось завершить запись");
                log::error!("audio stop error: {}", err);
                self.state = AppState::Idle;
                tray::update_status(self.hwnd, "Idle");
                return;
            }
        };
        self.state = AppState::Transcribing;
        tray::update_status(self.hwnd, "Transcribing");
        tray::show_notification(self.hwnd, "Распознавание", "Идет транскрибация");
        let cfg = self.cfg.clone();
        let hwnd = self.hwnd;
        let api_key = match self.store.get_api_key(&self.cfg) {
            Ok(Some(key)) => key,
            _ => {
                post_event(hwnd, AppEvent::TranscriptionDone(Err(anyhow!("missing api key"))));
                self.state = AppState::Idle;
                tray::update_status(self.hwnd, "Idle");
                return;
            }
        };
        let audio_size = buffer.wav_data.len();
        thread::spawn(move || {
            let result = stt::transcribe(&buffer, &cfg, &api_key);
            post_event(hwnd, AppEvent::TranscriptionDone(result));
        });
        log::info!("recording stopped, bytes={}", audio_size);
    }

    fn finish_transcription(&mut self, result: Result<String>) {
        match result {
            Ok(text) => {
                if text.trim().is_empty() {
                    tray::show_notification(self.hwnd, "Пустой результат", "Текст не распознан");
                } else {
                    self.state = AppState::Injecting;
                    let inject_mode = self.cfg.inject_mode;
                    if let Err(err) = inject_text(&text, inject_mode) {
                        tray::show_notification(self.hwnd, "Ошибка", "Не удалось вставить текст");
                        log::error!("inject error: {}", err);
                    } else {
                        tray::show_notification(self.hwnd, "Готово", "Текст вставлен");
                    }
                }
            }
            Err(err) => {
                let msg = if err.to_string().contains("missing api key") {
                    "Проверьте Groq API key"
                } else {
                    "Ошибка распознавания"
                };
                tray::show_notification(self.hwnd, "Ошибка", msg);
                log::error!("transcription error: {}", err);
            }
        }
        self.state = AppState::Idle;
        tray::update_status(self.hwnd, "Idle");
    }
}
