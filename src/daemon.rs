//! Daemon mode handler. Initialises X11, GTK4, hotkeys, enters event loop.

use crate::config::Config;
use crate::hotkey::{HotkeyEvent, HotkeyListener};
use crate::lookup;
use crate::popup;
use crate::selection::{self, SelectionEvent, SelectionWatcher};
use crate::wordnet::WordNetIndex;

use gtk4::glib;
use gtk4::prelude::*;
use libc;
use std::sync::Arc;

/// Run the wd daemon.
///
/// Startup sequence (per architecture spec §7.1):
/// 1. Read config.toml. Create with defaults if absent.
/// 2. Load WordNet index into memory.
/// 3. Connect to X11. Register XFixesSelectSelectionInput for PRIMARY buffer.
/// 4. Register XGrabKey for all configured hotkeys.
/// 5. Initialise GTK4 application context.
/// 6. Enter event loop. Process is parked by the kernel. CPU usage: 0.0%.
pub fn run(config: &Config) {
    log::info!("Starting wd daemon...");

    // Load WordNet index
    let wordnet_dir = Config::wordnet_dir();
    let wordnet = match WordNetIndex::load(&wordnet_dir) {
        Ok(idx) => {
            log::info!("WordNet index loaded successfully");
            Arc::new(idx)
        }
        Err(e) => {
            log::warn!("WordNet not available: {}. Will use Wiktionary only.", e);
            Arc::new(WordNetIndex::new())
        }
    };

    // Create async event channels that wake up the GTK main loop (fixes 100% CPU bug)
    let (selection_tx, mut selection_rx) = tokio::sync::mpsc::unbounded_channel::<SelectionEvent>();
    let (hotkey_tx, mut hotkey_rx) = tokio::sync::mpsc::unbounded_channel::<HotkeyEvent>();

    // Start X11 selection watcher in its own thread
    let sel_config = config.pdf_auto_trigger;
    let sel_tx = selection_tx.clone();
    std::thread::spawn(move || {
        let watcher = SelectionWatcher::new(sel_tx, sel_config);
        if let Err(e) = watcher.run() {
            log::error!("Selection watcher error: {}", e);
        }
    });

    // Start hotkey listener in its own thread
    let lookup_hotkey = config.lookup_hotkey.clone();
    let annotate_hotkey = config.annotate_hotkey.clone();
    let hk_tx = hotkey_tx.clone();
    std::thread::spawn(move || {
        let listener = HotkeyListener::new(hk_tx, lookup_hotkey, annotate_hotkey);
        if let Err(e) = listener.run() {
            log::error!("Hotkey listener error: {}", e);
        }
    });

    // Initialise GTK4 application
    let app = gtk4::Application::builder()
        .application_id("com.wd.daemon")
        .build();
    app.connect_activate(move |app| {
        log::info!("GTK4 application activated, entering event loop");
        std::mem::forget(app.hold());
    });

    // Process events from selection watcher
    let app_clone = app.clone();
    let wordnet_sel = wordnet.clone();
    let config_sel = config.clone();
    glib::MainContext::default().spawn_local(async move {
        while let Some(event) = selection_rx.recv().await {
            handle_lookup(&app_clone, &event.text, &wordnet_sel, &config_sel);
        }
    });

    // Process events from hotkey listener
    let app_clone = app.clone();
    let wordnet_hk = wordnet.clone();
    let config_hk = config.clone();
    glib::MainContext::default().spawn_local(async move {
        while let Some(event) = hotkey_rx.recv().await {
            match event {
                HotkeyEvent::Lookup => {
                    // Read current PRIMARY selection
                    match selection::read_primary_selection() {
                        Ok(text) if !text.is_empty() => {
                            handle_lookup(&app_clone, &text, &wordnet_hk, &config_hk);
                        }
                        Ok(_) => log::debug!("Empty selection, ignoring lookup hotkey"),
                        Err(e) => log::warn!("Failed to read PRIMARY selection: {}", e),
                    }
                }
                HotkeyEvent::Annotate => {
                    log::debug!("Annotate hotkey pressed (annotation handled in popup)");
                }
            }
        }
    });

    // Handle SIGINT/SIGTERM for clean shutdown using GTK's main-thread signal handler
    let app_for_sigint = app.clone();
    glib::unix_signal_add_local(libc::SIGINT, move || {
        log::info!("Received SIGINT, shutting down...");
        app_for_sigint.quit();
        glib::ControlFlow::Break
    });
    let app_for_sigterm = app.clone();
    glib::unix_signal_add_local(libc::SIGTERM, move || {
        log::info!("Received SIGTERM, shutting down...");
        app_for_sigterm.quit();
        glib::ControlFlow::Break
    });

    app.run_with_args::<String>(&[]);
}

/// Handle a word lookup and show the popup.
fn handle_lookup(
    app: &gtk4::Application,
    word: &str,
    wordnet: &WordNetIndex,
    config: &Config,
) {
    log::info!("Looking up: {:?}", word);

    match lookup::lookup(word, wordnet, config) {
        Ok(definitions) => {
            popup::show(app, &definitions, config.popup_font_size, config.popup_timeout_ms);
        }
        Err(e) => {
            log::debug!("Lookup failed: {}", e);
            // Optionally show a not-found notification
            if let Err(notify_err) = notify_rust::Notification::new()
                .summary("wd")
                .body(&format!("{}", e))
                .timeout(3000)
                .show()
            {
                log::debug!("Failed to show notification: {}", notify_err);
            }
        }
    }
}


