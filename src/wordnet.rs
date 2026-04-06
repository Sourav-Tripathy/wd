//! Parses WordNet dict files into an in-memory index on load.
//! Handles morphological variants (plurals, verb forms).

use crate::types::{Definition, LookupError, LookupSource, PoS, Sense};
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::Path;
use memmap2::Mmap;

/// In-memory WordNet index built from the dict files.
#[derive(Debug)]
pub struct WordNetIndex {
    /// Map from lemma (lowercase) to a list of entries grouped by PoS.
    entries: HashMap<String, Vec<WordNetEntry>>,
    /// Memory-mapped data files keyed by PoS character ('n', 'v', 'a', 'r').
    data_maps: HashMap<char, Mmap>,
}

#[derive(Debug, Clone)]
struct WordNetEntry {
    pos: PoS,
    pos_char: char,
    synset_offsets: Vec<u64>,
}

/// Morphological exception rules for English.
/// These handle common irregular forms.
static NOUN_EXCEPTIONS: &[(&str, &str)] = &[
    ("men", "man"),
    ("women", "woman"),
    ("children", "child"),
    ("mice", "mouse"),
    ("geese", "goose"),
    ("teeth", "tooth"),
    ("feet", "foot"),
    ("oxen", "ox"),
    ("people", "person"),
    ("dice", "die"),
    ("lice", "louse"),
];

static VERB_EXCEPTIONS: &[(&str, &str)] = &[
    ("was", "be"),
    ("were", "be"),
    ("been", "be"),
    ("had", "have"),
    ("has", "have"),
    ("did", "do"),
    ("done", "do"),
    ("went", "go"),
    ("gone", "go"),
    ("said", "say"),
    ("made", "make"),
    ("took", "take"),
    ("taken", "take"),
    ("came", "come"),
    ("saw", "see"),
    ("seen", "see"),
    ("got", "get"),
    ("gotten", "get"),
    ("gave", "give"),
    ("given", "give"),
    ("knew", "know"),
    ("known", "know"),
    ("thought", "think"),
    ("found", "find"),
    ("told", "tell"),
    ("felt", "feel"),
    ("left", "leave"),
    ("brought", "bring"),
    ("kept", "keep"),
    ("held", "hold"),
    ("wrote", "write"),
    ("written", "write"),
    ("stood", "stand"),
    ("heard", "hear"),
    ("ran", "run"),
    ("ate", "eat"),
    ("eaten", "eat"),
    ("spoke", "speak"),
    ("spoken", "speak"),
    ("broke", "break"),
    ("broken", "break"),
    ("drove", "drive"),
    ("driven", "drive"),
];

impl WordNetIndex {
    /// Create a new empty WordNet index.
    pub fn new() -> Self {
        WordNetIndex {
            entries: HashMap::new(),
            data_maps: HashMap::new(),
        }
    }

    /// Load the WordNet index from the dict directory.
    /// Mmaps `data.*` files and parses `index.*` files lazily.
    pub fn load(dict_dir: &Path) -> Result<Self, LookupError> {
        let mut index = WordNetIndex::new();

        let files = [
            ("index.noun", "data.noun", 'n'),
            ("index.verb", "data.verb", 'v'),
            ("index.adj", "data.adj", 'a'),
            ("index.adv", "data.adv", 'r'),
        ];

        for (index_file, data_file, pos_char) in &files {
            let index_path = dict_dir.join(index_file);
            let data_path = dict_dir.join(data_file);

            if !index_path.exists() || !data_path.exists() {
                log::debug!(
                    "WordNet file pair {}/{} not found, skipping",
                    index_file,
                    data_file
                );
                continue;
            }

            // Memory-map the data file
            let file = File::open(&data_path).map_err(|e| {
                LookupError::ParseError(format!("Failed to open {}: {}", data_path.display(), e))
            })?;
            let mmap = unsafe { Mmap::map(&file) }.map_err(|e| {
                LookupError::ParseError(format!("Failed to mmap {}: {}", data_path.display(), e))
            })?;
            index.data_maps.insert(*pos_char, mmap);

            // Load and parse index file
            let index_contents = fs::read_to_string(&index_path).map_err(|e| {
                LookupError::ParseError(format!("Failed to read {}: {}", index_path.display(), e))
            })?;

            index.parse_index_file(&index_contents, *pos_char)?;
        }

        // Also load exception files for morphological lookup
        index.load_exception_files(dict_dir);

        log::info!(
            "WordNet index loaded: {} unique lemmas",
            index.entries.len()
        );
        Ok(index)
    }

    /// Parse a WordNet index file into memory tracking offsets.
    fn parse_index_file(
        &mut self,
        index_contents: &str,
        pos_char: char,
    ) -> Result<(), LookupError> {
        for line in index_contents.lines() {
            // Skip comment lines (start with space or two spaces in WordNet format)
            if line.starts_with("  ") || line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 6 {
                continue;
            }

            let lemma = parts[0].replace('_', " ").to_lowercase();
            let synset_cnt: usize = match parts[2].parse() {
                Ok(n) => n,
                Err(_) => continue,
            };
            let ptr_cnt: usize = match parts[3].parse() {
                Ok(n) => n,
                Err(_) => continue,
            };

            // WordNet index format:
            // lemma pos synset_cnt p_cnt [ptr_symbol...] sense_cnt tagsense_cnt synset_offset...
            // - ptr_cnt items start at index 4.
            // - sense_cnt field is at index `4 + ptr_cnt`.
            // - tagsense_cnt field is at index `4 + ptr_cnt + 1`.
            // - synset offsets start at index `4 + ptr_cnt + 2`.
            let synset_start = 4 + ptr_cnt + 2; 

            // Collect synset offsets
            let mut synsets = Vec::new();
            for i in 0..synset_cnt {
                let idx = synset_start + i;
                if idx < parts.len() {
                    if let Ok(offset) = parts[idx].parse::<u64>() {
                        synsets.push(offset);
                    }
                }
            }

            if !synsets.is_empty() {
                let entry = WordNetEntry {
                    pos: PoS::from_wordnet_char(pos_char),
                    pos_char,
                    synset_offsets: synsets,
                };
                self.entries
                    .entry(lemma)
                    .or_insert_with(Vec::new)
                    .push(entry);
            }
        }

        Ok(())
    }

    /// Lazily parse a single data line from memory mapped byte buffer.
    fn read_sense_lazy(&self, pos_char: char, offset: u64) -> Option<Sense> {
        let mmap = self.data_maps.get(&pos_char)?;
        if offset as usize >= mmap.len() {
            return None;
        }

        let slice = &mmap[offset as usize..];
        let newline_pos = slice.iter().position(|&b| b == b'\n').unwrap_or(slice.len());
        let line = std::str::from_utf8(&slice[..newline_pos]).ok()?;

        // Format: synset_offset lex_filenum ss_type w_cnt word lex_id ... | gloss
        let parts: Vec<&str> = line.splitn(2, " | ").collect();
        if parts.len() < 2 {
            return None;
        }
        let gloss = parts[1].trim();

        let (definition, example) = Self::parse_gloss(gloss);
        Some(Sense { definition, example })
    }

    /// Parse a WordNet gloss into (definition, optional example).
    fn parse_gloss(gloss: &str) -> (String, Option<String>) {
        // The gloss may contain examples in quotes after a semicolon
        if let Some(semi_pos) = gloss.find(';') {
            let definition = gloss[..semi_pos].trim().to_string();
            let rest = gloss[semi_pos + 1..].trim();

            // Look for a quoted example
            if let Some(start) = rest.find('"') {
                if let Some(end) = rest[start + 1..].find('"') {
                    let example = rest[start + 1..start + 1 + end].to_string();
                    return (definition, Some(example));
                }
            }

            // If there's text after semicolon but no quotes, use it as-is
            if !rest.is_empty() {
                return (definition, Some(rest.trim_matches('"').to_string()));
            }

            return (definition, None);
        }

        (gloss.trim().to_string(), None)
    }

    /// Load WordNet exception files for morphological variants.
    fn load_exception_files(&mut self, dict_dir: &Path) {
        let exc_files = ["noun.exc", "verb.exc", "adj.exc", "adv.exc"];
        for file in &exc_files {
            let path = dict_dir.join(file);
            if let Ok(contents) = fs::read_to_string(&path) {
                for line in contents.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let inflected = parts[0].to_lowercase();
                        let base = parts[1].to_lowercase();
                        // If the base form exists in our index but the inflected doesn't,
                        // add the inflected form as an alias.
                        if self.entries.contains_key(&base) && !self.entries.contains_key(&inflected)
                        {
                            if let Some(entries) = self.entries.get(&base).cloned() {
                                self.entries.insert(inflected, entries);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Look up a word in the index, trying morphological variants if needed.
    pub fn lookup(&self, word: &str) -> Option<Vec<Definition>> {
        let normalised = word.to_lowercase().trim().to_string();

        // Direct lookup
        if let Some(ref definitions) = self.lookup_exact(&normalised) {
            if !definitions.is_empty() {
                return Some(definitions.clone());
            }
        }

        // Try morphological variants
        for variant in self.morphological_variants(&normalised) {
            if let Some(ref definitions) = self.lookup_exact(&variant) {
                if !definitions.is_empty() {
                    return Some(definitions.clone());
                }
            }
        }

        None
    }

    /// Exact lookup of a normalised word.
    fn lookup_exact(&self, word: &str) -> Option<Vec<Definition>> {
        self.entries.get(word).map(|entries| {
            entries
                .iter()
                .filter_map(|e| {
                    let mut senses = Vec::new();
                    for &offset in &e.synset_offsets {
                        if let Some(s) = self.read_sense_lazy(e.pos_char, offset) {
                            senses.push(s);
                        }
                    }
                    if senses.is_empty() {
                        None
                    } else {
                        Some(Definition {
                            word: word.to_string(),
                            pos: e.pos.clone(),
                            senses,
                            source: LookupSource::WordNet,
                        })
                    }
                })
                .collect()
        })
    }

    /// Generate morphological variants of a word for lookup.
    fn morphological_variants(&self, word: &str) -> Vec<String> {
        let mut variants = Vec::new();

        // Check hardcoded exception tables
        for (inflected, base) in NOUN_EXCEPTIONS {
            if word == *inflected {
                variants.push(base.to_string());
            }
        }
        for (inflected, base) in VERB_EXCEPTIONS {
            if word == *inflected {
                variants.push(base.to_string());
            }
        }

        // Common noun plural rules
        if word.ends_with('s') {
            // Remove trailing 's'
            variants.push(word[..word.len() - 1].to_string());

            // 'es' -> ''
            if word.ends_with("es") {
                variants.push(word[..word.len() - 2].to_string());

                // 'ies' -> 'y'
                if word.ends_with("ies") {
                    let mut v = word[..word.len() - 3].to_string();
                    v.push('y');
                    variants.push(v);
                }

                // 'ves' -> 'f' / 'fe' (e.g. knives -> knife, leaves -> leaf)
                if word.ends_with("ves") {
                    let stem = &word[..word.len() - 3];
                    variants.push(format!("{}f", stem));
                    variants.push(format!("{}fe", stem));
                }

                // 'ses', 'xes', 'zes' -> remove 'es'
                if word.ends_with("ses") || word.ends_with("xes") || word.ends_with("zes") {
                    variants.push(word[..word.len() - 2].to_string());
                }
            }
        }

        // Common verb forms
        if word.ends_with("ing") {
            let stem = &word[..word.len() - 3];
            variants.push(stem.to_string());
            variants.push(format!("{}e", stem));
            if stem.len() >= 2 {
                let bytes = stem.as_bytes();
                if bytes[bytes.len() - 1] == bytes[bytes.len() - 2] {
                    variants.push(stem[..stem.len() - 1].to_string());
                }
            }
        }

        if word.ends_with("ed") {
            let stem = &word[..word.len() - 2];
            variants.push(stem.to_string());
            variants.push(format!("{}e", stem));
            if stem.len() >= 2 {
                let bytes = stem.as_bytes();
                if bytes[bytes.len() - 1] == bytes[bytes.len() - 2] {
                    variants.push(stem[..stem.len() - 1].to_string());
                }
            }
        }

        if word.ends_with("er") {
            let stem = &word[..word.len() - 2];
            variants.push(stem.to_string());
            variants.push(format!("{}e", stem)); // e.g., larger -> large
            
            // 'ier' -> 'y' (e.g. happier -> happy)
            if word.ends_with("ier") {
                let base = &word[..word.len() - 3];
                variants.push(format!("{}y", base));
            }
        }

        if word.ends_with("est") {
            let stem = &word[..word.len() - 3];
            variants.push(stem.to_string());
            variants.push(format!("{}e", stem));
            
            // 'iest' -> 'y'
            if word.ends_with("iest") {
                let base = &word[..word.len() - 4];
                variants.push(format!("{}y", base));
            }
        }

        if word.ends_with("ly") {
            let stem = &word[..word.len() - 2];
            variants.push(stem.to_string());
        }

        variants.sort();
        variants.dedup();
        variants
    }
}
