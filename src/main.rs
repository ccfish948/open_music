#![allow(dead_code)]

mod command;
mod config;
mod context;
mod library;
mod player;
mod plugin;
mod ui;

use std::io::{self, BufRead, Write};
use std::sync::{Arc, RwLock};

use clap::Parser;
use log::info;

use command::executor::execute_command;
use context::AppContext;
use library::persistence::LibraryStore;
use player::AudioBackend;
use player::Player;

// ── CLI args (clap) ──

#[derive(Parser)]
#[command(name = "open_music", version, about = "🎵 Music library manager")]
struct Cli {
    /// Run a command and exit (e.g. `play "song name"`, `stats`, `help`)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    cmd: Vec<String>,
}

fn main() {
    // ── 初始化日誌 ──
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .init();

    // ── 解析 CLI ──
    let cli = Cli::parse();

    info!("open_music v{} starting", env!("CARGO_PKG_VERSION"));

    // ── 載入設定 ──
    let config = match config::Config::load() {
        Ok(c) => {
            info!("config loaded from {}", config::Config::config_path().display());
            c
        }
        Err(e) => {
            eprintln!("⚠️  無法載入設定 (使用預設值): {e}");
            config::Config::default()
        }
    };

    let data_dir = config.general.data_dir.clone();
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        eprintln!("⚠️  無法建立資料目錄 {}: {e}", data_dir.display());
    }

    // ── 載入音樂庫 ──
    let store = LibraryStore::new(data_dir);
    let library = match store.load() {
        Ok(lib) => {
            let count = lib.get_all_songs().len();
            info!("library loaded ({} songs)", count);
            lib
        }
        Err(e) => {
            eprintln!("⚠️  無法載入音樂庫: {e}");
            library::Library::new()
        }
    };

    // ── 音訊後端 ──
    let audio_backend = Arc::new(RwLock::new(AudioBackend::try_new()));

    // ── 建立 Player ──
    let player = Arc::new(RwLock::new(Player::new(audio_backend)));

    // ── 外掛系統 ──
    let plugins = Arc::new(RwLock::new(plugin::PluginRegistry::new()));

    // ── 統一的 AppContext ──
    let ctx = AppContext::new(config, library, player, plugins, store);

    // ── 設定 hot-reload ──
    let cfg = ctx.config.clone();
    let _watcher = config::Config::watch(move || {
        info!("config file changed — reloading");
        match config::Config::load() {
            Ok(new_cfg) => {
                if let Ok(mut c) = cfg.write() {
                    *c = new_cfg;
                }
            }
            Err(e) => eprintln!("⚠️  config reload failed: {e}"),
        }
    })
    .ok();

    // ── 單次模式 vs REPL ──
    if !cli.cmd.is_empty() {
        let line = cli.cmd.join(" ");
        if let Some(cmd) = command::parse_command(&line) {
            execute_command(&ctx, cmd);
        }
        return;
    }

    // ── REPL 模式 ──
    println!("🎵 open_music v{} — 音樂庫管理員", env!("CARGO_PKG_VERSION"));
    println!("🔌 外掛架構 · 設定驅動 · 音訊輸出 (rodio)");
    println!("輸入 help 查看指令，exit 離開");
    println!();

    repl_loop(&ctx);
}

fn repl_loop(ctx: &AppContext) {
    let stdin = io::stdin();
    loop {
        print!("🎵 > ");
        if io::stdout().flush().is_err() {
            break;
        }

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                println!();
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("⚠️  讀取輸入錯誤: {e}");
                break;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match command::parse_command(trimmed) {
            Some(cmd) => execute_command(ctx, cmd),
            None => {
                let implicit = format!("play {}", trimmed);
                if let Some(cmd) = command::parse_command(&implicit) {
                    execute_command(ctx, cmd);
                }
            }
        }
    }
}
