use anyhow::Result;
use log::info;

fn main() -> Result<()> {
    env_logger::init();
    info!("rele-tui starting");

    Ok(())
}
