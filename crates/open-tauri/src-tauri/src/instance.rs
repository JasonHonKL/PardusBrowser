use std::process::Child;

pub struct ManagedInstance {
    pub id: String,
    pub port: u16,
    pub process: Child,
    pub ws_url: String,
    pub browser_window_label: Option<String>,
    pub current_url: Option<String>,
    pub agent_status: String,
}

impl std::fmt::Debug for ManagedInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedInstance")
            .field("id", &self.id)
            .field("port", &self.port)
            .field("ws_url", &self.ws_url)
            .field("browser_window_label", &self.browser_window_label)
            .field("current_url", &self.current_url)
            .field("agent_status", &self.agent_status)
            .finish()
    }
}

pub fn find_free_port(base: u16) -> u16 {
    for offset in 0..100u16 {
        let port = base + offset;
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    base
}

pub fn spawn_browser_process(port: u16) -> anyhow::Result<Child> {
    // Try absolute path first (same dir as tauri binary), fall back to PATH
    let bin_path = std::env::current_exe()?
        .parent()
        .map(|p| p.join("open-browser"))
        .filter(|p| p.exists());

    let bin = bin_path
        .as_deref()
        .unwrap_or(std::path::Path::new("open-browser"));

    let child = std::process::Command::new(bin)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    Ok(child)
}

pub async fn wait_for_ready(port: u16, timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    loop {
        if std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            return true;
        }
        if start.elapsed().as_millis() as u64 > timeout_ms {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
