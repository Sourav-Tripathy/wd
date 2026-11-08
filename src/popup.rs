//! GTK4 popup window. Receives Definition, renders near cursor,
//! handles dismiss and annotation hotkey.

use crate::config::Config;
use crate::types::Definition;
use gtk4::prelude::*;
use gtk4::{
    gdk, glib, Application, ApplicationWindow, Box as GtkBox, CssProvider, Label, Orientation,
    ScrolledWindow,
};

thread_local! {
    static ACTIVE_WINDOW: std::cell::RefCell<Option<ApplicationWindow>> = const { std::cell::RefCell::new(None) };
    static CSS_PROVIDER: std::cell::RefCell<Option<CssProvider>> = const { std::cell::RefCell::new(None) };
}

fn parse_hex_color(color: &str) -> Option<(u8, u8, u8)> {
    let c = color.trim().strip_prefix('#').unwrap_or(color.trim());
    if c.len() == 6 {
        let r = u8::from_str_radix(&c[0..2], 16).ok()?;
        let g = u8::from_str_radix(&c[2..4], 16).ok()?;
        let b = u8::from_str_radix(&c[4..6], 16).ok()?;
        Some((r, g, b))
    } else if c.len() == 3 {
        let r = u8::from_str_radix(&c[0..1], 16).ok()?;
        let g = u8::from_str_radix(&c[1..2], 16).ok()?;
        let b = u8::from_str_radix(&c[2..3], 16).ok()?;
        Some((r * 17, g * 17, b * 17))
    } else {
        match color.to_lowercase().as_str() {
            "white" => Some((255, 255, 255)),
            "black" => Some((0, 0, 0)),
            _ => None,
        }
    }
}

fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    fn srgb_to_linear(v: u8) -> f64 {
        let val = (v as f64) / 255.0;
        if val <= 0.04045 {
            val / 12.92
        } else {
            ((val + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)
}

fn contrast_ratio(l1: f64, l2: f64) -> f64 {
    let (bright, dark) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (bright + 0.05) / (dark + 0.05)
}

/// Ensures the text color has adequate contrast against the background color.
/// If contrast is insufficient (< 3.0:1), automatically selects a high-contrast text color.
pub fn ensure_contrast(text_color: &str, bg_color: &str) -> String {
    if let (Some((tr, tg, tb)), Some((br, bg, bb))) =
        (parse_hex_color(text_color), parse_hex_color(bg_color))
    {
        let text_lum = relative_luminance(tr, tg, tb);
        let bg_lum = relative_luminance(br, bg, bb);
        let ratio = contrast_ratio(text_lum, bg_lum);
        if ratio < 3.0 {
            if bg_lum > 0.4 {
                "#1f2328".to_string()
            } else {
                "#f0f6fc".to_string()
            }
        } else {
            text_color.to_string()
        }
    } else {
        text_color.to_string()
    }
}

fn generate_css(config: &Config) -> String {
    let font_size = config.popup_font_size;
    let bg_color = &config.popup_bg_color;
    let text_color = ensure_contrast(&config.popup_text_color, bg_color);
    let header_color = ensure_contrast(&config.popup_header_color, bg_color);
    let accent_color = &config.popup_accent_color;
    let border_color = &config.popup_border_color;
    let example_color = &config.popup_example_color;

    format!(
        r#"
        window {{
            background-color: {bg_color};
            border-radius: 12px;
            border: 1px solid {border_color};
        }}
        .popup-container {{
            padding: 16px;
        }}
        .word-header {{
            color: {header_color};
            font-size: {h_size}pt;
            font-weight: bold;
        }}
        .pos-label {{
            color: {example_color};
            font-size: {font_size}pt;
            font-style: italic;
        }}
        .definition-text {{
            color: {text_color};
            font-size: {font_size}pt;
        }}
        .example-text {{
            color: {example_color};
            font-size: {ex_size}pt;
            font-style: italic;
        }}
        .source-label {{
            color: {example_color};
            font-size: {src_size}pt;
        }}
        .sense-number {{
            color: {accent_color};
            font-size: {font_size}pt;
            font-weight: bold;
        }}
        "#,
        bg_color = bg_color,
        border_color = border_color,
        header_color = header_color,
        h_size = font_size + 2,
        example_color = example_color,
        text_color = text_color,
        font_size = font_size,
        ex_size = font_size.saturating_sub(1),
        src_size = font_size.saturating_sub(2),
        accent_color = accent_color,
    )
}

/// Show a popup window near the cursor with the given definitions.
pub fn show(app: &Application, definitions: Vec<Definition>, config: &Config, x: i32, y: i32) {
    let timeout_ms = config.popup_timeout_ms;

    // Close the previous window if any to prevent task list accumulation
    ACTIVE_WINDOW.with(|active| {
        if let Some(old_window) = active.borrow_mut().take() {
            old_window.close();
        }
    });

    let window = ApplicationWindow::builder()
        .application(app)
        .title("wd")
        .default_width(380)
        .default_height(200)
        .decorated(false)
        .resizable(false)
        .build();

    // Store the new window reference
    ACTIVE_WINDOW.with(|active| {
        *active.borrow_mut() = Some(window.clone());
    });

    // Clear reference when the window is destroyed
    let window_clone = window.clone();
    window.connect_destroy(move |_| {
        ACTIVE_WINDOW.with(|active| {
            let mut borrow = active.borrow_mut();
            if let Some(ref current) = *borrow {
                if current == &window_clone {
                    *borrow = None;
                }
            }
        });
    });

    // Load or update CSS provider with current config colors
    CSS_PROVIDER.with(|cell| {
        let mut opt = cell.borrow_mut();
        let provider = opt.get_or_insert_with(|| {
            let p = CssProvider::new();
            if let Some(display) = gdk::Display::default() {
                gtk4::style_context_add_provider_for_display(
                    &display,
                    &p,
                    gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
            p
        });
        provider.load_from_data(&generate_css(config));
    });

    // Build content
    let container = GtkBox::new(Orientation::Vertical, 8);
    container.add_css_class("popup-container");

    for def in definitions {
        // Word header
        let header_box = GtkBox::new(Orientation::Horizontal, 8);
        let word_label = Label::new(Some(&def.word));
        word_label.add_css_class("word-header");
        header_box.append(&word_label);

        let pos_label = Label::new(Some(&format!("({})", def.pos)));
        pos_label.add_css_class("pos-label");
        header_box.append(&pos_label);

        container.append(&header_box);

        // Senses
        for (i, sense) in def.senses.iter().enumerate() {
            let sense_box = GtkBox::new(Orientation::Horizontal, 4);
            sense_box.set_margin_start(8);

            let num_label = Label::new(Some(&format!("{}.", i + 1)));
            num_label.add_css_class("sense-number");
            sense_box.append(&num_label);

            let def_label = Label::new(Some(&sense.definition));
            def_label.add_css_class("definition-text");
            def_label.set_wrap(true);
            def_label.set_xalign(0.0);
            def_label.set_hexpand(true);
            sense_box.append(&def_label);

            container.append(&sense_box);

            if let Some(ref example) = sense.example {
                let ex_label = Label::new(Some(&format!("\"{}\"", example)));
                ex_label.add_css_class("example-text");
                ex_label.set_wrap(true);
                ex_label.set_xalign(0.0);
                ex_label.set_margin_start(24);
                container.append(&ex_label);
            }
        }

        // Source label
        let source_label = Label::new(Some(&format!("[{}]", def.source)));
        source_label.add_css_class("source-label");
        source_label.set_xalign(1.0);
        source_label.set_margin_top(4);
        container.append(&source_label);
    }

    // Scrolled window for long definitions
    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .max_content_height(400)
        .child(&container)
        .build();

    window.set_child(Some(&scrolled));

    // Position near cursor
    // Note: On X11, we can try to get cursor position, but GTK4 surface
    // positioning is compositor-dependent. We'll let the WM handle initial
    // placement and use set_default_size instead.

    // Escape key to dismiss
    let event_controller = gtk4::EventControllerKey::new();
    let window_clone = window.clone();
    event_controller.connect_key_pressed(move |_, keyval, _, _| {
        if keyval == gdk::Key::Escape {
            window_clone.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(event_controller);

    // Auto-dismiss timeout
    if timeout_ms > 0 {
        let window_clone = window.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(timeout_ms), move || {
            window_clone.close();
        });
    }

    window.set_opacity(0.0);
    window.present();

    // Dual-staged positioning to defeat Wayland/XWayland compositing lag.
    let window_for_opacity = window.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
        // Move natively
        move_window_to_cursor_x11(x, y);

        // Wait another 40ms for the compositor to actually render the new coordinates
        // before we make the window visible, guaranteeing zero blips.
        glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
            window_for_opacity.set_opacity(1.0);
        });
    });
}

fn move_window_to_cursor_x11(cx: i32, cy: i32) {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConfigureWindowAux, ConnectionExt};

    log::debug!("Attempting to position window at cursor: ({}, {})", cx, cy);

    if let Ok((conn, screen_num)) = x11rb::connect(None) {
        let root = conn.setup().roots[screen_num].root;

        let mut target_window = None;
        let wm_name = conn
            .intern_atom(false, b"WM_NAME")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let net_wm_name = conn
            .intern_atom(false, b"_NET_WM_NAME")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let wm_class = conn
            .intern_atom(false, b"WM_CLASS")
            .unwrap()
            .reply()
            .unwrap()
            .atom;

        // Simple BFS to find the most recent window with title or class containing "wd"
        let mut queue = vec![root];

        while let Some(w) = queue.pop() {
            // Check WM_NAME
            if let Ok(c_prop) = conn.get_property(false, w, wm_name, AtomEnum::ANY, 0, 1024) {
                if let Ok(prop) = c_prop.reply() {
                    let val = String::from_utf8_lossy(&prop.value);
                    if val == "wd" {
                        target_window = Some(w);
                        break;
                    }
                }
            }
            // Check _NET_WM_NAME
            if let Ok(c_prop) = conn.get_property(false, w, net_wm_name, AtomEnum::ANY, 0, 1024) {
                if let Ok(prop) = c_prop.reply() {
                    let val = String::from_utf8_lossy(&prop.value);
                    if val == "wd" {
                        target_window = Some(w);
                        break;
                    }
                }
            }
            // Check WM_CLASS
            if let Ok(c_prop) = conn.get_property(false, w, wm_class, AtomEnum::ANY, 0, 1024) {
                if let Ok(prop) = c_prop.reply() {
                    let segments: Vec<&[u8]> = prop.value.split(|&b| b == 0).collect();
                    for seg in &segments {
                        if !seg.is_empty() {
                            let seg_str = String::from_utf8_lossy(seg);
                            if seg_str == "wd" || seg_str == "com.wd.daemon" {
                                target_window = Some(w);
                                break;
                            }
                        }
                    }
                    if target_window.is_some() {
                        break;
                    }
                }
            }

            if let Ok(c_tree) = conn.query_tree(w) {
                if let Ok(tree) = c_tree.reply() {
                    queue.extend(tree.children);
                }
            }
        }

        if let Some(w) = target_window {
            let width = 380;
            let height = 200;

            // Centered horizontally above/below the cursor
            let mut target_x = cx - (width / 2);
            let mut target_y = cy - height - 15;

            // If positioning above goes off the screen, flip below the cursor
            if target_y < 0 {
                target_y = cy + 20;
                log::debug!("Not enough space above cursor. Positioning below instead.");
            }

            // Clamp coordinates to screen geometry if query succeeds
            if let Ok(c_geom) = conn.get_geometry(root) {
                if let Ok(geom) = c_geom.reply() {
                    let screen_width = geom.width as i32;
                    let screen_height = geom.height as i32;
                    target_x = target_x.clamp(0, screen_width - width);
                    target_y = target_y.clamp(0, screen_height - height);
                    log::debug!(
                        "Screen size: {}x{}. Positioned coordinates: ({}, {})",
                        screen_width,
                        screen_height,
                        target_x,
                        target_y
                    );
                }
            }

            log::info!("Moving window {} to: ({}, {})", w, target_x, target_y);
            let _ = conn.configure_window(w, &ConfigureWindowAux::new().x(target_x).y(target_y));
            let _ = conn.flush();
        } else {
            log::warn!("Could not find the 'wd' window on the X11 display tree.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_contrast_preserves_good_contrast() {
        // Light text on dark background
        let text = ensure_contrast("#ffffff", "#1e1e2e");
        assert_eq!(text, "#ffffff");

        // Dark text on light background
        let text = ensure_contrast("#000000", "#ffffff");
        assert_eq!(text, "#000000");
    }

    #[test]
    fn test_ensure_contrast_fixes_poor_contrast() {
        // Light text on white background -> adapt to dark
        let text = ensure_contrast("#ffffff", "#ffffff");
        assert_eq!(text, "#1f2328");

        // Dark text on dark background -> adapt to light
        let text = ensure_contrast("#111111", "#1e1e2e");
        assert_eq!(text, "#f0f6fc");
    }

    #[test]
    fn test_generate_css_includes_custom_colors() {
        let config = Config {
            popup_bg_color: "#282828".to_string(),
            popup_text_color: "#ebdbb2".to_string(),
            popup_border_color: "#504945".to_string(),
            ..Default::default()
        };

        let css = generate_css(&config);
        assert!(css.contains("background-color: #282828;"));
        assert!(css.contains("color: #ebdbb2;"));
        assert!(css.contains("border: 1px solid #504945;"));
    }
}
