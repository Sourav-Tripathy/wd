//! Reads ~/.config/wd/config.toml. Provides typed Config struct. Creates defaults if absent.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Palette colors for predefined theme presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub bg: &'static str,
    pub text: &'static str,
    pub header: &'static str,
    pub accent: &'static str,
    pub border: &'static str,
    pub example: &'static str,
}

pub const PALETTE_DARK: ThemePalette = ThemePalette {
    bg: "#1e1e2e",
    text: "#bac2de",
    header: "#cdd6f4",
    accent: "#89b4fa",
    border: "#45475a",
    example: "#6c7086",
};

pub const PALETTE_LIGHT: ThemePalette = ThemePalette {
    bg: "#f8f9fa",
    text: "#24292f",
    header: "#0969da",
    accent: "#1f883d",
    border: "#d0d7de",
    example: "#57606a",
};

pub const PALETTE_SEPIA: ThemePalette = ThemePalette {
    bg: "#fbf0d9",
    text: "#433422",
    header: "#5f4325",
    accent: "#8c5a2b",
    border: "#e0d4b8",
    example: "#7a6855",
};

pub const PALETTE_NORD: ThemePalette = ThemePalette {
    bg: "#2e3440",
    text: "#d8dee9",
    header: "#88c0d0",
    accent: "#81a1c1",
    border: "#4c566a",
    example: "#e5e9f0",
};

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

    /// Predefined theme preset ("dark", "light", "sepia", "nord").
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Optional custom background color override.
    #[serde(default)]
    pub popup_bg_color: Option<String>,

    /// Optional custom text color override.
    #[serde(default)]
    pub popup_text_color: Option<String>,

    /// Optional custom header color override.
    #[serde(default)]
    pub popup_header_color: Option<String>,

    /// Optional custom accent color override.
    #[serde(default)]
    pub popup_accent_color: Option<String>,

    /// Optional custom border color override.
    #[serde(default)]
    pub popup_border_color: Option<String>,

    /// Optional custom example sentences/grammar label color override.
    #[serde(default)]
    pub popup_example_color: Option<String>,
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

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            lookup_hotkey: default_lookup_hotkey(),
            pdf_auto_trigger: default_pdf_auto_trigger(),
            popup_timeout_ms: 0,
            popup_font_size: default_popup_font_size(),
            max_definitions: default_max_definitions(),
            theme: default_theme(),
            popup_bg_color: None,
            popup_text_color: None,
            popup_header_color: None,
            popup_accent_color: None,
            popup_border_color: None,
            popup_example_color: None,
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

    /// Returns the resolved theme palette based on the `theme` field.
    pub fn palette(&self) -> ThemePalette {
        match self.theme.trim().to_lowercase().as_str() {
            "light" => PALETTE_LIGHT,
            "sepia" => PALETTE_SEPIA,
            "nord" => PALETTE_NORD,
            _ => PALETTE_DARK,
        }
    }

    /// Background color (uses custom override if set, else theme preset).
    pub fn bg_color(&self) -> &str {
        self.popup_bg_color.as_deref().unwrap_or(self.palette().bg)
    }

    /// Definition description text color (uses custom override if set, else theme preset).
    pub fn text_color(&self) -> &str {
        self.popup_text_color
            .as_deref()
            .unwrap_or(self.palette().text)
    }

    /// Word header title color (uses custom override if set, else theme preset).
    pub fn header_color(&self) -> &str {
        self.popup_header_color
            .as_deref()
            .unwrap_or(self.palette().header)
    }

    /// Sense numbering color (uses custom override if set, else theme preset).
    pub fn accent_color(&self) -> &str {
        self.popup_accent_color
            .as_deref()
            .unwrap_or(self.palette().accent)
    }

    /// Window outline border color (uses custom override if set, else theme preset).
    pub fn border_color(&self) -> &str {
        self.popup_border_color
            .as_deref()
            .unwrap_or(self.palette().border)
    }

    /// Example sentences & grammar label color (uses custom override if set, else theme preset).
    pub fn example_color(&self) -> &str {
        self.popup_example_color
            .as_deref()
            .unwrap_or(self.palette().example)
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
# POPUP THEME SETTINGS
# ---------------------------------------------------------------------------
# Choose from built-in theme presets:
#   "dark"  - Deep slate/navy background with soft light text (default)
#   "light" - Clean off-white background with charcoal text for daytime reading
#   "sepia" - Warm parchment tone with espresso text for e-book readers
#   "nord"  - Arctic blue-grey palette
theme = "dark"

# Optional: Override individual colors (leave commented out to use preset colors)
# Supports standard HEX codes (e.g. "#1e1e2e") or CSS color names.
# popup_bg_color = "#1e1e2e"
# popup_text_color = "#bac2de"
# popup_header_color = "#cdd6f4"
# popup_accent_color = "#89b4fa"
# popup_border_color = "#45475a"
# popup_example_color = "#6c7086"
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
            && self.theme == default_theme()
            && self.popup_bg_color.is_none()
            && self.popup_text_color.is_none()
            && self.popup_header_color.is_none()
            && self.popup_accent_color.is_none()
            && self.popup_border_color.is_none()
            && self.popup_example_color.is_none();

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
    fn test_default_config_has_dark_palette() {
        let config = Config::default();
        assert_eq!(config.theme, "dark");
        assert_eq!(config.bg_color(), "#1e1e2e");
        assert_eq!(config.text_color(), "#bac2de");
        assert_eq!(config.header_color(), "#cdd6f4");
        assert_eq!(config.accent_color(), "#89b4fa");
        assert_eq!(config.border_color(), "#45475a");
        assert_eq!(config.example_color(), "#6c7086");
    }

    #[test]
    fn test_sepia_theme_preset() {
        let toml_str = r#"theme = "sepia""#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.theme, "sepia");
        assert_eq!(config.bg_color(), "#fbf0d9");
        assert_eq!(config.text_color(), "#433422");
        assert_eq!(config.header_color(), "#5f4325");
        assert_eq!(config.accent_color(), "#8c5a2b");
    }

    #[test]
    fn test_nord_theme_preset() {
        let toml_str = r#"theme = "nord""#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.theme, "nord");
        assert_eq!(config.bg_color(), "#2e3440");
        assert_eq!(config.text_color(), "#d8dee9");
        assert_eq!(config.header_color(), "#88c0d0");
    }

    #[test]
    fn test_light_theme_preset() {
        let toml_str = r#"theme = "light""#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.theme, "light");
        assert_eq!(config.bg_color(), "#f8f9fa");
        assert_eq!(config.text_color(), "#24292f");
    }

    #[test]
    fn test_custom_color_override_over_theme() {
        let toml_str = r##"
            theme = "sepia"
            popup_bg_color = "#ffffff"
        "##;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.bg_color(), "#ffffff"); // Custom override wins
        assert_eq!(config.text_color(), "#433422"); // Sepia preset retained for rest
    }

    #[test]
    fn test_default_toml_content_parses() {
        let content = Config::default_toml_content();
        let config: Config = toml::from_str(content).unwrap();
        assert_eq!(config.theme, "dark");
        assert_eq!(config.bg_color(), "#1e1e2e");
    }
}
