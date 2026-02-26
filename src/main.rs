mod app;
mod autostart;
mod audio;
mod config;
mod events;
mod hotkey;
mod inject;
mod logger;
mod secret;
mod settings;
mod stt;
mod tray;
mod win;

use anyhow::Result;

fn main() -> Result<()> {
    logger::init()?;
    app::run()
}
