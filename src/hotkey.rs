//! Registers global hotkeys via XGrabKey (x11rb).
//! Emits trigger events to the daemon loop.

use std::sync::mpsc;

/// Event emitted when a global hotkey is pressed.
#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    /// User pressed the lookup hotkey.
    Lookup,
    /// User pressed the annotate hotkey.
    Annotate,
}

/// Parse a hotkey string like "Ctrl+Alt+W" into X11 modifier mask and keycode.
struct ParsedHotkey {
    modifiers: u16,
    keysym: u32,
}

fn parse_hotkey_string(hotkey: &str) -> Result<ParsedHotkey, String> {
    use x11rb::protocol::xproto;

    let parts: Vec<&str> = hotkey.split('+').collect();
    if parts.is_empty() {
        return Err("Empty hotkey string".to_string());
    }

    let mut modifiers: u16 = 0;
    let key_part = parts.last().ok_or("No key specified")?;

    for part in &parts[..parts.len() - 1] {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= xproto::ModMask::CONTROL.into(),
            "alt" | "mod1" => modifiers |= xproto::ModMask::M1.into(),
            "shift" => modifiers |= xproto::ModMask::SHIFT.into(),
            "super" | "mod4" => modifiers |= xproto::ModMask::M4.into(),
            other => return Err(format!("Unknown modifier: {}", other)),
        }
    }

    // Convert key name to X11 keysym
    let keysym = key_name_to_keysym(key_part)?;

    Ok(ParsedHotkey { modifiers, keysym })
}

fn key_name_to_keysym(key: &str) -> Result<u32, String> {
    // Common key mappings
    match key.to_uppercase().as_str() {
        "A" => Ok(0x61),
        "B" => Ok(0x62),
        "C" => Ok(0x63),
        "D" => Ok(0x64),
        "E" => Ok(0x65),
        "F" => Ok(0x66),
        "G" => Ok(0x67),
        "H" => Ok(0x68),
        "I" => Ok(0x69),
        "J" => Ok(0x6a),
        "K" => Ok(0x6b),
        "L" => Ok(0x6c),
        "M" => Ok(0x6d),
        "N" => Ok(0x6e),
        "O" => Ok(0x6f),
        "P" => Ok(0x70),
        "Q" => Ok(0x71),
        "R" => Ok(0x72),
        "S" => Ok(0x73),
        "T" => Ok(0x74),
        "U" => Ok(0x75),
        "V" => Ok(0x76),
        "W" => Ok(0x77),
        "X" => Ok(0x78),
        "Y" => Ok(0x79),
        "Z" => Ok(0x7a),
        "SPACE" => Ok(0x20),
        "RETURN" | "ENTER" => Ok(0xff0d),
        "ESCAPE" | "ESC" => Ok(0xff1b),
        "TAB" => Ok(0xff09),
        "F1" => Ok(0xffbe),
        "F2" => Ok(0xffbf),
        "F3" => Ok(0xffc0),
        "F4" => Ok(0xffc1),
        "F5" => Ok(0xffc2),
        "F6" => Ok(0xffc3),
        "F7" => Ok(0xffc4),
        "F8" => Ok(0xffc5),
        "F9" => Ok(0xffc6),
        "F10" => Ok(0xffc7),
        "F11" => Ok(0xffc8),
        "F12" => Ok(0xffc9),
        _ => Err(format!("Unknown key: {}", key)),
    }
}

/// Global hotkey registrar and listener.
pub struct HotkeyListener {
    sender: mpsc::Sender<HotkeyEvent>,
    lookup_hotkey: String,
    annotate_hotkey: String,
}

impl HotkeyListener {
    /// Create a new hotkey listener.
    pub fn new(
        sender: mpsc::Sender<HotkeyEvent>,
        lookup_hotkey: String,
        annotate_hotkey: String,
    ) -> Self {
        HotkeyListener {
            sender,
            lookup_hotkey,
            annotate_hotkey,
        }
    }

    /// Register global hotkeys and start listening.
    /// This function blocks and should run in its own thread.
    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{self, ConnectionExt};

        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;

        let lookup = parse_hotkey_string(&self.lookup_hotkey)?;
        let annotate = parse_hotkey_string(&self.annotate_hotkey)?;

        // Get keycodes from keysyms
        let setup = conn.setup();
        let min_keycode = setup.min_keycode;
        let max_keycode = setup.max_keycode;

        let keyboard_mapping = conn
            .get_keyboard_mapping(min_keycode, max_keycode - min_keycode + 1)?
            .reply()?;

        let keysyms_per_keycode = keyboard_mapping.keysyms_per_keycode as usize;

        let lookup_keycode = find_keycode(
            &keyboard_mapping.keysyms,
            keysyms_per_keycode,
            min_keycode,
            lookup.keysym,
        )
        .ok_or_else(|| format!("Cannot find keycode for lookup hotkey: {}", self.lookup_hotkey))?;

        let annotate_keycode = find_keycode(
            &keyboard_mapping.keysyms,
            keysyms_per_keycode,
            min_keycode,
            annotate.keysym,
        )
        .ok_or_else(|| {
            format!(
                "Cannot find keycode for annotate hotkey: {}",
                self.annotate_hotkey
            )
        })?;

        // Grab the keys on root window
        // We need to grab with various NumLock/CapsLock combinations
        let num_lock_mask: u16 = 0x10; // Mod2Mask typically
        let caps_lock_mask: u16 = 0x02; // LockMask
        let extra_modifiers: [u16; 4] = [
            0,
            num_lock_mask,
            caps_lock_mask,
            num_lock_mask | caps_lock_mask,
        ];

        for extra in &extra_modifiers {
            conn.grab_key(
                false,
                root,
                (lookup.modifiers | extra).into(),
                lookup_keycode,
                xproto::GrabMode::ASYNC,
                xproto::GrabMode::ASYNC,
            )?;

            conn.grab_key(
                false,
                root,
                (annotate.modifiers | extra).into(),
                annotate_keycode,
                xproto::GrabMode::ASYNC,
                xproto::GrabMode::ASYNC,
            )?;
        }

        conn.flush()?;

        log::info!(
            "Hotkeys registered: lookup={}, annotate={}",
            self.lookup_hotkey,
            self.annotate_hotkey
        );

        // Event loop
        loop {
            let event = conn.wait_for_event()?;
            let event_type = event.response_type() & 0x7f;

            if event_type == xproto::KEY_PRESS_EVENT {
                // Parse the key press event bytes manually
                let data = event.raw_bytes();
                if data.len() >= 8 {
                    let detail = data[1]; // keycode
                    let state = u16::from_ne_bytes([data[4], data[5]]); // modifier state

                    // Mask out NumLock and CapsLock
                    let clean_state = state & !(num_lock_mask | caps_lock_mask);

                    if detail == lookup_keycode && clean_state == lookup.modifiers {
                        log::debug!("Lookup hotkey pressed");
                        let _ = self.sender.send(HotkeyEvent::Lookup);
                    } else if detail == annotate_keycode && clean_state == annotate.modifiers {
                        log::debug!("Annotate hotkey pressed");
                        let _ = self.sender.send(HotkeyEvent::Annotate);
                    }
                }
            }
        }
    }
}

/// Find the keycode for a given keysym in the keyboard mapping.
fn find_keycode(
    keysyms: &[u32],
    keysyms_per_keycode: usize,
    min_keycode: u8,
    target_keysym: u32,
) -> Option<u8> {
    for (i, chunk) in keysyms.chunks(keysyms_per_keycode).enumerate() {
        for ks in chunk {
            if *ks == target_keysym {
                return Some(min_keycode + i as u8);
            }
        }
    }
    None
}
