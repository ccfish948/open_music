//! Shared application context — single source of truth for all subsystems.

use std::sync::{Arc, RwLock};

use crate::config::Config;
use crate::library::Library;
use crate::library::persistence::LibraryStore;
use crate::player::SharedPlayer;
use crate::plugin::PluginRegistry;

/// Shared configuration handle.
pub type SharedConfig = Arc<RwLock<Config>>;

/// Shared library handle.
pub type SharedLibrary = Arc<RwLock<Library>>;

/// Shared plugin registry.
pub type SharedPluginRegistry = Arc<RwLock<PluginRegistry>>;

/// Single application context shared across REPL, TUI, plugins, and watchers.
///
/// Every field is `Arc<RwLock<>>` so multiple consumers can operate concurrently.
pub struct AppContext {
    pub config: SharedConfig,
    pub library: SharedLibrary,
    pub player: SharedPlayer,
    pub plugins: SharedPluginRegistry,
    pub store: LibraryStore,
}

impl AppContext {
    pub fn new(
        config: Config,
        library: Library,
        player: SharedPlayer,
        plugins: SharedPluginRegistry,
        store: LibraryStore,
    ) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            library: Arc::new(RwLock::new(library)),
            player,
            plugins,
            store,
        }
    }
}
