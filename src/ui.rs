//! TUI — Terminal User Interface (stub for future ratatui-based GUI).
//!
//! Currently provides a placeholder that logs TUI startup.
//! In future versions this will use `crossterm` + `ratatui` for a full
//! terminal interface with playlist browsing, now-playing display, etc.

use log::info;

/// Initialize and run the TUI (blocking).
pub fn run_tui() {
    info!("TUI stub started — falling back to REPL");
    println!("🐚 TUI 未就緒，使用 REPL 模式。輸入 help 查看指令。");
}
