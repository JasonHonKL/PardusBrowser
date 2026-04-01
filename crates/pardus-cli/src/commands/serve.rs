use anyhow::Result;
use pardus_core::App;
use pardus_cdp::CdpServer;

pub async fn run(host: &str, port: u16, timeout: u64) -> Result<()> {
    let app = App::default();
    let server = CdpServer::new(host.to_string(), port, timeout, app);
    server.run().await?;
    Ok(())
}
