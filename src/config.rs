//! Reads ~/.config/wd/config.toml. Provides typed Config struct. Creates defaults if absent.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application configuration, matching all keys in the architecture spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Global hotkey to look up current X11 PRIMARY selection.
    #[serde(default = "default_lookup_hotkey")]
    pub lookup_hotkey: String,

    /// Hotkey to save definition as PDF annotation while popup is open.
    #[serde(default = "default_annotate_hotkey")]
    pub annotate_hotkey: String,

    /// Auto-lookup text selected inside Evince or Okular.
    #[serde(default = "default_pdf_auto_trigger")]
    pub pdf_auto_trigger: bool,

    /// Auto-dismiss popup after N ms. 0 means never.
    #[serde(default)]
    pub popup_timeout_ms: u64,

    /// Popup font size in points.
    #[serde(default = "default_popup_font_size")]
    pub popup_font_size: u32,

    /// Maximum definitions shown per word sense.
    #[serde(default = "default_max_definitions")]
    pub max_definitions: usize,

    /// Include a usage example in the annotation note body.
    #[serde(default = "default_annotate_include_example")]
    pub annotate_include_example: bool,
}

fn default_lookup_hotkey() -> String {
    "Ctrl+Alt+W".to_string()
}

fn default_annotate_hotkey() -> String {
    "Ctrl+Alt+S".to_string()
}

fn default_pdf_auto_trigger() -> bool {
    true
}

fn default_popup_font_size() -> u32 {
    13
}

fn default_max_definitions() -> usize {
    3
}

fn default_annotate_include_example() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Config {
            lookup_hotkey: default_lookup_hotkey(),
            annotate_hotkey: default_annotate_hotkey(),
            pdf_auto_trigger: default_pdf_auto_trigger(),
            popup_timeout_ms: 0,
            popup_font_size: default_popup_font_size(),
            max_definitions: default_max_definitions(),
            annotate_include_example: default_annotate_include_example(),
        }
    }
}

impl Config {
    /// Returns the path to the config file: ~/.config/wd/config.toml
    pub fn config_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("wd");
        config_dir.join("config.toml")
    }

    /// Returns the data directory path: ~/.local/share/wd/
    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("wd")
    }

    /// Returns the WordNet data directory path.
    /// Checks /usr/share/wordnet/ first, then ~/.local/share/wd/wordnet/.
    pub fn wordnet_dir() -> PathBuf {
        let system_path = PathBuf::from("/usr/share/wordnet");
        if system_path.exists() {
            return system_path;
        }
        Self::data_dir().join("wordnet")
    }

    /// Load configuration from disk, creating defaults if the file doesn't exist.
    pub fn load() -> Self {
        let config_path = Self::config_path();

        if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(contents) => match toml::from_str::<Config>(&contents) {
                    Ok(config) => return config,
                    Err(e) => {
                        log::warn!(
                            "Failed to parse config at {}: {}. Using defaults.",
                            config_path.display(),
                            e
                        );
                    }
                },
                Err(e) => {
                    log::warn!(
                        "Failed to read config at {}: {}. Using defaults.",
                        config_path.display(),
                        e
                    );
                }
            }
        } else {
            // Create default config file
            let config = Config::default();
            if let Err(e) = config.save() {
                log::warn!("Failed to write default config: {}", e);
            }
            return config;
        }

        Config::default()
    }

    /// Save current configuration to disk.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        fs::write(&config_path, contents)?;
        log::info!("Config written to {}", config_path.display());
        Ok(())
    }
}
