//! GTK4 popup window. Receives Definition, renders near cursor,
//! handles dismiss and annotation hotkey.

use crate::types::Definition;
use gtk4::prelude::*;
use gtk4::{
    gdk, glib, Application, ApplicationWindow, Box as GtkBox, CssProvider, Label, Orientation,
    ScrolledWindow,
};

/// Show a popup window near the cursor with the given definitions.
pub fn show(
    app: &Application,
    definitions: &[Definition],
    font_size: u32,
    timeout_ms: u64,
) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("wd")
        .default_width(380)
        .default_height(200)
        .decorated(false)
        .resizable(false)
        .build();

    // Apply CSS styling
    let css = CssProvider::new();
    css.load_from_string(&format!(
        r#"
        window {{
            background-color: #1e1e2e;
            border-radius: 12px;
            border: 1px solid #45475a;
        }}
        .popup-container {{
            padding: 16px;
        }}
        .word-header {{
            color: #cdd6f4;
            font-size: {}pt;
            font-weight: bold;
        }}
        .pos-label {{
            color: #a6adc8;
            font-size: {}pt;
            font-style: italic;
        }}
        .definition-text {{
            color: #bac2de;
            font-size: {}pt;
        }}
        .example-text {{
            color: #6c7086;
            font-size: {}pt;
            font-style: italic;
        }}
        .source-label {{
            color: #585b70;
            font-size: {}pt;
        }}
        .sense-number {{
            color: #89b4fa;
            font-size: {}pt;
            font-weight: bold;
        }}
        "#,
        font_size + 2,
        font_size,
        font_size,
        font_size.saturating_sub(1),
        font_size.saturating_sub(2),
        font_size,
    ));

    gtk4::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Could not get default display"),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

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

    // Focus-out to dismiss
    let focus_controller = gtk4::EventControllerFocus::new();
    let window_clone = window.clone();
    focus_controller.connect_leave(move |_| {
        window_clone.close();
    });
    window.add_controller(focus_controller);

    // Auto-dismiss timeout
    if timeout_ms > 0 {
        let window_clone = window.clone();
        glib::timeout_add_local_once(
            std::time::Duration::from_millis(timeout_ms),
            move || {
                window_clone.close();
            },
        );
    }

    window.present();
}
