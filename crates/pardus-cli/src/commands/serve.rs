use anyhow::Result;
use pardus_core::{App, BrowserConfig};
use pardus_cdp::CdpServer;
use std::sync::Arc;

pub async fn run(host: &str, port: u16, timeout: u64, config: BrowserConfig) -> Result<()> {
    let app = Arc::new(App::new(config));
    let server = CdpServer::new(host.to_string(), port, timeout, app);
    server.run().await?;
    Ok(())
}
