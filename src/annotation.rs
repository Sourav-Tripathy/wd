//! Sends highlight and note to Evince or Okular over DBus via zbus.
//! No-op if viewer DBus is absent.

use crate::types::Definition;

/// Attempt to annotate the currently selected text in a PDF viewer.
///
/// Tries Evince first, then Okular. If neither DBus interface is available,
/// this is a no-op and returns a status message.
pub async fn write(
    definitions: &[Definition],
    include_example: bool,
) -> Result<String, String> {
    // Try Evince first
    match annotate_evince(definitions, include_example).await {
        Ok(msg) => return Ok(msg),
        Err(e) => log::debug!("Evince annotation not available: {}", e),
    }

    // Try Okular
    match annotate_okular(definitions, include_example).await {
        Ok(msg) => return Ok(msg),
        Err(e) => log::debug!("Okular annotation not available: {}", e),
    }

    Err("Annotation unavailable: no supported PDF viewer DBus interface found".to_string())
}

/// Build the annotation note text from definitions.
fn build_note_text(definitions: &[Definition], include_example: bool) -> String {
    let mut note = String::new();

    for def in definitions {
        note.push_str(&format!("{} ({})\n", def.word, def.pos));

        for (i, sense) in def.senses.iter().enumerate() {
            note.push_str(&format!("  {}. {}\n", i + 1, sense.definition));

            if include_example {
                if let Some(ref example) = sense.example {
                    note.push_str(&format!("     \"{}\"\n", example));
                }
            }
        }

        note.push_str(&format!("[{}]\n", def.source));
    }

    note
}

/// Attempt to add annotation via Evince's DBus interface.
async fn annotate_evince(
    definitions: &[Definition],
    include_example: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let connection = zbus::Connection::session().await?;

    // Evince exposes org.gnome.evince.Daemon on the session bus
    // The actual annotation API is per-window via org.gnome.evince.Window
    let proxy = zbus::fdo::DBusProxy::new(&connection).await?;

    // List names to find Evince instances
    let names = proxy.list_names().await?;

    let evince_name = names
        .iter()
        .find(|n| n.as_str().starts_with("org.gnome.evince"))
        .ok_or("No Evince instance found on DBus")?;

    let note_text = build_note_text(definitions, include_example);

    // Call the annotation method
    // Note: Evince's DBus annotation API is limited. In practice, we create
    // a text annotation at the current page position.
    let _result: () = connection
        .call_method(
            Some(evince_name.as_str()),
            "/org/gnome/evince/Window/0",
            Some("org.gnome.evince.Window"),
            "AddAnnotation",
            &(note_text.as_str(),),
        )
        .await?
        .body()
        .deserialize()?;

    Ok("Annotation added to Evince".to_string())
}

/// Attempt to add annotation via Okular's DBus interface.
async fn annotate_okular(
    definitions: &[Definition],
    include_example: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let connection = zbus::Connection::session().await?;
    let proxy = zbus::fdo::DBusProxy::new(&connection).await?;

    let names = proxy.list_names().await?;

    let okular_name = names
        .iter()
        .find(|n| n.as_str().starts_with("org.kde.okular"))
        .ok_or("No Okular instance found on DBus")?;

    let note_text = build_note_text(definitions, include_example);

    // Okular's DBus interface for annotations
    let _result: () = connection
        .call_method(
            Some(okular_name.as_str()),
            "/okular",
            Some("org.kde.okular"),
            "addNote",
            &(note_text.as_str(),),
        )
        .await?
        .body()
        .deserialize()?;

    Ok("Annotation added to Okular".to_string())
}
