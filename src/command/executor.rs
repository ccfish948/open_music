use std::path::PathBuf;

use crate::command::Command;
use crate::context::AppContext;
use crate::library::{Playlist, Song};
use crate::player::RepeatMode;

/// Execute a parsed Command against the application state.
pub fn execute_command(ctx: &AppContext, cmd: Command) {
    match cmd {
        Command::Play { name, all } => cmd_play(ctx, name, all),
        Command::Pause => cmd_pause(ctx),
        Command::Resume => cmd_resume(ctx),
        Command::Next => cmd_next(ctx),
        Command::Prev => cmd_prev(ctx),
        Command::Stop => cmd_stop(ctx),
        Command::Volume(vol) => cmd_volume(ctx, vol),
        Command::Seek(secs) => cmd_seek(ctx, secs),
        Command::Repeat(mode) => cmd_repeat(ctx, &mode),
        Command::Shuffle => cmd_shuffle(ctx),

        Command::QueueAdd { name, all } => cmd_queue_add(ctx, name, all),
        Command::QueueShow => cmd_queue_show(ctx),
        Command::QueueClear => cmd_queue_clear(ctx),

        Command::Search(query) => cmd_search(ctx, &query),
        Command::Import { path } => cmd_import(ctx, &path),
        Command::Export { path } => cmd_export(ctx, &path),
        Command::RemoveSong(title) => cmd_remove_song(ctx, &title),
        Command::EditSong { title, field, value } => cmd_edit_song(ctx, &title, &field, &value),
        Command::Lyrics { title, text } => cmd_lyrics(ctx, &title, text.as_deref()),
        Command::Stats => cmd_stats(ctx),
        Command::History => cmd_history(ctx),

        Command::ListPlaylists => cmd_list_playlists(ctx),
        Command::ShowPlaylist(name) => cmd_show_playlist(ctx, &name),
        Command::AddToPlaylist { song, playlist } => cmd_add_to_playlist(ctx, &song, &playlist),

        Command::SetConfig { key, value } => cmd_set_config(ctx, &key, &value),
        Command::GetConfig(key) => cmd_get_config(ctx, &key),

        Command::Save => cmd_save(ctx),
        Command::Load => cmd_load(ctx),

        Command::Plugin(_args) => {
            println!("🔌 外掛系統 — 插件可註冊指令到 PluginRegistry");
        }
        Command::Help => crate::command::print_help(),
        Command::Exit => {
            cmd_save(ctx);
            println!("👋 再見！");
            std::process::exit(0);
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  Playback
// ════════════════════════════════════════════════════════════════════════════

fn cmd_play(ctx: &AppContext, name: Option<String>, all: bool) {
    let mut player = ctx.player.write().unwrap();

    if all {
        let library = ctx.library.read().unwrap();
        for (i, song) in library.get_all_songs().iter().enumerate() {
            let src = song.source.clone().unwrap_or_default();
            player.play(i, song.title.as_deref().unwrap_or("Untitled"), PathBuf::from(&src));
        }
        let count = library.get_all_songs().len();
        println!("▶️  已將 {} 首歌曲加入佇列", count);
        return;
    }

    if let Some(n) = &name {
        let library = ctx.library.read().unwrap();
        let songs = library.find_song(n);
        match songs.len() {
            0 => println!("❌ 找不到歌曲: {}", n),
            1 => {
                let song = songs[0];
                let id = library.get_all_songs().iter().position(|s| s.title == song.title).unwrap_or(0);
                let src = song.source.clone().unwrap_or_default();
                player.play(id, song.title.as_deref().unwrap_or("Untitled"), PathBuf::from(&src));
                println!("▶️  播放: {}", song.title.as_deref().unwrap_or("Untitled"));
            }
            _ => {
                println!("📋 找到多首歌，請指定其中之一:");
                for s in &songs {
                    println!("  • {}", s.title.as_deref().unwrap_or("Untitled"));
                }
            }
        }
    } else {
        if player.queue_list().is_empty() {
            println!("📭 佇列為空，使用 `play <歌名>` 或 `search` 加入歌曲");
        } else {
            player.resume();
            if let Some(song) = player.current_song() {
                println!("▶️  繼續播放: {}", song.song_title);
            }
        }
    }
}

fn cmd_pause(ctx: &AppContext) {
    ctx.player.write().unwrap().pause();
    println!("⏸️  已暫停");
}

fn cmd_resume(ctx: &AppContext) {
    let mut player = ctx.player.write().unwrap();
    player.resume();
    if let Some(song) = player.current_song() {
        println!("▶️  繼續播放: {}", song.song_title);
    }
}

fn cmd_next(ctx: &AppContext) {
    let mut player = ctx.player.write().unwrap();
    player.play_next();
    if let Some(song) = player.current_song() {
        println!("⏭️  下一首: {}", song.song_title);
    } else {
        println!("⏹️  佇列結束");
    }
}

fn cmd_prev(ctx: &AppContext) {
    let mut player = ctx.player.write().unwrap();
    player.play_prev();
    if let Some(song) = player.current_song() {
        println!("⏮️  上一首: {}", song.song_title);
    }
}

fn cmd_stop(ctx: &AppContext) {
    ctx.player.write().unwrap().stop();
    println!("⏹️  已停止");
}

fn cmd_volume(ctx: &AppContext, vol: u8) {
    let vol_f32 = (vol as f32) / 100.0;
    ctx.player.write().unwrap().set_volume(vol_f32);
    println!("🔊 音量: {}%", vol);
}

fn cmd_seek(ctx: &AppContext, secs: u64) {
    ctx.player.write().unwrap().seek(secs);
    let m = secs / 60;
    let s = secs % 60;
    println!("⏩ 跳轉至 {:02}:{:02}", m, s);
}

fn cmd_repeat(ctx: &AppContext, mode: &str) {
    let repeat_mode = match mode {
        "one" | "1" | "single" => RepeatMode::One,
        "all" => RepeatMode::All,
        "off" | "0" | "none" => RepeatMode::Off,
        _ => {
            println!("❌ 無效模式: {} (可用: off, one, all)", mode);
            return;
        }
    };
    ctx.player.write().unwrap().set_repeat(repeat_mode);
    let label = match ctx.player.read().unwrap().repeat {
        RepeatMode::Off => "關閉",
        RepeatMode::One => "單曲循環",
        RepeatMode::All => "全部循環",
    };
    println!("🔁 循環模式: {}", label);
}

fn cmd_shuffle(ctx: &AppContext) {
    let mut player = ctx.player.write().unwrap();
    player.toggle_shuffle();
    let state = if player.shuffle { "開啟" } else { "關閉" };
    println!("🔀 隨機播放: {}", state);
}

// ════════════════════════════════════════════════════════════════════════════
//  Queue
// ════════════════════════════════════════════════════════════════════════════

fn cmd_queue_add(ctx: &AppContext, name: Option<String>, all: bool) {
    if all {
        let library = ctx.library.read().unwrap();
        let count = library.get_all_songs().len();
        let mut player = ctx.player.write().unwrap();
        for (i, song) in library.get_all_songs().iter().enumerate() {
            let src = song.source.clone().unwrap_or_default();
            player.queue_add(vec![crate::player::QueueEntry {
                song_id: i,
                song_title: song.title.clone().unwrap_or_default(),
                source_path: PathBuf::from(&src),
            }]);
        }
        println!("📋 已將 {} 首加入佇列", count);
    } else if let Some(n) = &name {
        let library = ctx.library.read().unwrap();
        let songs = library.find_song(n);
        if songs.is_empty() {
            println!("❌ 找不到歌曲: {}", n);
            return;
        }
        let mut player = ctx.player.write().unwrap();
        for &song in &songs {
            let id = library.get_all_songs().iter().position(|s| s.title == song.title).unwrap_or(0);
            let src = song.source.clone().unwrap_or_default();
            player.queue_add(vec![crate::player::QueueEntry {
                song_id: id,
                song_title: song.title.clone().unwrap_or_default(),
                source_path: PathBuf::from(&src),
            }]);
            println!("📋 已加入: {}", song.title.as_deref().unwrap_or("Untitled"));
        }
    } else {
        println!("❌ 用法: queue add <歌名> 或 queue add -a (全部)");
    }
}

fn cmd_queue_show(ctx: &AppContext) {
    let player = ctx.player.read().unwrap();
    let queue = player.queue_list();
    if queue.is_empty() {
        println!("📭 佇列為空");
        return;
    }
    println!("📋 播放佇列 ({} 首):", queue.len());
    for (i, entry) in queue.iter().enumerate() {
        let marker = if player.current_index == Some(i) { " ▶" } else { "  " };
        println!("{}{}. {}", marker, i + 1, entry.song_title);
    }
}

fn cmd_queue_clear(ctx: &AppContext) {
    ctx.player.write().unwrap().queue_clear();
    println!("🗑️  佇列已清空");
}

// ════════════════════════════════════════════════════════════════════════════
//  Library
// ════════════════════════════════════════════════════════════════════════════

fn cmd_search(ctx: &AppContext, query: &str) {
    let results = {
        let library = ctx.library.read().unwrap();
        let all_songs: Vec<Song> = library.get_all_songs().iter().cloned().collect();
        // drop library lock so we don't hold borrow
        drop(library);

        all_songs
            .iter()
            .filter(|s| {
                let q = query.to_lowercase();
                s.title.as_deref().map_or(false, |t| t.to_lowercase().contains(&q))
                    || s.hashtags.as_ref().map_or(false, |tags| tags.iter().any(|t| t.to_lowercase().contains(&q)))
                    || s.credits.as_ref().map_or(false, |c| c.values().any(|cr| cr.name.to_lowercase().contains(&q)))
            })
            .cloned()
            .collect::<Vec<Song>>()
    };

    if results.is_empty() {
        println!("🔍 找不到「{}」的結果", query);
        return;
    }
    println!("🔍 搜尋「{}」— {} 個結果:", query, results.len());
    for song in &results {
        let title = song.title.as_deref().unwrap_or("Untitled");
        let dur = song.duration.map(|d| format!("{:02}:{:02}", d / 60, d % 60)).unwrap_or_default();
        println!("  • {} [{}]", title, dur);
    }
}

fn cmd_import(ctx: &AppContext, path: &PathBuf) {
    if !path.is_dir() {
        println!("❌ 不是有效的目錄: {}", path.display());
        return;
    }

    let extensions = ["mp3", "flac", "wav", "ogg", "m4a", "aac"];
    let mut count = 0;

    let mut library = ctx.library.write().unwrap();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_path = entry.path();
            if file_path.is_file() {
                if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                    if extensions.contains(&ext.to_lowercase().as_str()) {
                        let title = file_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        let mut song = Song::new();
                        song.title = Some(title);
                        if let Some(d) = get_duration(&file_path) {
                            song.duration = Some(d);
                        }
                        song.source = Some(file_path.to_string_lossy().to_string());
                        library.add_song(song);
                        count += 1;
                    }
                }
            }
        }
    }
    drop(library);
    println!("📥 已匯入 {} 首歌曲", count);
    cmd_save(ctx);
}

fn cmd_export(ctx: &AppContext, path: &PathBuf) {
    let config = ctx.config.read().unwrap();
    let export_path = if path.as_os_str().is_empty() {
        let mut p = config.general.data_dir.clone();
        p.push("library_export.json");
        p
    } else {
        path.clone()
    };
    drop(config);

    let library = ctx.library.read().unwrap();
    match std::fs::write(&export_path, serde_json::to_string_pretty(&*library).unwrap()) {
        Ok(_) => println!("📤 已匯出至 {}", export_path.display()),
        Err(e) => println!("❌ 匯出失敗: {}", e),
    }
}

fn cmd_remove_song(ctx: &AppContext, title: &str) {
    let mut library = ctx.library.write().unwrap();
    if library.remove_song(title).is_some() {
        println!("🗑️  已刪除: {}", title);
        drop(library);
        cmd_save(ctx);
    } else {
        println!("❌ 找不到歌曲: {}", title);
    }
}

fn cmd_edit_song(ctx: &AppContext, title: &str, field: &str, value: &str) {
    let mut library = ctx.library.write().unwrap();
    let indices: Vec<usize> = library.find_song(title).iter()
        .filter_map(|&s| library.get_all_songs().iter().position(|ls| ls.title == s.title))
        .collect();

    if indices.is_empty() {
        println!("❌ 找不到歌曲: {}", title);
        return;
    }

    let idx = indices[0];
    let song = &mut library.songs[idx];
    match field {
        "title" => song.title = Some(value.to_string()),
        "description" | "desc" => song.description = Some(value.to_string()),
        "hashtags" | "tags" => {
            song.hashtags = Some(value.split(',').map(|t| t.trim().to_string()).collect());
        }
        "credits" | "credit" => {
            song.credits = serde_json::from_str(value).ok();
        }
        "album" => {
            song.album = Some(crate::library::Album::new(value));
        }
        _ => {
            println!("❌ 未知欄位: {} (可用: title, description, hashtags, credits, album)", field);
            return;
        }
    }
    println!("✅ 已更新 {} 的 {}", field, song.title.as_deref().unwrap_or("Untitled"));
    drop(library);
    cmd_save(ctx);
}

fn cmd_lyrics(ctx: &AppContext, title: &str, text: Option<&str>) {
    let mut library = ctx.library.write().unwrap();
    let idx = library.find_song(title).first()
        .and_then(|&s| library.get_all_songs().iter().position(|ls| ls.title == s.title));

    match (idx, text) {
        (Some(i), Some(t)) => {
            library.songs[i].lyrics = Some(t.to_string());
            println!("📝 已儲存歌詞");
            drop(library);
            cmd_save(ctx);
        }
        (Some(i), None) => {
            match &library.songs[i].lyrics {
                Some(l) => {
                    let t = library.songs[i].title.as_deref().unwrap_or("Untitled");
                    println!("📝 歌詞 — {}:\n", t);
                    println!("{}", l);
                }
                None => println!("📝 沒有歌詞 — 使用 lyrics <歌名> <文字> 設定"),
            }
        }
        (None, _) => println!("❌ 找不到歌曲: {}", title),
    }
}

fn cmd_stats(ctx: &AppContext) {
    let library = ctx.library.read().unwrap();
    let songs = library.get_all_songs();
    let total_songs = songs.len();
    let total_duration: u64 = songs.iter().filter_map(|s| s.duration).map(|d| d as u64).sum();
    let total_played: u32 = songs.iter().map(|s| s.played_count).sum();
    let with_source = songs.iter().filter(|s| s.source.is_some()).count();
    let with_lyrics = songs.iter().filter(|s| s.lyrics.is_some()).count();
    let playlist_count = library.get_all_playlists().len();
    drop(library);

    let hours = total_duration / 3600;
    let minutes = (total_duration % 3600) / 60;
    println!("📊 音樂庫統計:");
    println!("  歌曲總數:     {}", total_songs);
    println!("  總時長:       {:02}:{:02} 小時", hours, minutes);
    println!("  有來源檔案:   {}", with_source);
    println!("  有歌詞:       {}", with_lyrics);
    println!("  總播放次數:   {}", total_played);
    println!("  播放清單數:   {}", playlist_count);
}

fn cmd_history(ctx: &AppContext) {
    let player = ctx.player.read().unwrap();
    if player.history.is_empty() {
        println!("📭 沒有播放歷史");
        return;
    }
    println!("📜 播放歷史 (最近 {} 首):", player.history.len());
    for (i, entry) in player.history.iter().rev().enumerate() {
        println!("  {}. {}", i + 1, entry.song_title);
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  Playlist
// ════════════════════════════════════════════════════════════════════════════

fn cmd_list_playlists(ctx: &AppContext) {
    let library = ctx.library.read().unwrap();
    let playlists = library.get_all_playlists();
    if playlists.is_empty() {
        println!("📭 沒有播放清單");
        return;
    }
    println!("📝 播放清單:");
    for pl in playlists {
        let name = pl.name.as_deref().unwrap_or("(未命名)");
        let count = pl.playlist.as_ref().map(|v| v.len()).unwrap_or(0);
        println!("  • {} ({} 首)", name, count);
    }
}

fn cmd_show_playlist(ctx: &AppContext, name: &str) {
    let library = ctx.library.read().unwrap();
    let pl = library.get_all_playlists().iter()
        .find(|p| p.name.as_deref().map_or(false, |n| n.contains(name)))
        .cloned();
    match pl {
        Some(p) => {
            let pname = p.name.as_deref().unwrap_or("(未命名)");
            println!("📝 播放清單: {}", pname);
            if let Some(songs) = &p.playlist {
                for (i, song) in songs.iter().enumerate() {
                    let title = song.title.as_deref().unwrap_or("Untitled");
                    let dur = song.duration.map(|d| format!("{:02}:{:02}", d / 60, d % 60)).unwrap_or_default();
                    println!("  {}. {} [{}]", i + 1, title, dur);
                }
            }
        }
        None => println!("❌ 找不到播放清單: {}", name),
    }
}

fn cmd_add_to_playlist(ctx: &AppContext, song_title: &str, playlist_name: &str) {
    let mut library = ctx.library.write().unwrap();
    let songs = library.find_song(song_title);
    if songs.is_empty() {
        println!("❌ 找不到歌曲: {}", song_title);
        return;
    }
    let song_clone = songs[0].clone();

    let mut found = false;
    for pl in &mut library.playlists {
        if pl.name.as_deref().map_or(false, |n| n.contains(playlist_name)) {
            pl.add_song(song_clone.clone());
            found = true;
            println!("✅ 已將歌曲加入播放清單「{}」", pl.name.as_deref().unwrap_or("(未命名)"));
            break;
        }
    }
    if !found {
        let mut pl = Playlist::new();
        pl.name(playlist_name);
        pl.add_song(song_clone);
        library.add_playlist(pl);
        println!("✅ 已創建播放清單「{}」並加入歌曲", playlist_name);
    }
    drop(library);
    cmd_save(ctx);
}

// ════════════════════════════════════════════════════════════════════════════
//  Config
// ════════════════════════════════════════════════════════════════════════════

fn cmd_set_config(ctx: &AppContext, key: &str, value: &str) {
    let mut config = ctx.config.write().unwrap();
    match config.set(key, value) {
        Ok(()) => {
            println!("⚙️  已設定 {} = {}", key, value);
            if let Err(e) = config.save() {
                println!("❌ 無法儲存設定: {}", e);
            }
        }
        Err(e) => println!("❌ {}", e),
    }
}

fn cmd_get_config(ctx: &AppContext, key: &str) {
    let config = ctx.config.read().unwrap();
    if key.is_empty() {
        println!("⚙️  目前設定:");
        println!("  general.data_dir = {}", config.general.data_dir.display());
        println!("  general.auto_save = {}", config.general.auto_save);
        println!("  general.auto_save_interval_secs = {}", config.general.auto_save_interval_secs);
        println!("  audio.default_volume = {}", config.audio.default_volume);
        println!("  audio.output_device = {:?}", config.audio.output_device);
        println!("  ui.theme = {}", config.ui.theme);
        println!("  ui.font_size = {}", config.ui.font_size);
        for (k, v) in &config.keybinds {
            println!("  keybinds.{} = {}", k, v);
        }
        return;
    }
    match config.get(key) {
        Some(v) => println!("⚙️  {} = {}", key, v),
        None => println!("❌ 未知配置鍵: {}", key),
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  Persistence
// ════════════════════════════════════════════════════════════════════════════

fn cmd_save(ctx: &AppContext) {
    let library = ctx.library.read().unwrap();
    match ctx.store.save(&library) {
        Ok(()) => println!("💾 已儲存"),
        Err(e) => println!("❌ 儲存失敗: {}", e),
    }
}

fn cmd_load(ctx: &AppContext) {
    match ctx.store.load() {
        Ok(lib) => {
            let count = lib.get_all_songs().len();
            *ctx.library.write().unwrap() = lib;
            println!("📂 已載入 ({} 首歌曲)", count);
        }
        Err(e) => println!("❌ 載入失敗: {}", e),
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  Helper
// ════════════════════════════════════════════════════════════════════════════

fn get_duration(path: &PathBuf) -> Option<u32> {
    let hint = symphonia::core::formats::probe::Hint::new();
    let format_opts = symphonia::core::formats::FormatOptions::default();
    let meta_opts = symphonia::core::meta::MetadataOptions::default();
    if let Ok(file) = std::fs::File::open(path) {
        let mss = symphonia::core::io::MediaSourceStream::new(Box::new(file), Default::default());
        if let Ok(format) = symphonia::default::get_probe().probe(&hint, mss, format_opts, meta_opts) {
            return format.tracks().first().and_then(|t| t.num_frames)
                .zip(format.tracks().first().and_then(|t| t.time_base))
                .map(|(frames, tb)| (frames as f64 * tb.numer.get() as f64 / tb.denom.get() as f64) as u32);
        }
    }
    None
}
