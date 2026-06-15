//! Configuration system for open_music.
//!
//! Configuration is stored as TOML at `$CONFIG_DIR/open_music/config.toml`
//! and is loaded once at startup.  A file-watcher (via [`notify`]) can be
//! started with [`Config::watch`] to hot-reload changes.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Read(#[source] io::Error),

    #[error("failed to write config file: {0}")]
    Write(#[source] io::Error),

    #[error("failed to parse config TOML: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("failed to serialise config TOML: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("unknown config key path: '{0}'")]
    UnknownKey(String),

    #[error("invalid value type for key '{key}': expected {expected}")]
    InvalidType { key: String, expected: String },

    #[error("file watcher error: {0}")]
    Watch(#[source] notify::Error),

    #[error("config directory does not exist and could not be created: {0}")]
    Dirs(#[source] io::Error),
}

pub type ConfigResult<T> = Result<T, ConfigError>;

// ---------------------------------------------------------------------------
// Config sub-structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Root directory for the music library data (database, cache, …).
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    /// Automatically persist library state on changes.
    #[serde(default = "default_true")]
    pub auto_save: bool,

    /// Interval in seconds between auto-saves (when `auto_save` is true).
    #[serde(default = "default_auto_save_interval")]
    pub auto_save_interval_secs: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Default playback volume clamped to `[0.0, 1.0]`.
    #[serde(default = "default_volume")]
    pub default_volume: f32,

    /// Preferred audio output device (platform-dependent name).
    #[serde(default)]
    pub output_device: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiConfig {
    /// Colour theme name, e.g. `"dracula"`, `"nord"`, `"default"`.
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Base font size in points for the TUI.
    #[serde(default = "default_font_size")]
    pub font_size: u16,
}

/// Top-level configuration that is serialised as TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub audio: AudioConfig,

    #[serde(default)]
    pub ui: UiConfig,

    /// Per-plugin configuration.  Keys are plugin names; values are
    /// arbitrary TOML tables that each plugin is responsible for
    /// interpreting.
    #[serde(default)]
    pub plugins: HashMap<String, toml::Value>,

    /// User-defined keybindings: `action → key-sequence`.
    #[serde(default)]
    pub keybinds: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Default helpers (used by `serde(default = ...)`)
// ---------------------------------------------------------------------------

fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("open_music")
}

fn default_true() -> bool {
    true
}

fn default_auto_save_interval() -> u64 {
    300 // 5 minutes
}

fn default_volume() -> f32 {
    0.8
}

fn default_theme() -> String {
    "default".to_string()
}

fn default_font_size() -> u16 {
    14
}

// ---------------------------------------------------------------------------
// Config impl
// ---------------------------------------------------------------------------

impl Config {
    // -- Construction -------------------------------------------------------

    /// Sensible defaults matching `config.toml` written by [`Config::save_default`].
    pub fn default() -> Self {
        Self {
            general: GeneralConfig {
                data_dir: default_data_dir(),
                auto_save: true,
                auto_save_interval_secs: 300,
            },
            audio: AudioConfig {
                default_volume: 0.8,
                output_device: None,
            },
            ui: UiConfig {
                theme: "default".into(),
                font_size: 14,
            },
            plugins: HashMap::new(),
            keybinds: HashMap::new(),
        }
    }

    // -- Path helpers -------------------------------------------------------

    /// Directory containing `config.toml`.
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("open_music")
    }

    /// Full path to `config.toml`.
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    // -- Persistence --------------------------------------------------------

    /// Load configuration from the default path.
    ///
    /// If the file does not exist a default configuration is created, saved,
    /// and returned.  If the file exists but cannot be parsed an error is
    /// returned.
    pub fn load() -> ConfigResult<Self> {
        let path = Self::config_path();

        if !path.exists() {
            info!(
                "no config found at '{}' – creating default",
                path.display()
            );
            let cfg = Self::default();
            cfg.save()?;
            return Ok(cfg);
        }

        let raw = fs::read_to_string(&path).map_err(ConfigError::Read)?;
        let cfg: Config = toml::from_str(&raw)?;
        debug!("loaded config from '{}'", path.display());
        Ok(cfg)
    }

    /// Write the current configuration to the default path.
    ///
    /// Parent directories are created automatically if they are missing.
    pub fn save(&self) -> ConfigResult<()> {
        let path = Self::config_path();

        // Ensure the directory exists.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ConfigError::Dirs)?;
        }

        let toml_str = toml::to_string_pretty(self)?;
        fs::write(&path, toml_str).map_err(ConfigError::Write)?;
        debug!("saved config to '{}'", path.display());
        Ok(())
    }

    // -- Hot-reload ---------------------------------------------------------

    /// Spawn a background thread that watches the config file for changes.
    ///
    /// Whenever the file is modified (created, written, or removed-and-
    /// recreated) the provided `callback` is invoked.  The callback runs on
    /// the watcher's event thread, so it should not block for long.
    ///
    /// Returns a handle that, when dropped, stops the watcher.
    pub fn watch<F>(callback: F) -> ConfigResult<ConfigWatcherHandle>
    where
        F: Fn() + Send + 'static,
    {
        use notify::event::EventKind;
        use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};

        let config_path = Self::config_path();
        let config_dir = config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        // The watcher sends events through this channel.
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            NotifyConfig::default(),
        )
        .map_err(ConfigError::Watch)?;

        // Watch the config directory non-recursively (the watcher sees
        // modifications to any file in that directory – we filter below).
        watcher
            .watch(&config_dir, RecursiveMode::NonRecursive)
            .map_err(ConfigError::Watch)?;

        let handle = ConfigWatcherHandle {
            _watcher: Some(watcher),
        };

        // Background thread: wait for events and filter for our file.
        thread::Builder::new()
            .name("config-watcher".into())
            .spawn(move || {
                // The config file name we care about.
                let file_name = config_path
                    .file_name()
                    .map(|f| f.to_os_string());

                loop {
                    match rx.recv() {
                        Ok(Ok(event)) => {
                            // Only react to data-change events on our file.
                            let relevant = event.paths.iter().any(|p| {
                                p.file_name().map(|n| Some(n) == file_name.as_deref())
                                    .unwrap_or(false)
                            });

                            if !relevant {
                                continue;
                            }

                            let is_modify = matches!(
                                event.kind,
                                EventKind::Create(_)
                                    | EventKind::Modify(_)
                                    | EventKind::Remove(_)
                            );

                            if is_modify {
                                debug!("config.toml changed on disk – triggering callback");
                                callback();
                            }
                        }
                        Ok(Err(e)) => {
                            warn!("config watcher error: {e}");
                        }
                        Err(mpsc::RecvError) => {
                            // Channel closed → watcher dropped.
                            debug!("config watcher shutting down");
                            break;
                        }
                    }
                }
            })
            .expect("spawn config-watcher thread");

        info!("config watcher started on '{}'", config_dir.display());
        Ok(handle)
    }

    // -- Key-path access ----------------------------------------------------

    /// Set a nested config value using dot-separated path notation.
    ///
    /// Supported paths:
    /// - `"general.data_dir"`
    /// - `"general.auto_save"`           (expects `"true"` / `"false"`)
    /// - `"general.auto_save_interval_secs"`
    /// - `"audio.default_volume"`        (float string, e.g. `"0.75"`)
    /// - `"audio.output_device"`         (`"null"` to clear, or a string)
    /// - `"ui.theme"`
    /// - `"ui.font_size"`
    /// - `"keybinds.<action>"`           (set/update a keybinding)
    ///
    /// Returns `Ok(())` on success or a [`ConfigError`] describing why the
    /// operation failed.
    pub fn set(&mut self, key_path: &str, value: &str) -> ConfigResult<()> {
        let segments: Vec<&str> = key_path.split('.').collect();

        match segments.as_slice() {
            ["general", "data_dir"] => {
                self.general.data_dir = PathBuf::from(value);
            }
            ["general", "auto_save"] => {
                self.general.auto_save = value
                    .parse::<bool>()
                    .map_err(|_| ConfigError::InvalidType {
                        key: key_path.into(),
                        expected: "bool (true/false)".into(),
                    })?;
            }
            ["general", "auto_save_interval_secs"] => {
                self.general.auto_save_interval_secs = value
                    .parse::<u64>()
                    .map_err(|_| ConfigError::InvalidType {
                        key: key_path.into(),
                        expected: "u64".into(),
                    })?;
            }
            ["audio", "default_volume"] => {
                let vol: f32 = value.parse::<f32>().map_err(|_| ConfigError::InvalidType {
                    key: key_path.into(),
                    expected: "f32 (e.g. 0.8)".into(),
                })?;
                if !(0.0..=1.0).contains(&vol) {
                    return Err(ConfigError::InvalidType {
                        key: key_path.into(),
                        expected: "f32 in range [0.0, 1.0]".into(),
                    });
                }
                self.audio.default_volume = vol;
            }
            ["audio", "output_device"] => {
                self.audio.output_device = if value.eq_ignore_ascii_case("null") || value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            ["ui", "theme"] => {
                self.ui.theme = value.to_string();
            }
            ["ui", "font_size"] => {
                self.ui.font_size = value
                    .parse::<u16>()
                    .map_err(|_| ConfigError::InvalidType {
                        key: key_path.into(),
                        expected: "u16".into(),
                    })?;
            }
            ["keybinds", action] => {
                self.keybinds
                    .insert(action.to_string(), value.to_string());
            }
            _ => {
                return Err(ConfigError::UnknownKey(key_path.into()));
            }
        }

        debug!("set config '{}' = '{}'", key_path, value);
        Ok(())
    }

    /// Read a config value by dot-separated path, returned as a string.
    ///
    /// Returns `None` if the key path is valid but the value is an optional
    /// that is currently unset (e.g. `"audio.output_device"` when `None`).
    /// Returns `None` for unknown paths as well (callers that need to
    /// distinguish should use direct field access or the typed accessors).
    pub fn get(&self, key_path: &str) -> Option<String> {
        let segments: Vec<&str> = key_path.split('.').collect();

        match segments.as_slice() {
            ["general", "data_dir"] => {
                Some(self.general.data_dir.to_string_lossy().into_owned())
            }
            ["general", "auto_save"] => Some(self.general.auto_save.to_string()),
            ["general", "auto_save_interval_secs"] => {
                Some(self.general.auto_save_interval_secs.to_string())
            }
            ["audio", "default_volume"] => Some(self.audio.default_volume.to_string()),
            ["audio", "output_device"] => self.audio.output_device.clone(),
            ["ui", "theme"] => Some(self.ui.theme.clone()),
            ["ui", "font_size"] => Some(self.ui.font_size.to_string()),
            ["keybinds", action] => self.keybinds.get(*action).cloned(),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Config watcher handle – dropping it stops the watcher
// ---------------------------------------------------------------------------

/// Opaque handle returned by [`Config::watch`].
///
/// The background watcher thread runs until this handle is dropped.
/// Dropping it closes the notify channel which causes the watcher thread to
/// exit cleanly.
pub struct ConfigWatcherHandle {
    _watcher: Option<notify::RecommendedWatcher>,
}
