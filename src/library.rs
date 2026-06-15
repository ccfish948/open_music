use std::path::PathBuf;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

pub mod persistence;

// ── base64 序列化模組 ──
// 讓 Option<Vec<u8>> 在 JSON 裡呈現為 base64 字串
mod opt_base64 {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match data {
            Some(bytes) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                s.serialize_str(&b64)
            }
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(d)?;
        match opt {
            Some(s) => base64::engine::general_purpose::STANDARD
                .decode(&s)
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

// ── Playlist ──
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub name: Option<String>,
    #[serde(with = "opt_base64", default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<Vec<u8>>,
    pub playlist: Option<Vec<Song>>,
    pub description: Option<String>,
}

impl Playlist {
    pub fn new() -> Self {
        Self {
            name: None,
            cover: None,
            playlist: None,
            description: None,
        }
    }

    pub fn name(&mut self, name: &str) -> &mut Self {
        self.name = Some(name.to_string());
        self
    }

    /// 從圖檔路徑載入 cover bytes
    pub fn cover(&mut self, path: &str) -> &mut Self {
        self.cover = std::fs::read(path).ok();
        self
    }

    pub fn description(&mut self, description: &str) -> &mut Self {
        self.description = Some(description.to_string());
        self
    }

    /// 直接設定 cover bytes（匯入時用）
    pub fn set_cover(&mut self, data: Vec<u8>) -> &mut Self {
        self.cover = Some(data);
        self
    }

    pub fn add_song(&mut self, song: Song) -> &mut Self {
        self.playlist.get_or_insert(Vec::new()).push(song);
        self
    }
}

// ── Creditor ──
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creditor {
    pub name: String,
    #[serde(with = "opt_base64", default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Vec<u8>>,
    pub description: Option<String>,
}

impl Creditor {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            icon: None,
            description: None,
        }
    }

    pub fn icon(&mut self, path: &str) -> &mut Self {
        self.icon = std::fs::read(path).ok();
        self
    }

    pub fn set_icon(&mut self, data: Vec<u8>) -> &mut Self {
        self.icon = Some(data);
        self
    }

    pub fn description(&mut self, desc: &str) -> &mut Self {
        self.description = Some(desc.to_string());
        self
    }
}

// ── Album ──
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub title: String,
    #[serde(with = "opt_base64", default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<Vec<u8>>,
}

impl Album {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            cover: None,
        }
    }

    pub fn cover(&mut self, path: &str) -> &mut Self {
        self.cover = std::fs::read(path).ok();
        self
    }

    pub fn set_cover(&mut self, data: Vec<u8>) -> &mut Self {
        self.cover = Some(data);
        self
    }
}

// ── Song ──
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub title: Option<String>,
    pub credits: Option<HashMap<String, Creditor>>,
    pub duration: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub album: Option<Album>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lyrics: Option<String>,
    #[serde(default)]
    pub played_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<String>,
    pub description: Option<String>,
    pub hashtags: Option<Vec<String>>,
}

impl Song {
    pub fn new() -> Self {
        Self {
            title: None,
            credits: None,
            duration: None,
            source: None,
            album: None,
            lyrics: None,
            played_count: 0,
            last_played: None,
            description: None,
            hashtags: None,
        }
    }

    pub fn title(&mut self, title: &str) -> &mut Self {
        self.title = Some(title.to_string());
        self
    }

    pub fn credits(&mut self, credits: &str) -> &mut Self {
        self.credits = serde_json::from_str(credits).ok();
        self
    }

    pub fn source(&mut self, path: PathBuf) -> &mut Self {
        if path.exists() {
            // read the duration by symphonia
            let hint = symphonia::core::formats::probe::Hint::new();
            let format_opts = symphonia::core::formats::FormatOptions::default();
            let meta_opts = symphonia::core::meta::MetadataOptions::default();

            if let Ok(file) = std::fs::File::open(&path) {
                let mss = symphonia::core::io::MediaSourceStream::new(
                    Box::new(file),
                    Default::default(),
                );
                if let Ok(format) = symphonia::default::get_probe()
                    .probe(&hint, mss, format_opts, meta_opts)
                {
                    self.duration = format
                        .tracks()
                        .first()
                        .and_then(|t| t.num_frames)
                        .zip(format.tracks().first().and_then(|t| t.time_base))
                        .map(|(frames, tb)| {
                            (frames as f64 * tb.numer.get() as f64 / tb.denom.get() as f64) as u32
                        });
                }
            }
            // save as path
            self.source = Some(path.to_string_lossy().to_string());
        }
        self
    }

    pub fn lyrics(&mut self, lyrics: &str) -> &mut Self {
        self.lyrics = Some(lyrics.to_string());
        self
    }
}

// ── Library ──
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub songs: Vec<Song>,
    pub playlists: Vec<Playlist>,
}

impl Library {
    pub fn new() -> Self {
        Self {
            songs: Vec::new(),
            playlists: Vec::new(),
        }
    }

    pub fn add_song(&mut self, song: Song) {
        self.songs.push(song);
    }

    pub fn remove_song(&mut self, title: &str) -> Option<Song> {
        if let Some(pos) = self
            .songs
            .iter()
            .position(|s| s.title.as_deref().map_or(false, |t| t.contains(title)))
        {
            Some(self.songs.remove(pos))
        } else {
            None
        }
    }

    pub fn add_playlist(&mut self, playlist: Playlist) {
        self.playlists.push(playlist);
    }

    pub fn remove_playlist(&mut self, name: &str) -> Option<Playlist> {
        if let Some(pos) = self.playlists.iter().position(|p| {
            p.name
                .as_deref()
                .map_or(false, |n| n.contains(name))
        }) {
            Some(self.playlists.remove(pos))
        } else {
            None
        }
    }

    pub fn find_song(&self, query: &str) -> Vec<&Song> {
        self.songs
            .iter()
            .filter(|s| s.title.as_deref().map_or(false, |t| t.contains(query)))
            .collect()
    }

    pub fn find_songs_by_hashtag(&self, tag: &str) -> Vec<&Song> {
        self.songs
            .iter()
            .filter(|s| {
                s.hashtags
                    .as_ref()
                    .map_or(false, |tags| tags.iter().any(|t| t == tag))
            })
            .collect()
    }

    pub fn find_songs_by_creditor(&self, name: &str) -> Vec<&Song> {
        self.songs
            .iter()
            .filter(|s| {
                s.credits.as_ref().map_or(false, |credits| {
                    credits.values().any(|c| c.name.contains(name))
                })
            })
            .collect()
    }

    pub fn get_all_songs(&self) -> &[Song] {
        &self.songs
    }

    pub fn get_all_playlists(&self) -> &[Playlist] {
        &self.playlists
    }
}

impl Default for Library {
    fn default() -> Self {
        Self::new()
    }
}
