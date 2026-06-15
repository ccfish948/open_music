//! Configuration system for open_music.
//!
//! Stored as TOML at `$CONFIG_DIR/open_music/config.toml`.
//! Every aspect of the app is controllable through this file.
//! Hot-reload watches for changes and notifies subscribers in real time.

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
// Config change notification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ConfigEvent {
    KeyChanged { key: String, old_value: Option<String>, new_value: String },
    Reloaded,
}

pub trait ConfigSubscriber: Send + 'static {
    fn on_config_event(&mut self, event: &ConfigEvent);
}

// ---------------------------------------------------------------------------
// Config sub-structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default = "default_true")]
    pub auto_save: bool,
    #[serde(default = "default_auto_save_interval")]
    pub auto_save_interval_secs: u64,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioConfig {
    #[serde(default = "default_volume")]
    pub default_volume: f32,
    #[serde(default)]
    pub output_device: Option<String>,
    #[serde(default)]
    pub crossfade_secs: u32,
    #[serde(default)]
    pub gapless: bool,
    #[serde(default)]
    pub normalize_audio: bool,
    #[serde(default = "default_replaygain")]
    pub replaygain_mode: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaybackConfig {
    #[serde(default = "default_repeat_mode")]
    pub repeat_mode: String,
    #[serde(default)]
    pub shuffle_on_start: bool,
    #[serde(default = "default_true")]
    pub auto_play_next: bool,
    #[serde(default = "default_true")]
    pub stop_at_queue_end: bool,
    #[serde(default)]
    pub resume_on_startup: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueConfig {
    #[serde(default)]
    pub max_size: usize,
    #[serde(default)]
    pub clear_on_play_all: bool,
    #[serde(default)]
    pub append_to_front: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryConfig {
    #[serde(default)]
    pub scan_on_startup: bool,
    #[serde(default)]
    pub watch_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default = "default_cover_size")]
    pub cover_size_limit_mb: u32,
    #[serde(default = "default_true")]
    pub prefer_embedded_cover: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_true")]
    pub fuzzy_match: bool,
    #[serde(default = "default_true")]
    pub match_credits: bool,
    #[serde(default = "default_true")]
    pub match_hashtags: bool,
    #[serde(default)]
    pub match_lyrics: bool,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_time_format")]
    pub time_format: String,
    #[serde(default = "default_progress_style")]
    pub progress_bar_style: String,
    #[serde(default = "default_true")]
    pub show_duration: bool,
    #[serde(default = "default_true")]
    pub show_playlist_count: bool,
    #[serde(default = "default_now_playing")]
    pub now_playing_format: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoryConfig {
    #[serde(default = "default_history_max")]
    pub max_entries: usize,
    #[serde(default = "default_true")]
    pub save_on_exit: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BehaviorConfig {
    #[serde(default = "default_on_startup")]
    pub on_startup: String,
    #[serde(default = "default_on_end")]
    pub on_playlist_end: String,
    #[serde(default = "default_on_error")]
    pub on_error: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_font_size")]
    pub font_size: u16,
    #[serde(default = "default_true")]
    pub show_cover_art: bool,
    #[serde(default = "default_true")]
    pub show_lyrics: bool,
    #[serde(default = "default_true")]
    pub show_queue: bool,
}

// ---------------------------------------------------------------------------
// Top-level Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub playback: PlaybackConfig,
    #[serde(default)]
    pub queue: QueueConfig,
    #[serde(default)]
    pub library: LibraryConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub history: HistoryConfig,
    #[serde(default)]
    pub behavior: BehaviorConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub plugins: HashMap<String, toml::Value>,
    #[serde(default)]
    pub keybinds: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Default helpers
// ---------------------------------------------------------------------------

fn default_data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("open_music")
}
fn default_true() -> bool { true }
fn default_auto_save_interval() -> u64 { 300 }
fn default_language() -> String { "zh-TW".into() }
fn default_log_level() -> String { "info".into() }
fn default_volume() -> f32 { 0.8 }
fn default_replaygain() -> String { "none".into() }
fn default_repeat_mode() -> String { "off".into() }
fn default_queue_max() -> usize { 0 }
fn default_cover_size() -> u32 { 5 }
fn default_max_results() -> usize { 50 }
fn default_time_format() -> String { "mm:ss".into() }
fn default_progress_style() -> String { "bar".into() }
fn default_now_playing() -> String { "{title} — {artist} [{duration}]".into() }
fn default_history_max() -> usize { 100 }
fn default_on_startup() -> String { "nothing".into() }
fn default_on_end() -> String { "stop".into() }
fn default_on_error() -> String { "skip".into() }
fn default_theme() -> String { "default".into() }
fn default_font_size() -> u16 { 14 }

// ---------------------------------------------------------------------------
// Config impl
// ---------------------------------------------------------------------------

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                data_dir: default_data_dir(),
                auto_save: true,
                auto_save_interval_secs: 300,
                language: default_language(),
                log_level: default_log_level(),
            },
            audio: AudioConfig {
                default_volume: 0.8,
                output_device: None,
                crossfade_secs: 0,
                gapless: false,
                normalize_audio: false,
                replaygain_mode: default_replaygain(),
            },
            playback: PlaybackConfig {
                repeat_mode: default_repeat_mode(),
                shuffle_on_start: false, auto_play_next: true,
                stop_at_queue_end: true, resume_on_startup: false,
            },
            queue: QueueConfig { max_size: 0, clear_on_play_all: false, append_to_front: false },
            library: LibraryConfig {
                scan_on_startup: false, watch_dirs: vec![],
                exclude_patterns: vec![], cover_size_limit_mb: 5,
                prefer_embedded_cover: true,
            },
            search: SearchConfig {
                fuzzy_match: true, match_credits: true, match_hashtags: true,
                match_lyrics: false, max_results: 50,
            },
            display: DisplayConfig {
                time_format: default_time_format(),
                progress_bar_style: default_progress_style(),
                show_duration: true, show_playlist_count: true,
                now_playing_format: default_now_playing(),
            },
            history: HistoryConfig { max_entries: 100, save_on_exit: true },
            behavior: BehaviorConfig {
                on_startup: default_on_startup(),
                on_playlist_end: default_on_end(),
                on_error: default_on_error(),
            },
            ui: UiConfig {
                theme: default_theme(), font_size: 14,
                show_cover_art: true, show_lyrics: true, show_queue: true,
            },
            plugins: HashMap::new(),
            keybinds: HashMap::new(),
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("open_music")
    }
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }
    pub fn load() -> ConfigResult<Self> {
        let path = Self::config_path();
        if !path.exists() {
            info!("no config at '{}' — creating default", path.display());
            let cfg = Self::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let raw = fs::read_to_string(&path).map_err(ConfigError::Read)?;
        let cfg: Config = toml::from_str(&raw)?;
        debug!("loaded config from '{}'", path.display());
        Ok(cfg)
    }
    pub fn save(&self) -> ConfigResult<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ConfigError::Dirs)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        fs::write(&path, toml_str).map_err(ConfigError::Write)?;
        debug!("saved config to '{}'", path.display());
        Ok(())
    }
    pub fn watch<F>(callback: F) -> ConfigResult<ConfigWatcherHandle>
    where F: Fn() + Send + 'static {
        use notify::event::EventKind;
        use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
        let config_path = Self::config_path();
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = RecommendedWatcher::new(move |res| { let _ = tx.send(res); }, NotifyConfig::default())
            .map_err(ConfigError::Watch)?;
        watcher.watch(&config_dir, RecursiveMode::NonRecursive).map_err(ConfigError::Watch)?;
        let handle = ConfigWatcherHandle { _watcher: Some(watcher) };
        thread::Builder::new().name("config-watcher".into()).spawn(move || {
            let file_name = config_path.file_name().map(|f| f.to_os_string());
            loop {
                match rx.recv() {
                    Ok(Ok(event)) => {
                        let relevant = event.paths.iter().any(|p|
                            p.file_name().map(|n| Some(n) == file_name.as_deref()).unwrap_or(false));
                        if !relevant { continue; }
                        if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
                            debug!("config.toml changed – triggering callback");
                            callback();
                        }
                    }
                    Ok(Err(e)) => warn!("config watcher error: {e}"),
                    Err(mpsc::RecvError) => break,
                }
            }
        }).expect("spawn config-watcher thread");
        info!("config watcher started on '{}'", config_dir.display());
        Ok(handle)
    }

    // ── Dot-path setter ──

    pub fn set(&mut self, key_path: &str, value: &str) -> ConfigResult<()> {
        let segs: Vec<&str> = key_path.split('.').collect();
        match segs.as_slice() {
            ["general", "data_dir"] => self.general.data_dir = PathBuf::from(value),
            ["general", "auto_save"] => self.general.auto_save = parse_bool(key_path, value)?,
            ["general", "auto_save_interval_secs"] => self.general.auto_save_interval_secs = parse_u64(key_path, value)?,
            ["general", "language"] => self.general.language = value.into(),
            ["general", "log_level"] => self.general.log_level = value.into(),
            ["audio", "default_volume"] => self.audio.default_volume = parse_f32_clamped(key_path, value, 0.0, 1.0)?,
            ["audio", "output_device"] => self.audio.output_device = parse_optional(value),
            ["audio", "crossfade_secs"] => self.audio.crossfade_secs = parse_u64(key_path, value)? as u32,
            ["audio", "gapless"] => self.audio.gapless = parse_bool(key_path, value)?,
            ["audio", "normalize_audio"] => self.audio.normalize_audio = parse_bool(key_path, value)?,
            ["audio", "replaygain_mode"] => self.audio.replaygain_mode = value.into(),
            ["playback", "repeat_mode"] => self.playback.repeat_mode = value.into(),
            ["playback", "shuffle_on_start"] => self.playback.shuffle_on_start = parse_bool(key_path, value)?,
            ["playback", "auto_play_next"] => self.playback.auto_play_next = parse_bool(key_path, value)?,
            ["playback", "stop_at_queue_end"] => self.playback.stop_at_queue_end = parse_bool(key_path, value)?,
            ["playback", "resume_on_startup"] => self.playback.resume_on_startup = parse_bool(key_path, value)?,
            ["queue", "max_size"] => self.queue.max_size = parse_u64(key_path, value)? as usize,
            ["queue", "clear_on_play_all"] => self.queue.clear_on_play_all = parse_bool(key_path, value)?,
            ["queue", "append_to_front"] => self.queue.append_to_front = parse_bool(key_path, value)?,
            ["library", "scan_on_startup"] => self.library.scan_on_startup = parse_bool(key_path, value)?,
            ["library", "watch_dirs"] => { self.library.watch_dirs = value.split(',').map(|s| PathBuf::from(s.trim())).collect(); }
            ["library", "exclude_patterns"] => { self.library.exclude_patterns = value.split(',').map(|s| s.trim().to_string()).collect(); }
            ["library", "cover_size_limit_mb"] => self.library.cover_size_limit_mb = parse_u64(key_path, value)? as u32,
            ["library", "prefer_embedded_cover"] => self.library.prefer_embedded_cover = parse_bool(key_path, value)?,
            ["search", "fuzzy_match"] => self.search.fuzzy_match = parse_bool(key_path, value)?,
            ["search", "match_credits"] => self.search.match_credits = parse_bool(key_path, value)?,
            ["search", "match_hashtags"] => self.search.match_hashtags = parse_bool(key_path, value)?,
            ["search", "match_lyrics"] => self.search.match_lyrics = parse_bool(key_path, value)?,
            ["search", "max_results"] => self.search.max_results = parse_u64(key_path, value)? as usize,
            ["display", "time_format"] => self.display.time_format = value.into(),
            ["display", "progress_bar_style"] => self.display.progress_bar_style = value.into(),
            ["display", "show_duration"] => self.display.show_duration = parse_bool(key_path, value)?,
            ["display", "show_playlist_count"] => self.display.show_playlist_count = parse_bool(key_path, value)?,
            ["display", "now_playing_format"] => self.display.now_playing_format = value.into(),
            ["history", "max_entries"] => self.history.max_entries = parse_u64(key_path, value)? as usize,
            ["history", "save_on_exit"] => self.history.save_on_exit = parse_bool(key_path, value)?,
            ["behavior", "on_startup"] => self.behavior.on_startup = value.into(),
            ["behavior", "on_playlist_end"] => self.behavior.on_playlist_end = value.into(),
            ["behavior", "on_error"] => self.behavior.on_error = value.into(),
            ["ui", "theme"] => self.ui.theme = value.into(),
            ["ui", "font_size"] => self.ui.font_size = parse_u64(key_path, value)? as u16,
            ["ui", "show_cover_art"] => self.ui.show_cover_art = parse_bool(key_path, value)?,
            ["ui", "show_lyrics"] => self.ui.show_lyrics = parse_bool(key_path, value)?,
            ["ui", "show_queue"] => self.ui.show_queue = parse_bool(key_path, value)?,
            ["keybinds", action] => { self.keybinds.insert(action.to_string(), value.to_string()); }
            _ => return Err(ConfigError::UnknownKey(key_path.into())),
        }
        debug!("set config '{}' = '{}'", key_path, value);
        Ok(())
    }

    // ── Dot-path getter ──

    pub fn get(&self, key_path: &str) -> Option<String> {
        let segs: Vec<&str> = key_path.split('.').collect();
        match segs.as_slice() {
            ["general", "data_dir"] => Some(self.general.data_dir.to_string_lossy().into()),
            ["general", "auto_save"] => Some(self.general.auto_save.to_string()),
            ["general", "auto_save_interval_secs"] => Some(self.general.auto_save_interval_secs.to_string()),
            ["general", "language"] => Some(self.general.language.clone()),
            ["general", "log_level"] => Some(self.general.log_level.clone()),
            ["audio", "default_volume"] => Some(self.audio.default_volume.to_string()),
            ["audio", "output_device"] => self.audio.output_device.clone(),
            ["audio", "crossfade_secs"] => Some(self.audio.crossfade_secs.to_string()),
            ["audio", "gapless"] => Some(self.audio.gapless.to_string()),
            ["audio", "normalize_audio"] => Some(self.audio.normalize_audio.to_string()),
            ["audio", "replaygain_mode"] => Some(self.audio.replaygain_mode.clone()),
            ["playback", "repeat_mode"] => Some(self.playback.repeat_mode.clone()),
            ["playback", "shuffle_on_start"] => Some(self.playback.shuffle_on_start.to_string()),
            ["playback", "auto_play_next"] => Some(self.playback.auto_play_next.to_string()),
            ["playback", "stop_at_queue_end"] => Some(self.playback.stop_at_queue_end.to_string()),
            ["playback", "resume_on_startup"] => Some(self.playback.resume_on_startup.to_string()),
            ["queue", "max_size"] => Some(self.queue.max_size.to_string()),
            ["queue", "clear_on_play_all"] => Some(self.queue.clear_on_play_all.to_string()),
            ["queue", "append_to_front"] => Some(self.queue.append_to_front.to_string()),
            ["library", "scan_on_startup"] => Some(self.library.scan_on_startup.to_string()),
            ["library", "watch_dirs"] => Some(self.library.watch_dirs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")),
            ["library", "exclude_patterns"] => Some(self.library.exclude_patterns.join(", ")),
            ["library", "cover_size_limit_mb"] => Some(self.library.cover_size_limit_mb.to_string()),
            ["library", "prefer_embedded_cover"] => Some(self.library.prefer_embedded_cover.to_string()),
            ["search", "fuzzy_match"] => Some(self.search.fuzzy_match.to_string()),
            ["search", "match_credits"] => Some(self.search.match_credits.to_string()),
            ["search", "match_hashtags"] => Some(self.search.match_hashtags.to_string()),
            ["search", "match_lyrics"] => Some(self.search.match_lyrics.to_string()),
            ["search", "max_results"] => Some(self.search.max_results.to_string()),
            ["display", "time_format"] => Some(self.display.time_format.clone()),
            ["display", "progress_bar_style"] => Some(self.display.progress_bar_style.clone()),
            ["display", "show_duration"] => Some(self.display.show_duration.to_string()),
            ["display", "show_playlist_count"] => Some(self.display.show_playlist_count.to_string()),
            ["display", "now_playing_format"] => Some(self.display.now_playing_format.clone()),
            ["history", "max_entries"] => Some(self.history.max_entries.to_string()),
            ["history", "save_on_exit"] => Some(self.history.save_on_exit.to_string()),
            ["behavior", "on_startup"] => Some(self.behavior.on_startup.clone()),
            ["behavior", "on_playlist_end"] => Some(self.behavior.on_playlist_end.clone()),
            ["behavior", "on_error"] => Some(self.behavior.on_error.clone()),
            ["ui", "theme"] => Some(self.ui.theme.clone()),
            ["ui", "font_size"] => Some(self.ui.font_size.to_string()),
            ["ui", "show_cover_art"] => Some(self.ui.show_cover_art.to_string()),
            ["ui", "show_lyrics"] => Some(self.ui.show_lyrics.to_string()),
            ["ui", "show_queue"] => Some(self.ui.show_queue.to_string()),
            ["keybinds", action] => self.keybinds.get(*action).cloned(),
            _ => None,
        }
    }
}

// ── Parse helpers ──

fn parse_bool(key: &str, v: &str) -> ConfigResult<bool> {
    v.parse::<bool>().map_err(|_| ConfigError::InvalidType { key: key.into(), expected: "bool".into() })
}
fn parse_u64(key: &str, v: &str) -> ConfigResult<u64> {
    v.parse::<u64>().map_err(|_| ConfigError::InvalidType { key: key.into(), expected: "u64".into() })
}
fn parse_f32_clamped(key: &str, v: &str, min: f32, max: f32) -> ConfigResult<f32> {
    let val: f32 = v.parse().map_err(|_| ConfigError::InvalidType { key: key.into(), expected: format!("f32[{min},{max}]") })?;
    if !(min..=max).contains(&val) {
        return Err(ConfigError::InvalidType { key: key.into(), expected: format!("f32[{min},{max}]") });
    }
    Ok(val)
}
fn parse_optional(v: &str) -> Option<String> {
    if v.eq_ignore_ascii_case("null") || v.is_empty() { None } else { Some(v.to_string()) }
}

// ── Watcher handle ──

pub struct ConfigWatcherHandle {
    _watcher: Option<notify::RecommendedWatcher>,
}
