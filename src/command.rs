pub mod executor;

use std::path::PathBuf;

/// All supported commands for open_music.
#[derive(Debug, Clone)]
pub enum Command {
    // ── Playback ──
    Play { name: Option<String>, all: bool },
    Pause,
    Resume,
    Next,
    Prev,
    Stop,
    Volume(u8),
    Seek(u64),
    Repeat(String),
    Shuffle,

    // ── Queue ──
    QueueAdd { name: Option<String>, all: bool },
    QueueShow,
    QueueClear,

    // ── Library ──
    Search(String),
    Import { path: PathBuf },
    Export { path: PathBuf },
    RemoveSong(String),
    EditSong { title: String, field: String, value: String },
    Lyrics { title: String, text: Option<String> },
    Stats,
    History,

    // ── Playlist ──
    ListPlaylists,
    ShowPlaylist(String),
    AddToPlaylist { song: String, playlist: String },

    // ── Config ──
    SetConfig { key: String, value: String },
    GetConfig(String),

    // ── Persistence ──
    Save,
    Load,

    // ── System ──
    Plugin(String),
    Help,
    Exit,
}

/// Parse a line of user input into a Command.
pub fn parse_command(line: &str) -> Option<Command> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let parts = shell_words(line);
    if parts.is_empty() {
        return None;
    }

    let cmd = parts[0].to_lowercase();
    let args = &parts[1..];

    match cmd.as_str() {
        // ── Playback ──
        "play" | "p" => {
            let mut name = None;
            let mut all = false;
            let mut i = 0;
            while i < args.len() {
                match &args[i] {
                    a if a == "-n" || a == "--name" => {
                        i += 1;
                        name = args.get(i).cloned();
                    }
                    a if a == "-a" || a == "--all" => all = true,
                    _ => name = Some(args[i].clone()),
                }
                i += 1;
            }
            Some(Command::Play { name, all })
        }
        "pause" => Some(Command::Pause),
        "resume" | "unpause" => Some(Command::Resume),
        "next" | "n" => Some(Command::Next),
        "prev" | "previous" | "b" => Some(Command::Prev),
        "stop" | "s" => Some(Command::Stop),
        "volume" | "vol" => {
            let vol = args.first()?.parse().ok()?;
            Some(Command::Volume(vol))
        }
        "seek" | "goto" => {
            let secs = args.first()?.parse().ok()?;
            Some(Command::Seek(secs))
        }
        "repeat" | "loop" => {
            let mode = args.first()?.to_lowercase();
            Some(Command::Repeat(mode))
        }
        "shuffle" | "random" => Some(Command::Shuffle),

        // ── Queue ──
        "queue" | "q" => {
            let sub = args.first().map(|s| s.as_str()).unwrap_or("");
            match sub {
                "add" | "a" | "append" => {
                    let mut name = None;
                    let mut all = false;
                    let mut i = 1;
                    while i < args.len() {
                        match &args[i] {
                            a if a == "-n" || a == "--name" => {
                                i += 1;
                                name = args.get(i).cloned();
                            }
                            a if a == "-a" || a == "--all" => all = true,
                            _ => name = Some(args[i].clone()),
                        }
                        i += 1;
                    }
                    Some(Command::QueueAdd { name, all })
                }
                "show" | "ls" | "list" => Some(Command::QueueShow),
                "clear" | "c" => Some(Command::QueueClear),
                _ if sub.is_empty() => Some(Command::QueueShow),
                _ => Some(Command::QueueShow),
            }
        }

        // ── Library ──
        "search" | "find" => Some(Command::Search(args.join(" "))),
        "import" => {
            let path = PathBuf::from(args.first()?);
            Some(Command::Import { path })
        }
        "export" => {
            let path = PathBuf::from(args.first().unwrap_or(&String::new()));
            Some(Command::Export { path })
        }
        "remove" | "rm" => Some(Command::RemoveSong(args.join(" "))),
        "edit" => {
            let title = args.first()?.to_string();
            let field = args.get(1).cloned().unwrap_or_default();
            let value = args.get(2..).map(|v| v.join(" ")).unwrap_or_default();
            Some(Command::EditSong { title, field, value })
        }
        "lyrics" => {
            let title = args.first()?.to_string();
            let text = if args.len() > 1 {
                Some(args[1..].join(" "))
            } else {
                None
            };
            Some(Command::Lyrics { title, text })
        }
        "stats" => Some(Command::Stats),
        "history" | "hist" => Some(Command::History),

        // ── Playlist ──
        "list" | "ls" => Some(Command::ListPlaylists),
        "show" | "cat" => {
            let name = args.join(" ");
            Some(Command::ShowPlaylist(name))
        }
        "addtoplaylist" | "add-to-playlist" => {
            let song = args.first()?.to_string();
            let playlist = args.get(1)?.to_string();
            Some(Command::AddToPlaylist { song, playlist })
        }

        // ── Config ──
        "setconfig" | "set" => {
            let key = args.first()?.to_string();
            let value = args.get(1)?.to_string();
            Some(Command::SetConfig { key, value })
        }
        "getconfig" | "get" => {
            let key = args.first().cloned().unwrap_or_default();
            Some(Command::GetConfig(key))
        }

        // ── Persistence ──
        "save" | "write" => Some(Command::Save),
        "load" | "read" => Some(Command::Load),

        // ── System ──
        "plugin" | "plugins" => Some(Command::Plugin(args.join(" "))),
        "help" | "h" | "?" => Some(Command::Help),
        "exit" | "quit" | "q!" => Some(Command::Exit),

        _ => {
            // Try as implicit play command
            Some(Command::Play {
                name: Some(line.to_string()),
                all: false,
            })
        }
    }
}

/// Simple shell-like word splitter (handles quoted strings).
fn shell_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for ch in s.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

pub fn print_help() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                 open_music — 幫助                       ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("🎵 播放控制:");
    println!("  play | p [name|-n <name>|-a|--all]  播放歌曲/全部");
    println!("  pause                              暫停");
    println!("  resume | unpause                   繼續");
    println!("  next | n                           下一首");
    println!("  prev | previous | b                上一首");
    println!("  stop | s                           停止");
    println!("  volume | vol <0-100>               音量");
    println!("  seek | goto <secs>                 跳轉");
    println!("  repeat | loop <off|one|all>        循環模式");
    println!("  shuffle | random                   隨機切換");
    println!();
    println!("📋 佇列:");
    println!("  queue | q [add|show|clear]         佇列操作");
    println!("  queue add | q a <name|-a>          加入佇列");
    println!();
    println!("📚 音樂庫:");
    println!("  search | find <query>              搜尋");
    println!("  import <dir>                       匯入目錄");
    println!("  export [path]                      匯出庫");
    println!("  remove | rm <title>                刪除歌曲");
    println!("  edit <title> <field> <value>       編輯歌曲");
    println!("  lyrics <title> [text]              顯示/設定歌詞");
    println!("  stats                              統計資訊");
    println!("  history | hist                     播放歷史");
    println!();
    println!("📝 播放清單:");
    println!("  list | ls                          列出清單");
    println!("  show | cat <name>                  檢視清單");
    println!("  add-to-playlist <song> <playlist>  加入清單");
    println!();
    println!("⚙️  設定:");
    println!("  setconfig | set <key> <value>      設定配置");
    println!("  getconfig | get <key>              讀取配置");
    println!("  save | write                       儲存庫");
    println!("  load | read                        載入庫");
    println!();
    println!("🔌 系統:");
    println!("  plugin <args>                      外掛操作");
    println!("  help | h | ?                       顯示幫助");
    println!("  exit | quit | q!                   離開");
    println!();
    println!("💡 任何其他文字自動視為播放指令");
    println!();
}
