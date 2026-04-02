use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::app::App;
use crate::page::Page;
use super::actions::InteractionResult;

pub async fn wait_for_selector(
    app: &Arc<App>,
    page: &Page,
    selector: &str,
    timeout_ms: u32,
    interval_ms: u32,
) -> anyhow::Result<InteractionResult> {
    if page.has_selector(selector) {
        return Ok(InteractionResult::WaitSatisfied {
            selector: selector.to_string(),
            found: true,
        });
    }

    let timeout = Duration::from_millis(timeout_ms as u64);
    let interval = Duration::from_millis(interval_ms as u64);
    let start = Instant::now();

    while start.elapsed() < timeout {
        tokio::time::sleep(interval).await;

        match Page::from_url(app, &page.url).await {
            Ok(new_page) => {
                if new_page.has_selector(selector) {
                    return Ok(InteractionResult::WaitSatisfied {
                        selector: selector.to_string(),
                        found: true,
                    });
                }
            }
            Err(_) => continue,
        }
    }

    Ok(InteractionResult::WaitSatisfied {
        selector: selector.to_string(),
        found: false,
    })
}

pub async fn wait_for_selector_with_js(
    app: &Arc<App>,
    page: &Page,
    selector: &str,
    timeout_ms: u32,
    interval_ms: u32,
    js_wait_ms: u32,
) -> anyhow::Result<InteractionResult> {
    if page.has_selector(selector) {
        return Ok(InteractionResult::WaitSatisfied {
            selector: selector.to_string(),
            found: true,
        });
    }

    let timeout = Duration::from_millis(timeout_ms as u64);
    let interval = Duration::from_millis(interval_ms as u64);
    let start = Instant::now();

    while start.elapsed() < timeout {
        tokio::time::sleep(interval).await;

        match Page::from_url_with_js(app, &page.url, js_wait_ms).await {
            Ok(new_page) => {
                if new_page.has_selector(selector) {
                    return Ok(InteractionResult::WaitSatisfied {
                        selector: selector.to_string(),
                        found: true,
                    });
                }
            }
            Err(_) => continue,
        }
    }

    Ok(InteractionResult::WaitSatisfied {
        selector: selector.to_string(),
        found: false,
    })
}
