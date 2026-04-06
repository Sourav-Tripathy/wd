//! CLI mode handler. Calls lookup, formats output to stdout, exits.

use crate::config::Config;
use crate::lookup;
use crate::types::LookupError;
use crate::wordnet::WordNetIndex;
use std::process;

/// Run a CLI word lookup. Prints formatted output and exits.
pub fn run(word: &str, config: &Config) -> Result<(), ()> {
    // Load WordNet index
    let wordnet_dir = Config::wordnet_dir();
    let wordnet = match WordNetIndex::load(&wordnet_dir) {
        Ok(idx) => idx,
        Err(e) => {
            log::debug!("WordNet not available: {}", e);
            // Continue with an empty index; will fall back to Wiktionary
            WordNetIndex::new()
        }
    };

    match lookup::lookup(word, &wordnet, config) {
        Ok(definitions) => {
            print_definitions(&definitions);
            Ok(())
        }
        Err(LookupError::NotFound(w)) => {
            eprintln!("wd: no definition found for '{}'", w);
            Err(())
        }
        Err(e) => {
            eprintln!("{}", e);
            Err(())
        }
    }
}

/// Format and print definitions to stdout 
///
/// Example output:
/// ```text
/// think  (verb)
///
///   1. Judge or regard; look upon; judge.
///
///      "She thinks he is a saint"
///
///   2. Expect, believe, or suppose.
///
///      "I thought to find her in a bad state"
///
/// [WordNet]
/// ```
fn print_definitions(definitions: &[crate::types::Definition]) {
    for (def_idx, def) in definitions.iter().enumerate() {
        if def_idx > 0 {
            println!();
        }

        // Word and part of speech header
        println!("{}  ({})", def.word, def.pos);
        println!();

        // Numbered senses
        for (i, sense) in def.senses.iter().enumerate() {
            println!("  {}. {}", i + 1, sense.definition);

            if let Some(ref example) = sense.example {
                println!();
                println!("     \"{}\"", example);
            }

            println!();
        }

        // Source footer
        println!("[{}]", def.source);
    }
}
