use anyhow::Result;
use voice_asr_client::{app, logger};

fn main() -> Result<()> {
    logger::init()?;
    app::run()
}
