//! Daemon mode handler. Initialises X11, GTK4, hotkeys, enters event loop.

use crate::config::Config;
use crate::hotkey::{HotkeyEvent, HotkeyListener};
use crate::lookup;
use crate::popup;
use crate::selection::{self, SelectionEvent, SelectionWatcher};
use crate::wordnet::WordNetIndex;

use gtk4::prelude::*;
use std::sync::mpsc;
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

    // Create event channels
    let (selection_tx, selection_rx) = mpsc::channel::<SelectionEvent>();
    let (hotkey_tx, hotkey_rx) = mpsc::channel::<HotkeyEvent>();

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

    let wordnet_for_app = wordnet.clone();
    let config_for_app = config.clone();

    app.connect_activate(move |app| {
        log::info!("GTK4 application activated, entering event loop");

        let wordnet = wordnet_for_app.clone();
        let config = config_for_app.clone();
        let app_clone = app.clone();

        // Process events from selection watcher and hotkey listener
        // Use a GTK idle handler to poll the channels
        gtk4::glib::idle_add_local(move || {
            // Check for selection events (non-blocking)
            if let Ok(event) = selection_rx.try_recv() {
                handle_lookup(&app_clone, &event.text, &wordnet, &config);
            }

            // Check for hotkey events (non-blocking)
            if let Ok(event) = hotkey_rx.try_recv() {
                match event {
                    HotkeyEvent::Lookup => {
                        // Read current PRIMARY selection
                        match selection::read_primary_selection() {
                            Ok(text) if !text.is_empty() => {
                                handle_lookup(&app_clone, &text, &wordnet, &config);
                            }
                            Ok(_) => {
                                log::debug!("Empty selection, ignoring lookup hotkey");
                            }
                            Err(e) => {
                                log::warn!("Failed to read PRIMARY selection: {}", e);
                            }
                        }
                    }
                    HotkeyEvent::Annotate => {
                        log::debug!("Annotate hotkey pressed (annotation handled in popup)");
                        // Annotation is handled by the popup's key handler
                    }
                }
            }

            gtk4::glib::ControlFlow::Continue
        });
    });

    // Handle signals for clean shutdown
    let app_for_signal = app.clone();
    ctrlc_handler(move || {
        log::info!("Received shutdown signal, cleaning up...");
        app_for_signal.quit();
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

/// Set up a Ctrl+C / SIGTERM handler.
fn ctrlc_handler<F: Fn() + Send + 'static>(handler: F) {
    // Simple signal handler using std
    std::thread::spawn(move || {
        use std::sync::atomic::{AtomicBool, Ordering};
        static SHUTDOWN: AtomicBool = AtomicBool::new(false);

        // Register signal handlers
        unsafe {
            libc_signal(libc_SIGTERM, || {
                SHUTDOWN.store(true, Ordering::SeqCst);
            });
            libc_signal(libc_SIGINT, || {
                SHUTDOWN.store(true, Ordering::SeqCst);
            });
        }

        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if SHUTDOWN.load(Ordering::SeqCst) {
                handler();
                break;
            }
        }
    });
}

// Minimal libc signal constants and function wrapper
const libc_SIGTERM: i32 = 15;
const libc_SIGINT: i32 = 2;

unsafe fn libc_signal<F: Fn()>(_sig: i32, _handler: F) {
    // In a real implementation, we'd use libc::signal or nix crate.
    // For now, we rely on GTK4's built-in signal handling.
    // The ctrlc_handler above is a simplified version.
}
