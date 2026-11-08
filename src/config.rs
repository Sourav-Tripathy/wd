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

    /// Auto-lookup text selected inside PDF viewers (Papers, Evince, or Okular).
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

    /// Background color of the definition popup window.
    #[serde(default = "default_popup_bg_color")]
    pub popup_bg_color: String,

    /// Main definition text color.
    #[serde(default = "default_popup_text_color")]
    pub popup_text_color: String,

    /// Looked up word title / header color.
    #[serde(default = "default_popup_header_color")]
    pub popup_header_color: String,

    /// Accent color for numbering (1., 2., 3.).
    #[serde(default = "default_popup_accent_color")]
    pub popup_accent_color: String,

    /// Outline border color of the popup window.
    #[serde(default = "default_popup_border_color")]
    pub popup_border_color: String,

    /// Example sentences and grammar label text color.
    #[serde(default = "default_popup_example_color")]
    pub popup_example_color: String,
}

fn default_lookup_hotkey() -> String {
    "Ctrl+Alt+W".to_string()
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

fn default_popup_bg_color() -> String {
    "#1e1e2e".to_string()
}

fn default_popup_text_color() -> String {
    "#bac2de".to_string()
}

fn default_popup_header_color() -> String {
    "#cdd6f4".to_string()
}

fn default_popup_accent_color() -> String {
    "#89b4fa".to_string()
}

fn default_popup_border_color() -> String {
    "#45475a".to_string()
}

fn default_popup_example_color() -> String {
    "#6c7086".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            lookup_hotkey: default_lookup_hotkey(),
            pdf_auto_trigger: default_pdf_auto_trigger(),
            popup_timeout_ms: 0,
            popup_font_size: default_popup_font_size(),
            max_definitions: default_max_definitions(),
            popup_bg_color: default_popup_bg_color(),
            popup_text_color: default_popup_text_color(),
            popup_header_color: default_popup_header_color(),
            popup_accent_color: default_popup_accent_color(),
            popup_border_color: default_popup_border_color(),
            popup_example_color: default_popup_example_color(),
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

    /// Default TOML configuration with rich comments explaining all available variables and colors.
    pub fn default_toml_content() -> &'static str {
        r##"# wd configuration file
# Location: ~/.config/wd/config.toml

# ---------------------------------------------------------------------------
# GENERAL SETTINGS
# ---------------------------------------------------------------------------

# Global hotkey to look up the current text selection from any application.
lookup_hotkey = "Ctrl+Alt+W"

# Auto-lookup text selected inside PDF viewers (Papers, Evince, Okular).
# When true, highlighting a word inside a PDF viewer instantly opens the popup.
pdf_auto_trigger = true

# Auto-dismiss popup after N milliseconds (e.g. 5000 for 5s). Set to 0 to keep open until dismissed.
popup_timeout_ms = 0

# Base font size for popup text in points.
popup_font_size = 13

# Maximum definitions shown per word sense.
max_definitions = 3

# ---------------------------------------------------------------------------
# POPUP COLOR SETTINGS
# ---------------------------------------------------------------------------
# Customize the appearance of the popup window.
# Supports standard HEX codes (e.g. "#1e1e2e") or standard CSS color names.
#
# Available Color Variables:
#   popup_bg_color      - Popup window background color (default: "#1e1e2e")
#   popup_text_color    - Main definition description text (default: "#bac2de")
#   popup_header_color  - Looked up word title header (default: "#cdd6f4")
#   popup_accent_color  - Sense numbering indicators (1., 2., 3.) (default: "#89b4fa")
#   popup_border_color  - Popup window outer border line (default: "#45475a")
#   popup_example_color - Example sentences & grammar part-of-speech (default: "#6c7086")
#
# NOTE: The daemon automatically verifies contrast. If you select a light background
# without changing text colors, it will automatically ensure high-contrast readability.

popup_bg_color = "#1e1e2e"
popup_text_color = "#bac2de"
popup_header_color = "#cdd6f4"
popup_accent_color = "#89b4fa"
popup_border_color = "#45475a"
popup_example_color = "#6c7086"

# ---------------------------------------------------------------------------
# POPULAR THEME PRESETS (Copy and paste any block above):
# ---------------------------------------------------------------------------
# Clean Light Theme:
#   popup_bg_color = "#ffffff"
#   popup_text_color = "#24292f"
#   popup_header_color = "#0969da"
#   popup_accent_color = "#1f883d"
#   popup_border_color = "#d0d7de"
#   popup_example_color = "#57606a"
#
# Gruvbox Dark:
#   popup_bg_color = "#282828"
#   popup_text_color = "#ebdbb2"
#   popup_header_color = "#fabd2f"
#   popup_accent_color = "#fe8019"
#   popup_border_color = "#504945"
#   popup_example_color = "#928374"
#
# Tokyo Night:
#   popup_bg_color = "#1a1b26"
#   popup_text_color = "#a9b1d6"
#   popup_header_color = "#7aa2f7"
#   popup_accent_color = "#bb9af7"
#   popup_border_color = "#414868"
#   popup_example_color = "#565f89"
"##
    }

    /// Save current configuration to disk.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // If the configuration matches default, preserve the helpful comments
        let is_default = self.lookup_hotkey == default_lookup_hotkey()
            && self.pdf_auto_trigger == default_pdf_auto_trigger()
            && self.popup_timeout_ms == 0
            && self.popup_font_size == default_popup_font_size()
            && self.max_definitions == default_max_definitions()
            && self.popup_bg_color == default_popup_bg_color()
            && self.popup_text_color == default_popup_text_color()
            && self.popup_header_color == default_popup_header_color()
            && self.popup_accent_color == default_popup_accent_color()
            && self.popup_border_color == default_popup_border_color()
            && self.popup_example_color == default_popup_example_color();

        let contents = if is_default {
            Self::default_toml_content().to_string()
        } else {
            toml::to_string_pretty(self)?
        };

        fs::write(&config_path, contents)?;
        log::info!("Config written to {}", config_path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_has_colors() {
        let config = Config::default();
        assert_eq!(config.popup_bg_color, "#1e1e2e");
        assert_eq!(config.popup_text_color, "#bac2de");
        assert_eq!(config.popup_header_color, "#cdd6f4");
        assert_eq!(config.popup_accent_color, "#89b4fa");
        assert_eq!(config.popup_border_color, "#45475a");
        assert_eq!(config.popup_example_color, "#6c7086");
    }

    #[test]
    fn test_deserialize_custom_colors() {
        let toml_str = r##"
            lookup_hotkey = "Ctrl+Alt+D"
            popup_bg_color = "#ffffff"
            popup_text_color = "#000000"
        "##;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.lookup_hotkey, "Ctrl+Alt+D");
        assert_eq!(config.popup_bg_color, "#ffffff");
        assert_eq!(config.popup_text_color, "#000000");
        // Defaults preserved for omitted fields
        assert_eq!(config.popup_header_color, "#cdd6f4");
    }

    #[test]
    fn test_default_toml_content_parses() {
        let content = Config::default_toml_content();
        let config: Config = toml::from_str(content).unwrap();
        assert_eq!(config.popup_bg_color, "#1e1e2e");
        assert_eq!(config.popup_text_color, "#bac2de");
    }
}
