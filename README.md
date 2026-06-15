# 🎵 open_music

**Music library manager — play, organise, extend.**

`open_music` is a terminal-based music library manager written in Rust. It features **real audio playback** via rodio, a **plugin architecture** for extensibility, **TOML-based configuration** with hot-reload, and a dual **REPL + CLI** interface.

```
╔══════════════════════════════════════════════════════╗
║                 open_music — 音樂庫管理員             ║
╚══════════════════════════════════════════════════════╝
🎵 播放控制:  play, pause, resume, next, prev, stop
🔊 音量/跳轉:  volume, seek, repeat, shuffle
📋 佇列管理:   queue add, queue show, queue clear
📚 音樂庫:     search, import, export, edit, stats
📝 播放清單:   list, show, add-to-playlist
⚙️  設定:      setconfig, getconfig, save, load
```

---

## ✨ Features

| Feature | Description |
|---------|-------------|
| **🎵 Real audio playback** | rodio + cpal — play MP3, FLAC, WAV, Ogg, AAC, M4A |
| **🔌 Plugin architecture** | `Plugin` trait + `PluginRegistry` — extend with custom commands |
| **⚙️ Config-driven** | TOML config at `~/.config/open_music/config.toml` with hot-reload |
| **📦 Library persistence** | JSON-based storage at `~/.local/share/open_music/library.json` |
| **🔄 Dual interface** | REPL (interactive) + CLI one-shot mode (`open_music play "song"`) |
| **🧵 Thread-safe** | All state in `Arc<RwLock<>>` — shared `AppContext` |
| **🔊 Volume control** | Set volume via CLI, persisted in config |
| **🔁 Repeat & Shuffle** | Repeat off/one/all + toggle shuffle |
| **📋 Queue management** | Add, show, clear, auto-next through queue |
| **📝 Playlists** | Create, show, add songs to playlists |
| **🔍 Search** | By title, hashtag, or creditor name |
| **📥 Import** | Batch-import audio files from a directory (auto-extracts duration via symphonia) |
| **📤 Export** | Export full library as JSON |

---

## 🚀 Quick Start

### Prerequisites

- Rust 2024 edition (1.85+)
- Audio output device (ALSA, PulseAudio, or PipeWire on Linux)

### Build & Run

```bash
git clone https://github.com/ccfish948/open_music.git
cd open_music
cargo run
```

### REPL Mode

```bash
cargo run
```

```
🎵 open_music v0.1.0 — 音樂庫管理員
🔌 外掛架構 · 設定驅動 · 音訊輸出 (rodio)
輸入 help 查看指令，exit 離開

🎵 > help
🎵 > play ~/Music/song.mp3
🎵 > stats
🎵 > queue show
```

### CLI One-Shot Mode

```bash
# Play a song
cargo run -- play "song name"

# Import a directory
cargo run -- import ~/Music/my-library

# Show stats
cargo run -- stats

# Search
cargo run -- search "jazz"
```

---

## 📖 Commands

### Playback

| Command | Alias | Description |
|---------|-------|-------------|
| `play <name>` | `p` | Play a song by name (fuzzy match) |
| `play -a` | `p -a` | Play all songs in library |
| `pause` | | Pause playback |
| `resume` | `unpause` | Resume playback |
| `next` | `n` | Skip to next track |
| `prev` | `previous`, `b` | Go back to previous track |
| `stop` | `s` | Stop playback |
| `volume <0-100>` | `vol` | Set volume percentage |
| `seek <secs>` | `goto` | Seek to position in seconds |
| `repeat <off\|one\|all>` | `loop` | Set repeat mode |
| `shuffle` | `random` | Toggle shuffle mode |

### Queue

| Command | Description |
|---------|-------------|
| `queue show` | Show the current play queue |
| `queue add <name>` | Add a song to the queue |
| `queue add -a` | Add all songs to the queue |
| `queue clear` | Clear the queue |

### Library

| Command | Description |
|---------|-------------|
| `search <query>` | Search songs by title, hashtag, or creditor |
| `import <dir>` | Import audio files from a directory (mp3/flac/wav/ogg/m4a/aac) |
| `export [path]` | Export library as JSON |
| `remove <title>` | Remove a song from the library |
| `edit <title> <field> <value>` | Edit song metadata (title, description, hashtags, credits, album) |
| `lyrics <title>` | Show lyrics for a song |
| `lyrics <title> <text>` | Set lyrics for a song |
| `stats` | Show library statistics |
| `history` | Show play history |

### Playlists

| Command | Description |
|---------|-------------|
| `list` | List all playlists |
| `show <name>` | Show playlist contents |
| `add-to-playlist <song> <playlist>` | Add a song to a playlist |

### Config & System

| Command | Description |
|---------|-------------|
| `setconfig <key> <value>` | Set a config value (e.g. `audio.default_volume 0.5`) |
| `getconfig [key]` | Get a config value (or all if no key) |
| `save` | Save library to disk |
| `load` | Load library from disk |
| `help` | Show help |
| `exit` | Quit |

---

## 🏗️ Architecture

```
src/
├── main.rs              # CLI (clap) + REPL loop + config hot-reload
├── context.rs           # AppContext — unified state (all Arc<RwLock<>>)
├── config.rs            # TOML config: load/save/hot-reload/dot-path access
├── command.rs           # Command enum + parser + help text
├── command/
│   └── executor.rs      # All command implementations (~590 lines)
├── library.rs           # Song, Playlist, Album, Creditor, Library data model
├── library/
│   └── persistence.rs   # JSON persistence for library
├── player.rs            # AudioBackend (rodio) + Player state machine
├── plugin.rs            # Plugin trait + PluginRegistry (extension point)
└── ui.rs                # TUI stub (reserved for future ratatui interface)
```

### Thread Safety

Every shared state object is wrapped in `Arc<RwLock<>>`:

```rust
pub struct AppContext {
    pub config:  Arc<RwLock<Config>>,
    pub library: Arc<RwLock<Library>>,
    pub player:  Arc<RwLock<Player>>,
    pub plugins: Arc<RwLock<PluginRegistry>>,
    pub store:   LibraryStore,
}
```

### Plugin System

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn on_init(&self, ctx: &AppContext) -> PluginResult<()> { Ok(()) }
    fn commands(&self) -> Vec<PluginCommand> { Vec::new() }
    fn config_defaults(&self) -> HashMap<String, toml::Value> { HashMap::new() }
}
```

Plugins can register custom commands, inject config defaults, and hook into the app lifecycle.

---

## ⚙️ Configuration

**Path:** `~/.config/open_music/config.toml`

```toml
[general]
data_dir = "/home/user/.local/share/open_music"
auto_save = true
auto_save_interval_secs = 300

[audio]
default_volume = 0.8
output_device = "default"

[ui]
theme = "default"
font_size = 14
```

Config changes are **hot-reloaded** automatically when the file is modified.

---

## 📦 Companion: media-puller

Download music from YouTube, Bilibili, Douyin and 1000+ platforms:

```bash
pip install yt-dlp
git clone https://github.com/ccfish948/media-puller.git
cd media-puller
python3 -m media_puller --open-music https://youtu.be/xxxx
```

Then import into open_music:

```
🎵 > import ~/Music/media-puller
```

---

## 🛠️ Development

```bash
# Build
cargo build

# Run with verbose logging
RUST_LOG=debug cargo run

# Run tests
cargo test

# Check for unused dependencies
cargo +nightly udeps
```

### Dependencies

- **symphonia** — audio decoding (MP3, FLAC, AAC, Vorbis, WAV, M4A)
- **rodio** — audio output via cpal
- **serde + serde_json** — serialization
- **clap** — CLI argument parsing
- **toml** — config file format
- **notify** — file watching for config hot-reload
- **crossterm + ratatui** — TUI (stub, future use)
- **base64 + image** — cover art handling
- **dirs** — platform config/data directories

---

## 📄 License

MIT
