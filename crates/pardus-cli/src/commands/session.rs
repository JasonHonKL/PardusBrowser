use anyhow::Result;
use std::path::PathBuf;

use crate::SessionAction;

pub fn run(action: SessionAction) -> Result<()> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("pardus-browser");

    match action {
        SessionAction::List => {
            let sessions = pardus_core::SessionStore::list_sessions(&cache_dir)?;
            if sessions.is_empty() {
                println!("No sessions found.");
            } else {
                println!("Sessions:");
                for name in &sessions {
                    let store = pardus_core::SessionStore::load(name, &cache_dir)?;
                    println!(
                        "  {}  ({} cookies, {} headers, {} localStorage origins)",
                        name,
                        store.cookie_count(),
                        store.header_count(),
                        store.local_storage_origins().len(),
                    );
                }
            }
        }
        SessionAction::Info { name } => {
            let session_name = name.as_deref().unwrap_or("default");
            let store = match pardus_core::SessionStore::load(session_name, &cache_dir) {
                Ok(s) => s,
                Err(_) => {
                    println!("Session '{}' not found.", session_name);
                    return Ok(());
                }
            };
            println!("Session: {}", store.session_name());
            println!("  Path:   {}", store.session_dir().display());
            println!("  Cookies: {}", store.cookie_count());
            println!("  Headers: {}", store.header_count());

            let origins = store.local_storage_origins();
            if origins.is_empty() {
                println!("  localStorage: (empty)");
            } else {
                println!("  localStorage:");
                for origin in &origins {
                    let keys = store.local_storage_keys(origin);
                    println!("    {}  ({} keys)", origin, keys.len());
                }
            }
        }
        SessionAction::Destroy { name } => {
            let sessions = pardus_core::SessionStore::list_sessions(&cache_dir)?;
            if !sessions.contains(&name) {
                println!("Session '{}' not found.", name);
                return Ok(());
            }
            pardus_core::SessionStore::destroy(&name, &cache_dir)?;
            println!("Session '{}' deleted.", name);
        }
        SessionAction::ExportCookies { name } => {
            let session_name = name.as_deref().unwrap_or("default");
            let store = match pardus_core::SessionStore::load(session_name, &cache_dir) {
                Ok(s) => s,
                Err(_) => {
                    println!("Session '{}' not found.", session_name);
                    return Ok(());
                }
            };

            let cookies_path = store.session_dir().join("cookies.json");
            if !cookies_path.exists() {
                println!("No cookies found in session '{}'.", session_name);
                return Ok(());
            }

            let data = std::fs::read_to_string(&cookies_path)?;
            println!("{}", data);
        }
    }

    Ok(())
}
