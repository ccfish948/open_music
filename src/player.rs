//! Player engine — queue management, playback state, and rodio audio output.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use log::{info, warn};

// ── Enums ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    One,
    All,
}

// ── QueueEntry ──

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub song_id: usize,
    pub song_title: String,
    pub source_path: PathBuf,
}

// ── Rodio audio backend ──

/// Wraps rodio output stream and sink for audio playback.
/// Created once in `main.rs` and shared with the Player via the `SharedAudio` type.
pub struct AudioBackend {
    /// Must be kept alive — dropping it stops all audio.
    #[allow(dead_code)]
    stream: rodio::OutputStream,
    /// Handle used to create sinks.
    handle: rodio::OutputStreamHandle,
    /// Current playback controller.
    sink: Option<rodio::Sink>,
}

impl AudioBackend {
    /// Initialise the default audio output device.
    pub fn try_new() -> Option<Self> {
        match rodio::OutputStream::try_default() {
            Ok((stream, handle)) => {
                info!("audio backend initialised");
                Some(Self { stream, handle, sink: None })
            }
            Err(e) => {
                warn!("no audio output device available: {e}");
                None
            }
        }
    }

    /// Start playing a file through the sink.
    pub fn play_file(&mut self, path: &PathBuf) {
        // Stop any current playback
        if let Some(ref sink) = self.sink {
            sink.stop();
        }

        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) => {
                warn!("cannot open audio file {}: {e}", path.display());
                return;
            }
        };

        match rodio::Decoder::new(file) {
            Ok(source) => {
                match rodio::Sink::try_new(&self.handle) {
                    Ok(sink) => {
                        sink.append(source);
                        self.sink = Some(sink);
                        info!("audio playback started: {}", path.display());
                    }
                    Err(e) => warn!("cannot create audio sink: {e}"),
                }
            }
            Err(e) => warn!("cannot decode audio file {}: {e}", path.display()),
        }
    }

    /// Pause the current sink.
    pub fn pause(&self) {
        if let Some(ref sink) = self.sink {
            sink.pause();
        }
    }

    /// Resume the current sink.
    pub fn resume(&self) {
        if let Some(ref sink) = self.sink {
            sink.play();
        }
    }

    /// Stop the current sink.
    pub fn stop(&mut self) {
        if let Some(ref sink) = self.sink {
            sink.stop();
        }
        self.sink = None;
    }

    /// Update volume on the current sink (0.0 – 1.0).
    pub fn set_volume(&self, vol: f32) {
        if let Some(ref sink) = self.sink {
            sink.set_volume(vol);
        }
    }

    /// Returns `true` if the sink has finished playing all its queued data.
    pub fn is_empty(&self) -> bool {
        self.sink.as_ref().map_or(true, |s| s.empty())
    }
}

/// Thread-safe wrapper around `Option<AudioBackend>`.
pub type SharedAudio = Arc<RwLock<Option<AudioBackend>>>;

// ── Player ──

pub struct Player {
    pub state: PlayerState,
    pub repeat: RepeatMode,
    pub shuffle: bool,
    pub volume: f32,
    pub queue: VecDeque<QueueEntry>,
    pub current_index: Option<usize>,
    pub history: Vec<QueueEntry>,
    pub elapsed: Duration,
    /// Shared reference to the audio backend.
    pub audio: SharedAudio,
}

impl Player {
    pub fn new(audio: SharedAudio) -> Self {
        Self {
            state: PlayerState::Stopped,
            repeat: RepeatMode::Off,
            shuffle: false,
            volume: 0.8,
            queue: VecDeque::new(),
            current_index: None,
            history: Vec::new(),
            elapsed: Duration::ZERO,
            audio,
        }
    }

    /// Add a song to the queue and start playback if stopped.
    pub fn play(&mut self, song_id: usize, title: &str, path: PathBuf) {
        let entry = QueueEntry {
            song_id,
            song_title: title.to_string(),
            source_path: path,
        };

        self.queue.push_back(entry);

        if self.state == PlayerState::Stopped {
            self.current_index = Some(self.queue.len() - 1);
            self.state = PlayerState::Playing;
            self.elapsed = Duration::ZERO;
            self.start_playback();
        }
    }

    /// Advance to the next song, respecting repeat and shuffle.
    pub fn play_next(&mut self) {
        if self.queue.is_empty() {
            self.state = PlayerState::Stopped;
            self.current_index = None;
            return;
        }

        // Save current song to history
        if let Some(idx) = self.current_index {
            if idx < self.queue.len() {
                self.history.push(self.queue[idx].clone());
            }
        }

        match self.repeat {
            RepeatMode::One => {
                self.elapsed = Duration::ZERO;
                self.restart_file();
                return;
            }
            RepeatMode::All => {
                self.current_index = Some(
                    self.current_index.map_or(0, |i| (i + 1) % self.queue.len()),
                );
            }
            RepeatMode::Off => {
                if let Some(idx) = self.current_index {
                    if idx + 1 < self.queue.len() {
                        self.current_index = Some(idx + 1);
                    } else {
                        self.current_index = None;
                        self.state = PlayerState::Stopped;
                        self.elapsed = Duration::ZERO;
                        self.stop_audio();
                        return;
                    }
                } else {
                    self.current_index = Some(0);
                }
            }
        }

        self.elapsed = Duration::ZERO;
        if self.state != PlayerState::Playing {
            self.state = PlayerState::Playing;
        }
        self.restart_file();
    }

    /// Go back to the previous song from history.
    pub fn play_prev(&mut self) {
        if let Some(prev) = self.history.pop() {
            if let Some(pos) = self.queue.iter().position(|e| e.song_id == prev.song_id) {
                self.current_index = Some(pos);
            } else {
                self.queue.push_front(prev);
                self.current_index = Some(0);
            }
            self.elapsed = Duration::ZERO;
            if self.state != PlayerState::Playing {
                self.state = PlayerState::Playing;
            }
            self.restart_file();
        }
    }

    pub fn pause(&mut self) {
        if self.state == PlayerState::Playing {
            self.state = PlayerState::Paused;
            if let Ok(audio) = self.audio.read() {
                if let Some(ref backend) = *audio {
                    backend.pause();
                }
            }
        }
    }

    pub fn resume(&mut self) {
        if self.state == PlayerState::Paused {
            self.state = PlayerState::Playing;
            if let Ok(audio) = self.audio.read() {
                if let Some(ref backend) = *audio {
                    backend.resume();
                }
            }
        }
    }

    pub fn stop(&mut self) {
        self.state = PlayerState::Stopped;
        self.current_index = None;
        self.elapsed = Duration::ZERO;
        self.stop_audio();
    }

    pub fn seek(&mut self, seconds: u64) {
        self.elapsed = Duration::from_secs(seconds);
        // Rodio does not support seeking — log it.
        info!("seek to {seconds}s requested (not supported by rodio)");
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        if let Ok(audio) = self.audio.read() {
            if let Some(ref backend) = *audio {
                backend.set_volume(self.volume);
            }
        }
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    pub fn queue_add(&mut self, entries: Vec<QueueEntry>) {
        self.queue.extend(entries);
    }

    pub fn queue_clear(&mut self) {
        self.queue.clear();
        self.current_index = None;
        self.state = PlayerState::Stopped;
        self.elapsed = Duration::ZERO;
        self.stop_audio();
    }

    pub fn queue_list(&self) -> &VecDeque<QueueEntry> {
        &self.queue
    }

    pub fn current_song(&self) -> Option<&QueueEntry> {
        self.current_index.and_then(|idx| self.queue.get(idx))
    }

    // ── Audio helpers ──

    fn start_playback(&self) {
        if let Some(song) = self.current_song() {
            if let Ok(mut audio) = self.audio.write() {
                if let Some(ref mut backend) = *audio {
                    backend.play_file(&song.source_path);
                    backend.set_volume(self.volume);
                    return;
                }
            }
            // No audio backend — just log
            info!("playback would start: {}", song.source_path.display());
        }
    }

    fn restart_file(&self) {
        if let Some(song) = self.current_song() {
            if let Ok(mut audio) = self.audio.write() {
                if let Some(ref mut backend) = *audio {
                    backend.play_file(&song.source_path);
                    backend.set_volume(self.volume);
                    return;
                }
            }
            info!("playback would restart: {}", song.source_path.display());
        }
    }

    fn stop_audio(&self) {
        if let Ok(mut audio) = self.audio.write() {
            if let Some(ref mut backend) = *audio {
                backend.stop();
            }
        }
    }
}

// ── Convenience type alias ──

pub type SharedPlayer = Arc<RwLock<Player>>;
