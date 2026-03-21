//! Core lookup function used by both CLI and daemon.
//! WordNet first, Wiktionary fallback, returns Definition struct.

use crate::config::Config;
use crate::types::{Definition, LookupError};
use crate::wiktionary;
use crate::wordnet::WordNetIndex;

/// Normalise input: lowercase, strip punctuation from edges, trim whitespace.
fn normalise(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

/// Perform a word lookup using the given WordNet index and config.
///
/// Resolution order:
/// 1. Normalise the input (lowercase, strip punctuation, trim).
/// 2. Check the in-memory WordNet index, return immediately if found.
/// 3. If not found, query the Wiktionary REST API silently.
/// 4. If still no result, return a NotFound error.
pub fn lookup(
    word: &str,
    wordnet: &WordNetIndex,
    config: &Config,
) -> Result<Vec<Definition>, LookupError> {
    let normalised = normalise(word);

    if normalised.is_empty() {
        return Err(LookupError::NotFound(word.to_string()));
    }

    // Step 1: Check WordNet
    if let Some(mut definitions) = wordnet.lookup(&normalised) {
        // Trim senses per the max_definitions config
        for def in &mut definitions {
            def.senses.truncate(config.max_definitions);
        }
        return Ok(definitions);
    }

    // Step 2: Wiktionary fallback (silent)
    match wiktionary::fetch(&normalised) {
        Ok(mut definitions) => {
            for def in &mut definitions {
                def.senses.truncate(config.max_definitions);
            }
            Ok(definitions)
        }
        Err(LookupError::NotFound(_)) => Err(LookupError::NotFound(normalised)),
        Err(LookupError::NetworkError(e)) => {
            log::debug!("Wiktionary network error: {}", e);
            Err(LookupError::NotFound(normalised))
        }
        Err(e) => {
            log::debug!("Wiktionary error: {}", e);
            Err(LookupError::NotFound(normalised))
        }
    }
}
