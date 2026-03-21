//! X11 PRIMARY selection watcher via x11rb.
//! Emits word events when selection changes in a PDF viewer.


/// Known PDF viewer window class names.
pub const PDF_VIEWER_CLASSES: &[&str] = &[
    "evince",
    "Evince",
    "okular",
    "Okular",
    "org.gnome.Evince",
    "org.kde.okular",
];

/// Event emitted when a word is selected in a PDF viewer.
#[derive(Debug, Clone)]
pub struct SelectionEvent {
    pub text: String,
}

/// X11 PRIMARY selection watcher.
///
/// Uses XFixes to monitor selection changes, checks if the source window
/// belongs to a known PDF viewer, and sends selected text through a channel.
pub struct SelectionWatcher {
    sender: tokio::sync::mpsc::UnboundedSender<SelectionEvent>,
    pdf_auto_trigger: bool,
}

impl SelectionWatcher {
    /// Create a new selection watcher.
    pub fn new(sender: tokio::sync::mpsc::UnboundedSender<SelectionEvent>, pdf_auto_trigger: bool) -> Self {
        SelectionWatcher {
            sender,
            pdf_auto_trigger,
        }
    }

    /// Start watching for X11 PRIMARY selection changes.
    ///
    /// This function blocks and runs in its own thread.
    /// It connects to the X11 display, registers for XFixesSelectionNotify
    /// events on the PRIMARY selection, and monitors for changes.
    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.pdf_auto_trigger {
            log::info!("PDF auto-trigger disabled, selection watcher not started");
            return Ok(());
        }

        use x11rb::connection::Connection;
        use x11rb::protocol::xfixes::{self, ConnectionExt as XFixesExt};
        use x11rb::protocol::xproto::{self, ConnectionExt};

        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;

        // Query XFixes extension and get its event base
        let xfixes_info = conn.xfixes_query_version(5, 0)?.reply()?;
        let _ = xfixes_info; // version confirmed

        // Get extension info for event-base arithmetic
        let ext_info = conn
            .query_extension(b"XFIXES")?
            .reply()?;
        // XFixes SelectionNotify is event_base + 0
        let xfixes_selection_notify_type = ext_info.first_event;

        // Get PRIMARY atom
        let primary_atom = xproto::AtomEnum::PRIMARY;

        // Create a window to receive selection events
        let event_window = conn.generate_id()?;
        conn.create_window(
            0,
            event_window,
            root,
            0,
            0,
            1,
            1,
            0,
            xproto::WindowClass::INPUT_ONLY,
            0,
            &xproto::CreateWindowAux::new(),
        )?;

        // Register for selection change notifications
        conn.xfixes_select_selection_input(
            event_window,
            primary_atom.into(),
            xfixes::SelectionEventMask::SET_SELECTION_OWNER
                | xfixes::SelectionEventMask::SELECTION_WINDOW_DESTROY
                | xfixes::SelectionEventMask::SELECTION_CLIENT_CLOSE,
        )?;

        conn.flush()?;

        log::info!("Selection watcher started, monitoring PRIMARY selection");

        // UTF8_STRING atom for selection conversion
        let utf8_string = conn
            .intern_atom(false, b"UTF8_STRING")?
            .reply()?
            .atom;
        let wd_sel = conn
            .intern_atom(false, b"WD_SELECTION")?
            .reply()?
            .atom;

        // Event loop
        loop {
            let event = conn.wait_for_event()?;

            // Check for XFixes SelectionNotify events
            let event_type = event.response_type() & 0x7f;
            log::debug!("X11 event received: type={}", event_type);

            // XFixes selection notify event type (first_event + 0)
            if event_type == xfixes_selection_notify_type {
                log::debug!("XFixes SelectionNotify received");
                // Read the selection owner window
                let selection_owner = conn
                    .get_selection_owner(primary_atom.into())?
                    .reply()?
                    .owner;

                log::debug!("Selection owner window: {}", selection_owner);
                if selection_owner == 0 {
                    continue;
                }

                // Check if the owner window belongs to a PDF viewer
                if !self.is_pdf_viewer_window(&conn, selection_owner) {
                    log::debug!("Selection owner is not a PDF viewer, skipping");
                    continue;
                }

                // Request the selection content
                conn.convert_selection(
                    event_window,
                    primary_atom.into(),
                    utf8_string,
                    wd_sel,
                    x11rb::CURRENT_TIME,
                )?;
                conn.flush()?;
            }

            // Handle SelectionNotify (response to convert_selection)
            if event_type == xproto::SELECTION_NOTIFY_EVENT {
                // Read the property
                let prop = conn
                    .get_property(
                        true,
                        event_window,
                        wd_sel,
                        utf8_string,
                        0,
                        1024 * 1024,
                    )?
                    .reply()?;

                if let Ok(text) = String::from_utf8(prop.value) {
                    let text = text.trim().to_string();
                    if !text.is_empty() && text.split_whitespace().count() <= 3 {
                        // Only trigger for single words or short phrases
                        log::debug!("PDF selection detected: {:?}", text);
                        let _ = self.sender.send(SelectionEvent { text });
                    } else if !text.is_empty() {
                        log::debug!("Selection too long ({}), ignoring: {:?}", text.split_whitespace().count(), &text[..text.len().min(50)]);
                    }
                }
            }
        }
    }

    /// Check if a window belongs to a known PDF viewer by examining WM_CLASS,
    /// traversing up the window tree since the selection owner might be a child window.
    fn is_pdf_viewer_window(
        &self,
        conn: &impl x11rb::protocol::xproto::ConnectionExt,
        window: u32,
    ) -> bool {
        let wm_class_atom = match conn.intern_atom(false, b"WM_CLASS") {
            Ok(cookie) => match cookie.reply() {
                Ok(reply) => reply.atom,
                Err(_) => return false,
            },
            Err(_) => return false,
        };

        let mut current_window = window;
        
        // Traverse up the window tree
        for depth in 0..10 {
            if current_window == 0 {
                break;
            }
            
            let prop = match conn.get_property(
                false,
                current_window,
                wm_class_atom,
                x11rb::protocol::xproto::AtomEnum::ANY,
                0,
                1024,
            ) {
                Ok(cookie) => match cookie.reply() {
                    Ok(prop) => prop,
                    Err(e) => {
                        log::debug!("get_property reply error at depth {}: {:?}", depth, e);
                        return false;
                    }
                },
                Err(e) => {
                    log::debug!("get_property error at depth {}: {:?}", depth, e);
                    return false;
                }
            };

            if prop.value.is_empty() {
                // Try parent window
                match conn.query_tree(current_window) {
                    Ok(cookie) => match cookie.reply() {
                        Ok(tree) => {
                            if tree.parent == current_window || tree.parent == 0 || tree.parent == tree.root {
                                log::debug!("Reached root/null at depth {}. Treating as Wayland proxy window.", depth);
                                return false; // Reverted: Global popup User can make this true if they want
                            }
                            log::debug!("WM_CLASS empty on win {}, trying parent: {}", current_window, tree.parent);
                            current_window = tree.parent;
                            continue;
                        }
                        Err(e) => {
                            log::debug!("query_tree reply error: {:?}", e);
                            break;
                        }
                    },
                    Err(e) => {
                        log::debug!("query_tree error: {:?}", e);
                        break;
                    }
                }
            }

            // WM_CLASS is two null-terminated strings: instance\0class\0
            // We must split on null bytes, not treat as a single UTF-8 string
            let segments: Vec<&[u8]> = prop.value.split(|&b| b == 0).collect();
            for seg in &segments {
                if seg.is_empty() {
                    continue;
                }
                let seg_str = String::from_utf8_lossy(seg).to_lowercase();
                log::debug!("WM_CLASS segment found on win {}: {:?}", current_window, seg_str);
                for class_name in PDF_VIEWER_CLASSES {
                    if seg_str.contains(&class_name.to_lowercase()) {
                        log::debug!("PDF viewer detected: {:?}", seg_str);
                        return true;
                    }
                }
            }
            
            // If we found a WM_CLASS but it didn't match, we stop traversing.
            log::debug!("WM_CLASS found but no match. Not a PDF viewer.");
            return false;
        }

        false
    }
}

/// Read the current X11 PRIMARY selection text.
pub fn read_primary_selection() -> Result<String, Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{self, ConnectionExt};

    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    let utf8_string = conn.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;
    let wd_sel = conn.intern_atom(false, b"WD_SEL_READ")?.reply()?.atom;

    // Create a temporary window
    let window = conn.generate_id()?;
    conn.create_window(
        0,
        window,
        root,
        0,
        0,
        1,
        1,
        0,
        xproto::WindowClass::INPUT_ONLY,
        0,
        &xproto::CreateWindowAux::new(),
    )?;

    // Request selection conversion
    conn.convert_selection(
        window,
        xproto::AtomEnum::PRIMARY.into(),
        utf8_string,
        wd_sel,
        x11rb::CURRENT_TIME,
    )?;
    conn.flush()?;

    // Wait for SelectionNotify
    loop {
        let event = conn.wait_for_event()?;
        let event_type = event.response_type() & 0x7f;
        if event_type == xproto::SELECTION_NOTIFY_EVENT {
            break;
        }
    }

    // Read the property
    let prop = conn
        .get_property(true, window, wd_sel, utf8_string, 0, 1024 * 1024)?
        .reply()?;

    conn.destroy_window(window)?;
    conn.flush()?;

    let text = String::from_utf8(prop.value)
        .map_err(|e| format!("Invalid UTF-8 in selection: {}", e))?;

    Ok(text.trim().to_string())
}
