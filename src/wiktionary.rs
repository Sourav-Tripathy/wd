//! Queries Wiktionary REST API via ureq. Parses JSON response into Definition struct.

use crate::types::{Definition, LookupError, LookupSource, PoS, Sense};

/// The Wiktionary REST API base URL.
const WIKTIONARY_API_BASE: &str =
    "https://en.wiktionary.org/api/rest_v1/page/definition";

/// Fetch a word definition from Wiktionary.
/// This is the fallback when WordNet has no result.
pub fn fetch(word: &str) -> Result<Vec<Definition>, LookupError> {
    let url = format!("{}/{}", WIKTIONARY_API_BASE, urlencoded(word));

    let response = ureq::get(&url)
        .set("Accept", "application/json")
        .set(
            "User-Agent",
            "wd/0.1.0 (Linux word lookup tool; mailto:contact@example.com)",
        )
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => LookupError::NotFound(word.to_string()),
            ureq::Error::Status(code, _) => {
                LookupError::NetworkError(format!("HTTP {}", code))
            }
            ureq::Error::Transport(t) => {
                LookupError::NetworkError(format!("Transport error: {}", t))
            }
        })?;

    let json: serde_json::Value = response
        .into_json()
        .map_err(|e| LookupError::ParseError(format!("JSON parse error: {}", e)))?;

    parse_wiktionary_response(word, &json)
}

/// Parse the Wiktionary REST API JSON response into Definition structs.
fn parse_wiktionary_response(
    word: &str,
    json: &serde_json::Value,
) -> Result<Vec<Definition>, LookupError> {
    let mut definitions = Vec::new();

    // The response has an "en" key for English definitions
    let en_entries = json
        .get("en")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LookupError::NotFound(word.to_string()))?;

    for entry in en_entries {
        let pos_str = entry
            .get("partOfSpeech")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let pos = PoS::from_str_lossy(pos_str);

        let definitions_array = match entry.get("definitions").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => continue,
        };

        let mut senses = Vec::new();
        for def_entry in definitions_array {
            let definition = match def_entry.get("definition").and_then(|v| v.as_str()) {
                Some(d) => strip_html_tags(d),
                None => continue,
            };

            // Skip empty definitions
            if definition.trim().is_empty() {
                continue;
            }

            let example = def_entry
                .get("examples")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|ex| ex.as_str())
                .map(|s| strip_html_tags(s));

            senses.push(Sense {
                definition,
                example,
            });
        }

        if !senses.is_empty() {
            definitions.push(Definition {
                word: word.to_string(),
                pos,
                senses,
                source: LookupSource::Wiktionary,
            });
        }
    }

    if definitions.is_empty() {
        return Err(LookupError::NotFound(word.to_string()));
    }

    Ok(definitions)
}

/// Strip basic HTML tags from Wiktionary responses.
fn strip_html_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut inside_tag = false;

    for ch in input.chars() {
        if ch == '<' {
            inside_tag = true;
        } else if ch == '>' {
            inside_tag = false;
        } else if !inside_tag {
            result.push(ch);
        }
    }

    // Decode common HTML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

/// Simple URL encoding for the word (handles spaces and special chars).
fn urlencoded(word: &str) -> String {
    let mut encoded = String::new();
    for byte in word.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}
