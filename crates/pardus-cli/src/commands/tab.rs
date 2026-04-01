//! Tab management commands for pardus-cli

use anyhow::Result;

use crate::OutputFormatArg;

/// Run the tab list command
pub async fn list(manager: &pardus_core::TabManager, format: OutputFormatArg) -> Result<()> {
    match format {
        OutputFormatArg::Json => {
            let tabs: Vec<_> = manager.list().iter().map(|t| t.info()).collect();
            let summary = manager.summary();
            let output = serde_json::json!({
                "summary": summary,
                "tabs": tabs,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        _ => {
            let tabs = manager.list();
            if tabs.is_empty() {
                println!("No tabs open");
                return Ok(());
            }

            println!("Tabs ({} total):", tabs.len());
            println!("{:<6} {:<10} {:<40} {}", "ID", "STATE", "URL", "TITLE");
            println!("{}", "-".repeat(100));

            for tab in tabs {
                let active_marker = if manager.active().map(|a| a.id) == Some(tab.id) {
                    "*"
                } else {
                    " "
                };
                let title = tab.title.as_deref().unwrap_or("(no title)");
                let title_truncated = if title.len() > 30 {
                    format!("{}...", &title[..27])
                } else {
                    title.to_string()
                };
                let url_truncated = if tab.url.len() > 38 {
                    format!("{}...", &tab.url[..35])
                } else {
                    tab.url.clone()
                };
                println!(
                    "{:<5}{} {:<10} {:<40} {}",
                    tab.id.to_string(),
                    active_marker,
                    format!("{:?}", tab.state),
                    url_truncated,
                    title_truncated
                );
            }
        }
    }
    Ok(())
}

/// Run tab open command - create and activate new tab
pub async fn open(
    manager: &mut pardus_core::TabManager,
    url: &str,
    js: bool,
    stealth: bool,
    proxy: Option<&str>,
) -> Result<()> {
    use pardus_core::TabConfig;

    let config = TabConfig {
        js_enabled: js,
        stealth,
        ..Default::default()
    };

    let id = if js || stealth {
        manager.create_tab_with_config(url, config)
    } else {
        manager.create_tab(url)
    };

    println!("Created tab {}", id);
    manager.switch_to(id).await?;
    println!("Loaded: {}", url);
    Ok(())
}

/// Run tab close command
pub fn close(manager: &mut pardus_core::TabManager, id: u64) -> Result<()> {
    use pardus_core::TabId;

    let tab_id = TabId::from_u64(id);
    match manager.close_tab(tab_id) {
        Ok(was_active) => {
            if was_active {
                println!("Closed tab {} (was active)", id);
            } else {
                println!("Closed tab {}", id);
            }
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("Failed to close tab: {}", e);
        }
    }
}

/// Close all tabs
pub fn close_all(manager: &mut pardus_core::TabManager) -> Result<()> {
    let count = manager.len();
    manager.close_all();
    println!("Closed {} tab(s)", count);
    Ok(())
}

/// Close all tabs except the active one
pub fn close_others(manager: &mut pardus_core::TabManager) -> Result<()> {
    let before_count = manager.len();
    manager.close_others();
    let after_count = manager.len();
    println!("Closed {} tab(s), kept 1 active", before_count - after_count);
    Ok(())
}

/// Switch to a tab by ID
pub async fn switch(
    manager: &mut pardus_core::TabManager,
    id: u64,
) -> Result<()> {
    use pardus_core::TabId;

    let tab_id = TabId::from_u64(id);
    let tab = manager.switch_to(tab_id).await?;
    println!("Switched to tab {}: {}", id, tab.url);
    Ok(())
}

/// Navigate the active tab to a new URL
pub async fn navigate(
    manager: &mut pardus_core::TabManager,
    url: &str,
) -> Result<()> {
    let tab = manager.navigate_active(url).await?;
    println!("Navigated to: {}", tab.url);
    Ok(())
}

/// Reload the active tab
pub async fn reload(manager: &mut pardus_core::TabManager) -> Result<()> {
    let tab = manager.reload_active().await?;
    println!("Reloaded: {}", tab.url);
    Ok(())
}

/// Go back in active tab history
pub async fn go_back(manager: &mut pardus_core::TabManager) -> Result<()> {
    match manager.go_back().await {
        Ok(Some(tab)) => {
            println!("Went back to: {}", tab.url);
            Ok(())
        }
        Ok(None) => {
            println!("No previous page in history");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Go forward in active tab history
pub async fn go_forward(manager: &mut pardus_core::TabManager) -> Result<()> {
    match manager.go_forward().await {
        Ok(Some(tab)) => {
            println!("Went forward to: {}", tab.url);
            Ok(())
        }
        Ok(None) => {
            println!("No next page in history");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Show active tab info
pub fn info(manager: &pardus_core::TabManager, format: OutputFormatArg) -> Result<()> {
    match manager.active() {
        Some(tab) => {
            match format {
                OutputFormatArg::Json => {
                    let info = tab.info();
                    println!("{}", serde_json::to_string_pretty(&info)?);
                }
                _ => {
                    println!("Active Tab {}:", tab.id);
                    println!("  URL: {}", tab.url);
                    println!("  Title: {}", tab.title.as_deref().unwrap_or("(none)"));
                    println!("  State: {:?}", tab.state);
                    println!("  History: {}/{}", tab.history_index + 1, tab.history.len());
                    println!("  Can go back: {}", tab.can_go_back());
                    println!("  Can go forward: {}", tab.can_go_forward());
                }
            }
            Ok(())
        }
        None => {
            anyhow::bail!("No active tab");
        }
    }
}
