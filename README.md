# Voice ASR Client (Windows MVP)

Минимальный Windows‑клиент голосового ввода по hotkey с распознаванием через Groq STT (Whisper‑совместимые модели).

## Что делает
- Работает в фоне и управляется через иконку в трее.
- Запись начинается/заканчивается по горячей клавише.
- Аудио отправляется в Groq API, текст вставляется в активное окно.
- Есть базовые настройки и логирование.

## Быстрый старт
1. Установите Rust (`rustup`).
2. Соберите проект:
```powershell
cargo build --release
```
3. Запустите `target\release\voice_asr_client.exe`.

## Настройки
Открываются из трей‑меню **Настройки**.

Поля:
- `Groq API key` — ключ Groq (сохраняется локально в защищённом виде).
- `STT модель` — модель распознавания.
- `Язык` — `auto`, `ru`, `en`.
- `Hotkey` — строка вида `Ctrl+Alt+Space`.
- `Режим записи` — `hold` или `toggle`.
- `Микрофон` — устройство ввода.
- `Timeout (сек)` — таймаут запроса к API.
- `Лимит записи (сек)` — максимальная длина записи.
- `Вставка текста` — `direct`, `clipboard`, `clipboard_only`.
- `Логирование` — `info` или `debug`.
- `Автозапуск с Windows` — включает запись в `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

Если поле API key оставить пустым, сохранится предыдущий ключ.

Поддерживаемые модели Groq STT:
- `whisper-large-v3`
- `whisper-large-v3-turbo`

## Пути данных
- Конфиг: `%AppData%\VoiceASRClient\config.json`
- Логи: `%LocalAppData%\VoiceASRClient\logs\voice-asr-client.log`

## Ограничения MVP
- Нет офлайн‑распознавания и фонового прослушивания.
- Возможны ограничения вставки текста в повышенные (elevated) приложения.

## Безопасность
API‑ключ хранится локально в зашифрованном виде через DPAPI (Windows).

## Тесты
Запуск:
```powershell
cargo test
```

## Инсталлятор (NSIS)
1. Соберите релиз:
```powershell
cargo build --release
```
2. Запустите сборку инсталлятора:
```powershell
.\installer\build-installer.ps1
```
Инсталлятор будет создан в `installer\out`.

## Инсталлятор (MSI, WiX)
Требуется WiX Toolset v3 и Windows Feature `NetFx3` (.NET 3.5).

1. Установите WiX Toolset и включите .NET 3.5:
```powershell
dism /online /enable-feature /featurename:NetFx3 /All /NoRestart
winget install --id WiXToolset.WiXToolset -e --accept-package-agreements --accept-source-agreements
```
2. Соберите MSI:
```powershell
.\installer\build-msi.ps1
```
MSI будет создан в `installer\out`.
