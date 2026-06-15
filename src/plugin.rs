//! Plugin system for open_music.
//!
//! Plugins can extend the application with custom commands, register config
//! defaults, and hook into the application lifecycle via [`Plugin::on_init`].
//!
//! The central registry ([`PluginRegistry`]) is thread-safe and designed to be
//! shared across the application via `Arc<RwLock<PluginRegistry>>`.

use std::collections::HashMap;

use log::info;
use thiserror::Error;

use crate::context::AppContext;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during plugin operations.
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("plugin '{name}' is already registered")]
    AlreadyRegistered { name: String },

    #[error("plugin '{name}' is not registered")]
    NotRegistered { name: String },

    #[error("plugin command '{command}' failed: {message}")]
    CommandFailed { command: String, message: String },

    #[error("plugin '{plugin}' init failed: {message}")]
    InitFailed { plugin: String, message: String },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience alias for results from plugin operations.
pub type PluginResult<T> = Result<T, PluginError>;

// ---------------------------------------------------------------------------
// PluginCommand
// ---------------------------------------------------------------------------

/// Describes a single command exposed by a plugin.
///
/// Commands are invocable actions that appear in the CLI and (optionally) the
/// TUI command palette.
#[derive(Clone)]
pub struct PluginCommand {
    /// Unique command name, e.g. `"fetch-lyrics"`.
    pub name: &'static str,

    /// Short human-readable description shown in help text.
    pub description: &'static str,

    /// The function that executes the command.
    ///
    /// Receives a reference to [`AppContext`] plus any positional arguments
    /// passed on the command line after the command name.
    pub handler: fn(ctx: &AppContext, args: &[String]) -> PluginResult<()>,
}

impl PluginCommand {
    /// Create a new command descriptor.
    pub const fn new(
        name: &'static str,
        description: &'static str,
        handler: fn(&AppContext, &[String]) -> PluginResult<()>,
    ) -> Self {
        Self {
            name,
            description,
            handler,
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin trait
// ---------------------------------------------------------------------------

/// The trait every open_music plugin must implement.
pub trait Plugin: Send + Sync {
    /// Human-readable name (used as the registry key).
    fn name(&self) -> &'static str;

    /// Semver version string, e.g. `"1.2.0"`.
    fn version(&self) -> &'static str;

    /// Called once after the plugin is registered and the application context
    /// is available. Plugins can use this to inject per-plugin config defaults
    /// or perform one-time setup.
    fn on_init(&self, _ctx: &AppContext) -> PluginResult<()> {
        Ok(())
    }

    /// Return the commands this plugin exposes to the CLI / TUI.
    fn commands(&self) -> Vec<PluginCommand> {
        Vec::new()
    }

    /// Return the default configuration values for this plugin.
    ///
    /// These are merged into the `[plugins.<name>]` table when the plugin is
    /// first loaded, and **do not** overwrite existing user values.
    fn config_defaults(&self) -> HashMap<String, toml::Value> {
        HashMap::new()
    }
}

// ---------------------------------------------------------------------------
// PluginRegistry
// ---------------------------------------------------------------------------

/// Thread-safe registry of all loaded plugins.
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn Plugin>>,
    /// Flattened command map: `command_name → (plugin_name, command_index)`.
    command_index: HashMap<String, (String, usize)>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            command_index: HashMap::new(),
        }
    }

    // -- Registration -------------------------------------------------------

    /// Register a plugin.
    ///
    /// Calls [`Plugin::on_init`] before inserting so the plugin can set up.
    /// Returns an error if a plugin with the same name is already present.
    pub fn register(
        &mut self,
        plugin: Box<dyn Plugin>,
        ctx: &AppContext,
    ) -> PluginResult<()> {
        let name = plugin.name().to_string();

        if self.plugins.contains_key(&name) {
            return Err(PluginError::AlreadyRegistered { name });
        }

        // Let the plugin initialise itself.
        plugin.on_init(ctx).map_err(|e| PluginError::InitFailed {
            plugin: name.clone(),
            message: e.to_string(),
        })?;

        info!("registered plugin '{}' v{}", name, plugin.version());

        // Index commands.
        for (idx, cmd) in plugin.commands().iter().enumerate() {
            self.command_index
                .insert(cmd.name.to_string(), (name.clone(), idx));
        }

        self.plugins.insert(name, plugin);
        Ok(())
    }

    /// Unregister a plugin by name.
    pub fn unregister(&mut self, name: &str) -> PluginResult<Box<dyn Plugin>> {
        let plugin = self
            .plugins
            .remove(name)
            .ok_or_else(|| PluginError::NotRegistered {
                name: name.to_string(),
            })?;

        // Purge any commands contributed by this plugin.
        self.command_index.retain(|_, (pn, _)| pn != name);

        info!("unregistered plugin '{}'", name);
        Ok(plugin)
    }

    /// Get a reference to a plugin by name.
    pub fn get(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    /// Return the names of all registered plugins.
    pub fn list(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }

    /// Return every registered plugin name and version.
    pub fn list_detailed(&self) -> Vec<(&str, &str)> {
        self.plugins
            .values()
            .map(|p| (p.name(), p.version()))
            .collect()
    }

    /// Look up a command by name.
    pub fn find_command(&self, cmd_name: &str) -> Option<(&dyn Plugin, PluginCommand)> {
        let (plugin_name, idx) = self.command_index.get(cmd_name)?;
        let plugin = self.plugins.get(plugin_name)?;
        let cmd = plugin.commands().get(*idx)?.clone();
        Some((plugin.as_ref(), cmd))
    }

    /// Number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Returns `true` if no plugins are registered.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
