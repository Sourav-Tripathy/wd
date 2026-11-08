//! Daemon mode handler. Initialises X11, GTK4, hotkeys, enters event loop.

use crate::config::Config;
use crate::hotkey::{HotkeyEvent, HotkeyListener};
use crate::lookup;
use crate::popup;
use crate::selection::{self, SelectionEvent, SelectionWatcher};
use crate::wordnet::WordNetIndex;

use gtk4::glib;
use gtk4::prelude::*;
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
    setup_wayland_pdf_compatibility();

    let _rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = _rt.enter();

    let config_arc = Arc::new(config.clone());
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
    let hk_tx = hotkey_tx.clone();
    std::thread::spawn(move || {
        let listener = HotkeyListener::new(hk_tx, lookup_hotkey);
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
        // GTK4 requires holding the application alive for daemons that don't immediately open a window.
        // std::mem::forget ensures the hold is never dropped until process exit.
        std::mem::forget(app.hold());
    });

    // Process events from selection watcher
    let app_clone = app.clone();
    let wordnet_sel = wordnet.clone();
    let config_sel = config_arc.clone();
    glib::MainContext::default().spawn_local(async move {
        while let Some(event) = selection_rx.recv().await {
            handle_lookup(
                &app_clone,
                event.text,
                wordnet_sel.clone(),
                config_sel.clone(),
                event.x,
                event.y,
            );
        }
    });

    // Process events from hotkey listener
    let app_clone = app.clone();
    let wordnet_hk = wordnet.clone();
    let config_hk = config_arc.clone();
    glib::MainContext::default().spawn_local(async move {
        while let Some(event) = hotkey_rx.recv().await {
            match event {
                HotkeyEvent::Lookup(x, y) => {
                    // Read current PRIMARY selection
                    match selection::read_primary_selection() {
                        Ok(text) if !text.is_empty() => {
                            handle_lookup(
                                &app_clone,
                                text,
                                wordnet_hk.clone(),
                                config_hk.clone(),
                                x,
                                y,
                            );
                        }
                        Ok(_) => log::debug!("Empty selection, ignoring lookup hotkey"),
                        Err(e) => log::warn!("Failed to read PRIMARY selection: {}", e),
                    }
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
    word: String,
    wordnet: Arc<WordNetIndex>,
    config: Arc<Config>,
    x: i32,
    y: i32,
) {
    let app = app.clone();
    glib::MainContext::default().spawn_local(async move {
        log::info!("Looking up: {:?}", word);

        let config_lookup = config.clone();
        let result =
            tokio::task::spawn_blocking(move || lookup::lookup(&word, &wordnet, &config_lookup))
                .await;

        match result {
            Ok(Ok(definitions)) => {
                popup::show(&app, definitions, &config, x, y);
            }
            Ok(Err(e)) => {
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
            Err(e) => log::error!("Task panicked: {}", e),
        }
    });
}

/// Helper to patch an Exec line in a .desktop file to prefix it with an environment variable.
fn patch_exec_command(content: &str, target_cmd: &str, env_prefix: &str) -> Option<String> {
    if content.contains(env_prefix) {
        return None;
    }

    let mut modified = false;
    let mut lines = Vec::new();

    for line in content.lines() {
        if let Some(exec_val) = line.strip_prefix("Exec=") {
            let trimmed = exec_val.trim_start();
            let matches_binary = trimmed == target_cmd
                || trimmed.starts_with(&format!("{} ", target_cmd))
                || trimmed == format!("/usr/bin/{}", target_cmd)
                || trimmed.starts_with(&format!("/usr/bin/{} ", target_cmd));

            if matches_binary {
                lines.push(format!("Exec=env {} {}", env_prefix, trimmed));
                modified = true;
                continue;
            }
        }
        lines.push(line.to_string());
    }

    if modified {
        let mut result = lines.join("\n");
        if content.ends_with('\n') {
            result.push('\n');
        }
        Some(result)
    } else {
        None
    }
}

fn apply_desktop_override(
    system_desktop: &std::path::Path,
    local_desktop: &std::path::Path,
    target_cmd: &str,
    env_prefix: &str,
    viewer_name: &str,
) {
    let source_path = if local_desktop.exists() {
        local_desktop
    } else if system_desktop.exists() {
        system_desktop
    } else {
        return;
    };

    if let Ok(content) = std::fs::read_to_string(source_path) {
        if let Some(updated) = patch_exec_command(&content, target_cmd, env_prefix) {
            log::info!(
                "Wayland detected. Writing {} desktop override for XWayland.",
                viewer_name
            );
            if let Err(e) = std::fs::write(local_desktop, updated) {
                log::warn!(
                    "Failed to write local {} desktop override: {}",
                    viewer_name,
                    e
                );
            }
        }
    }
}

/// Create local .desktop overrides to force PDF viewers (Papers/Evince/Okular) to run
/// under XWayland on Wayland sessions, so their text selections are visible to
/// the X11 PRIMARY selection watcher.
fn setup_wayland_pdf_compatibility() {
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => {
                log::debug!("Could not determine home directory. Skipping Wayland overrides.");
                return;
            }
        };

        let local_apps_dir = home.join(".local/share/applications");
        if !local_apps_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&local_apps_dir) {
                log::warn!("Failed to create local applications directory: {}. Skipping Wayland overrides.", e);
                return;
            }
        }

        // 1. GNOME Papers (default in Ubuntu 24.10+ / 26.04+) — force X11 backend
        let system_papers = [
            std::path::PathBuf::from("/usr/share/applications/org.gnome.Papers.desktop"),
            std::path::PathBuf::from("/usr/share/applications/papers.desktop"),
        ];
        for sys_path in &system_papers {
            let filename = sys_path.file_name().unwrap();
            let local_path = local_apps_dir.join(filename);
            apply_desktop_override(sys_path, &local_path, "papers", "GDK_BACKEND=x11", "Papers");
        }

        // 2. GNOME Evince (Ubuntu 22.04 / 24.04) — force X11 backend
        let system_evince =
            std::path::PathBuf::from("/usr/share/applications/org.gnome.Evince.desktop");
        let local_evince = local_apps_dir.join("org.gnome.Evince.desktop");
        apply_desktop_override(
            &system_evince,
            &local_evince,
            "evince",
            "GDK_BACKEND=x11",
            "Evince",
        );

        // 3. KDE Okular — force xcb platform
        let system_okular =
            std::path::PathBuf::from("/usr/share/applications/org.kde.okular.desktop");
        let local_okular = local_apps_dir.join("org.kde.okular.desktop");
        apply_desktop_override(
            &system_okular,
            &local_okular,
            "okular",
            "QT_QPA_PLATFORM=xcb",
            "Okular",
        );

        // Update desktop database so the overrides are picked up immediately
        if let Ok(status) = std::process::Command::new("update-desktop-database")
            .arg(&local_apps_dir)
            .status()
        {
            if !status.success() {
                log::debug!("update-desktop-database exited with non-zero status.");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_exec_command_papers() {
        let input = "[Desktop Entry]\nName=Papers\nExec=papers %U\nType=Application\n";
        let patched = patch_exec_command(input, "papers", "GDK_BACKEND=x11").unwrap();
        assert_eq!(
            patched,
            "[Desktop Entry]\nName=Papers\nExec=env GDK_BACKEND=x11 papers %U\nType=Application\n"
        );
    }

    #[test]
    fn test_patch_exec_command_usr_bin() {
        let input = "[Desktop Entry]\nName=Papers\nExec=/usr/bin/papers %U\nType=Application\n";
        let patched = patch_exec_command(input, "papers", "GDK_BACKEND=x11").unwrap();
        assert_eq!(
            patched,
            "[Desktop Entry]\nName=Papers\nExec=env GDK_BACKEND=x11 /usr/bin/papers %U\nType=Application\n"
        );
    }

    #[test]
    fn test_patch_exec_command_already_patched() {
        let input =
            "[Desktop Entry]\nName=Papers\nExec=env GDK_BACKEND=x11 papers %U\nType=Application\n";
        assert!(patch_exec_command(input, "papers", "GDK_BACKEND=x11").is_none());
    }

    #[test]
    fn test_patch_exec_command_unmatched() {
        let input = "[Desktop Entry]\nName=Text Editor\nExec=gedit %U\nType=Application\n";
        assert!(patch_exec_command(input, "papers", "GDK_BACKEND=x11").is_none());
    }
}
