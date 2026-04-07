use std::sync::Arc;

use anyhow::Result;
use open_cdp::CdpServer;
use open_core::{App, BrowserConfig};

pub async fn run(host: &str, port: u16, timeout: u64, config: BrowserConfig) -> Result<()> {
    let app = Arc::new(App::new(config)?);
    let server = CdpServer::new(host.to_string(), port, timeout, app);
    server.run().await?;
    Ok(())
}
