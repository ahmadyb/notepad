//! Headless, thread-safe logic used by the native NotePad Pro shell.
pub mod config;
pub mod db;
pub mod editor;
pub mod files;
pub mod highlight;
pub mod types;

pub use config::{SessionStore, SettingsStore};
pub use db::NotesStore;
pub use editor::{find_all,replace_all,replace_first,EditorBuffer,FindMatch,FindOptions,MetadataOverlay,Selection};
pub use files::{FileManager, LineEnding, TextEncoding};
pub use highlight::{extract_by_colour, highlight_stats, HighlightStats};
pub use types::{AppInfo, ColorOrder, ExtractOrder, LineColour, LineMetadata, ListType, LoadedFile, Note, NoteData, SessionData, SessionTab, Settings, WindowState};

pub const APP_NAME: &str = "NotePad Pro";
pub const APP_VERSION: &str = "1.0.2-scintilla";
pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("database pool error: {0}")]
    Pool(String),
    #[error("invalid setting `{key}`: {message}")]
    InvalidSetting { key: String, message: String },
    #[error("invalid value: {0}")]
    InvalidValue(String),
    #[error("encoding error: {0}")]
    Encoding(String),
}
impl From<r2d2::Error> for CoreError { fn from(e: r2d2::Error) -> Self { Self::Pool(e.to_string()) } }
