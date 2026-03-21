//! Entry point. Parses argv. Routes to daemon mode or CLI mode. No business logic.

mod annotation;
mod cli;
mod config;
mod daemon;
mod hotkey;
mod lookup;
mod popup;
mod selection;
mod types;
mod wiktionary;
mod wordnet;

use clap::Parser;
use config::Config;

/// wd — a lightweight word-lookup daemon and CLI tool for Linux.
#[derive(Parser, Debug)]
#[command(
    name = "wd",
    version,
    about = "Look up the meaning of a word",
    long_about = "wd is a lightweight word-lookup daemon and CLI tool for Linux.\n\n\
                   CLI mode:    wd <word>\n\
                   Daemon mode: wd daemon   (or: wd --daemon)"
)]
struct Args {
    /// Run as a background daemon (watches PDF selections, responds to hotkeys).
    #[arg(long)]
    daemon: bool,

    /// The word to look up (CLI mode).
    #[arg(value_name = "WORD")]
    word: Option<String>,
}

fn main() {
    // Initialise logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .init();

    let args = Args::parse();
    let config = Config::load();

    // Support both `wd --daemon` and `wd daemon` (intuitive subcommand style)
    let start_daemon = args.daemon || args.word.as_deref() == Some("daemon");

    if start_daemon {
        // Daemon mode
        daemon::run(&config);
    } else if let Some(word) = args.word {
        // CLI mode
        cli::run(&word, &config);
    } else {
        // No arguments: print help
        use clap::CommandFactory;
        Args::command().print_help().ok();
        println!();
        std::process::exit(1);
    }
}
