use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

mod commands;

#[derive(Parser)]
#[command(name = "pardus-browser")]
#[command(version, about = "Headless browser for AI agents — semantic tree, no pixels")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Navigate to URL, build semantic tree, print it
    Navigate {
        /// URL to navigate to
        url: String,

        /// Output format
        #[arg(short, long, default_value = "md")]
        format: OutputFormatArg,

        /// Only show interactive elements
        #[arg(long)]
        interactive_only: bool,

        /// Wait time in milliseconds for JS execution
        #[arg(long, default_value = "3000")]
        wait_ms: u32,

        /// Enable JavaScript execution (for SPAs)
        #[arg(long)]
        js: bool,

        /// Include navigation graph
        #[arg(long)]
        with_nav: bool,

        /// Use persistent session (save cookies/storage)
        #[arg(long)]
        persistent: bool,

        /// Custom HTTP header (format: "Name: Value")
        #[arg(long)]
        header: Option<String>,

        /// Verbose logging
        #[arg(short, long)]
        verbose: bool,

        /// Capture and display network request table
        #[arg(long)]
        network_log: bool,
    },

    /// Interact with a page (click, type, submit, wait, scroll)
    Interact {
        /// URL to navigate to
        url: String,

        /// Action to perform
        #[command(subcommand)]
        action: InteractAction,

        /// Output format for result page
        #[arg(short, long, default_value = "md")]
        format: OutputFormatArg,

        /// Enable JavaScript execution
        #[arg(long)]
        js: bool,

        /// Wait time for JS execution (ms)
        #[arg(long, default_value = "3000")]
        wait_ms: u32,
    },

    /// Start CDP WebSocket server for automation
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on
        #[arg(long, default_value = "9222")]
        port: u16,

        /// Inactivity timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },

    /// Wipe all cache, cookies, and storage
    Clean {
        /// Clean specific directory
        #[arg(long)]
        cache_dir: Option<PathBuf>,

        /// Only clean cookies
        #[arg(long)]
        cookies_only: bool,

        /// Only clean cache
        #[arg(long)]
        cache_only: bool,
    },
}

#[derive(Clone, Subcommand)]
pub enum InteractAction {
    /// Click on an element
    Click {
        /// CSS selector of the element to click
        selector: String,
    },

    /// Type text into a field
    Type {
        /// CSS selector of the field
        selector: String,
        /// Value to type
        value: String,
    },

    /// Submit a form
    Submit {
        /// CSS selector of the form
        selector: String,
        /// Field values as "name=value" pairs
        #[arg(long)]
        field: Vec<String>,
    },

    /// Wait for a selector to appear
    Wait {
        /// CSS selector to wait for
        selector: String,
        /// Timeout in milliseconds
        #[arg(long, default_value = "5000")]
        timeout_ms: u32,
    },

    /// Scroll the page
    Scroll {
        /// Direction (down, up, to-top, to-bottom)
        #[arg(long, default_value = "down")]
        direction: String,
    },
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormatArg {
    Md,
    Tree,
    Json,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Navigate {
            url,
            format,
            interactive_only,
            wait_ms,
            with_nav,
            persistent: _,
            header: _,
            js,
            verbose,
            network_log,
        } => {
            if verbose {
                tracing_subscriber::fmt()
                    .with_env_filter("pardus_core=debug")
                    .init();
            }

            commands::navigate::run(&url, format, interactive_only, with_nav, js, wait_ms, network_log).await?;
        }
        Commands::Interact {
            url,
            action,
            format,
            js,
            wait_ms,
        } => {
            commands::interact::run(&url, action, format, js, wait_ms).await?;
        }
        Commands::Serve { host, port, timeout } => {
            tracing::info!("Starting CDP WebSocket server on ws://{host}:{port}");
            commands::serve::run(&host, port, timeout).await?;
        }
        Commands::Clean {
            cache_dir,
            cookies_only,
            cache_only,
        } => {
            commands::clean::run(cache_dir, cookies_only, cache_only)?;
        }
    }

    Ok(())
}
