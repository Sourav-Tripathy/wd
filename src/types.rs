//! Shared types: Definition, Sense, PoS, LookupSource, LookupError.

use std::fmt;

/// Part of speech for a word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoS {
    Noun,
    Verb,
    Adjective,
    Adverb,
    Other(String),
}

impl fmt::Display for PoS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoS::Noun => write!(f, "noun"),
            PoS::Verb => write!(f, "verb"),
            PoS::Adjective => write!(f, "adjective"),
            PoS::Adverb => write!(f, "adverb"),
            PoS::Other(s) => write!(f, "{}", s),
        }
    }
}

impl PoS {
    /// Parse a WordNet part-of-speech character into a `PoS`.
    pub fn from_wordnet_char(c: char) -> Self {
        match c {
            'n' => PoS::Noun,
            'v' => PoS::Verb,
            'a' | 's' => PoS::Adjective,
            'r' => PoS::Adverb,
            _ => PoS::Other(c.to_string()),
        }
    }

    /// Parse a part-of-speech string (e.g., from Wiktionary).
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "noun" => PoS::Noun,
            "verb" => PoS::Verb,
            "adjective" | "adj" => PoS::Adjective,
            "adverb" | "adv" => PoS::Adverb,
            other => PoS::Other(other.to_string()),
        }
    }
}

/// A single definition sense with an optional usage example.
#[derive(Debug, Clone)]
pub struct Sense {
    pub definition: String,
    pub example: Option<String>,
}

/// A word definition, potentially containing multiple parts of speech and senses.
#[derive(Debug, Clone)]
pub struct Definition {
    pub word: String,
    pub pos: PoS,
    pub senses: Vec<Sense>,
    pub source: LookupSource,
}

/// Where the definition came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupSource {
    WordNet,
    Wiktionary,
}

impl fmt::Display for LookupSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookupSource::WordNet => write!(f, "WordNet"),
            LookupSource::Wiktionary => write!(f, "Wiktionary"),
        }
    }
}

/// Errors that can occur during lookup.
#[derive(Debug)]
pub enum LookupError {
    /// Word was not found in any source.
    NotFound(String),
    /// Network error when querying Wiktionary.
    NetworkError(String),
    /// Error parsing WordNet data.
    ParseError(String),
    /// I/O error.
    IoError(std::io::Error),
}

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookupError::NotFound(word) => {
                write!(f, "wd: no definition found for '{}'", word)
            }
            LookupError::NetworkError(msg) => {
                write!(f, "wd: network error: {}", msg)
            }
            LookupError::ParseError(msg) => {
                write!(f, "wd: parse error: {}", msg)
            }
            LookupError::IoError(e) => {
                write!(f, "wd: I/O error: {}", e)
            }
        }
    }
}

impl std::error::Error for LookupError {}

impl From<std::io::Error> for LookupError {
    fn from(e: std::io::Error) -> Self {
        LookupError::IoError(e)
    }
}
