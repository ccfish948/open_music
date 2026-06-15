use std::fs;
use std::path::PathBuf;

use thiserror::Error;

use super::Library;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct LibraryStore {
    data_dir: PathBuf,
    pub auto_save: bool,
}

impl LibraryStore {
    /// Create a new LibraryStore with an explicit data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            auto_save: true,
        }
    }

    /// Returns the default data directory: `dirs::data_dir()/open_music/`.
    pub fn default_data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("open_music")
    }

    /// Full path to the library JSON file.
    fn library_path(&self) -> PathBuf {
        self.data_dir.join("library.json")
    }

    /// Load the Library from `<data_dir>/library.json`.
    /// Returns a default (empty) Library if the file does not exist.
    pub fn load(&self) -> Result<Library, StoreError> {
        let path = self.library_path();
        if path.exists() {
            let data = fs::read_to_string(&path)?;
            let library: Library = serde_json::from_str(&data)?;
            Ok(library)
        } else {
            Ok(Library::default())
        }
    }

    /// Serialize the Library and write it to `<data_dir>/library.json`.
    /// Creates the data directory if it does not exist.
    pub fn save(&self, library: &Library) -> Result<(), StoreError> {
        fs::create_dir_all(&self.data_dir)?;
        let json = serde_json::to_string_pretty(library)?;
        fs::write(self.library_path(), json)?;
        Ok(())
    }

    /// Save the library only if `auto_save` is enabled.
    pub fn save_if_needed(&self, library: &Library) -> Result<(), StoreError> {
        if self.auto_save {
            self.save(library)
        } else {
            Ok(())
        }
    }
}
