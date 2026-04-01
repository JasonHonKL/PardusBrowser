use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;

use pardus_core::{FormState, InteractionResult, ScrollDirection};

use crate::{InteractAction, OutputFormatArg};

pub async fn run(
    url: &str,
    action: InteractAction,
    format: OutputFormatArg,
    js: bool,
    wait_ms: u32,
) -> Result<()> {
    let start = Instant::now();
    let app = Arc::new(pardus_core::App::new(pardus_core::BrowserConfig::default()));

    // Fetch the page
    let page = if js {
        pardus_core::Page::from_url_with_js(&app, url, wait_ms).await?
    } else {
        pardus_core::Page::from_url(&app, url).await?
    };

    let elapsed = start.elapsed().as_millis();
    eprintln!("Connected in {}ms", elapsed);

    match action {
        InteractAction::Click { selector } => {
            let result = app.click_selector(&page, &selector).await?;
            output_result(&result, &format);
        }
        InteractAction::Type { selector, value } => {
            match page.query(&selector) {
                Some(handle) => {
                    let result = pardus_core::App::type_text(&page, &handle, &value)?;
                    output_result(&result, &format);
                }
                None => eprintln!("Element not found: {}", selector),
            }
        }
        InteractAction::Submit { selector, field } => {
            let mut state = FormState::new();
            for f in &field {
                let parts: Vec<&str> = f.splitn(2, '=').collect();
                if parts.len() == 2 {
                    state.set(parts[0], parts[1]);
                } else {
                    eprintln!("Invalid field format '{}', expected name=value", f);
                }
            }
            let result = app.submit_form(&page, &selector, &state).await?;
            output_result(&result, &format);
        }
        InteractAction::Wait {
            selector,
            timeout_ms,
        } => {
            let result = app
                .wait_for_selector(&page, &selector, timeout_ms, 500)
                .await?;
            output_result(&result, &format);
        }
        InteractAction::Scroll { direction } => {
            let dir = match direction.as_str() {
                "up" => ScrollDirection::Up,
                "to-top" => ScrollDirection::ToTop,
                "to-bottom" => ScrollDirection::ToBottom,
                _ => ScrollDirection::Down,
            };
            let result = app.scroll(&page, dir).await?;
            output_result(&result, &format);
        }
    }

    Ok(())
}

fn output_result(result: &InteractionResult, format: &OutputFormatArg) {
    match result {
        InteractionResult::Navigated(new_page) => {
            eprintln!("Navigated to: {}", new_page.url);
            let tree = new_page.semantic_tree();
            match format {
                OutputFormatArg::Md => {
                    let output = pardus_core::output::md_formatter::format_md(&tree);
                    println!("{}", output);
                }
                OutputFormatArg::Tree => {
                    let output = pardus_core::output::tree_formatter::format_tree(&tree);
                    println!("{}", output);
                }
                OutputFormatArg::Json => {
                    let json = pardus_core::output::json_formatter::format_json(
                        &new_page.url,
                        new_page.title(),
                        &tree,
                        None,
                        None,
                    )
                    .unwrap_or_default();
                    println!("{}", json);
                }
            }
        }
        InteractionResult::Typed { selector, value } => {
            println!("Typed '{}' into {}", value, selector);
        }
        InteractionResult::Toggled { selector, checked } => {
            println!(
                "Toggled {} -> {}",
                selector,
                if *checked { "checked" } else { "unchecked" }
            );
        }
        InteractionResult::Selected { selector, value } => {
            println!("Selected '{}' in {}", value, selector);
        }
        InteractionResult::ElementNotFound { selector, reason } => {
            eprintln!("Element not found: {} — {}", selector, reason);
        }
        InteractionResult::WaitSatisfied { selector, found } => {
            if *found {
                println!("Wait satisfied: {} found", selector);
            } else {
                eprintln!("Wait timeout: {} not found", selector);
            }
        }
        InteractionResult::Scrolled { url, page: new_page } => {
            eprintln!("Scrolled to: {}", url);
            let tree = new_page.semantic_tree();
            match format {
                OutputFormatArg::Md => {
                    let output = pardus_core::output::md_formatter::format_md(&tree);
                    println!("{}", output);
                }
                OutputFormatArg::Tree => {
                    let output = pardus_core::output::tree_formatter::format_tree(&tree);
                    println!("{}", output);
                }
                OutputFormatArg::Json => {
                    let json = pardus_core::output::json_formatter::format_json(
                        &new_page.url,
                        new_page.title(),
                        &tree,
                        None,
                        None,
                    )
                    .unwrap_or_default();
                    println!("{}", json);
                }
            }
        }
    }
}
