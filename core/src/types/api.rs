use crate::files::{LineEnding, TextEncoding};
use crate::types::{LineColour, LineMetadata};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

impl Default for AppInfo {
    fn default() -> Self {
        Self {
            name: "NotePad Pro".into(),
            version: "1.0.2-scintilla".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColorOrder {
    #[default]
    Document,
    Grouped,
}

pub type ExtractOrder = ColorOrder;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
    pub sidebar_open: bool,
    pub show_line_numbers: bool,
    pub autosave: bool,
    pub autosave_seconds: u64,
    pub font_family: String,
    pub font_size: f32,
    pub tab_width: u8,
    pub recent_files_limit: usize,
    pub default_encoding: TextEncoding,
    pub default_line_ending: LineEnding,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            sidebar_open: true,
            show_line_numbers: true,
            autosave: true,
            autosave_seconds: 30,
            font_family: "DejaVu Sans Mono".into(),
            font_size: 15.0,
            tab_width: 4,
            recent_files_limit: 20,
            default_encoding: TextEncoding::Utf8,
            default_line_ending: LineEnding::Lf,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> std::result::Result<(), String> {
        const THEMES: [&str; 7] = [
            "light",
            "dark",
            "glass_dark",
            "clay_light",
            "clay_dark",
            "neumorphic_light",
            "neumorphic_dark",
        ];
        if !THEMES.contains(&self.theme.as_str()) {
            return Err(format!("unknown theme `{}`", self.theme));
        }
        if !(8.0..=96.0).contains(&self.font_size) {
            return Err("font_size must be between 8 and 96".into());
        }
        if !(1..=16).contains(&self.tab_width) {
            return Err("tab_width must be between 1 and 16".into());
        }
        if !(1..=3600).contains(&self.autosave_seconds) {
            return Err("autosave_seconds must be between 1 and 3600".into());
        }
        if self.recent_files_limit > 200 {
            return Err("recent_files_limit must be at most 200".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTab {
    pub path: Option<PathBuf>,
    pub title: String,
    pub text: String,
    pub cursor: usize,
    pub metadata: Vec<LineMetadata>,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    #[serde(default)]
    pub had_bom: bool,
    pub modified: bool,
}

impl Default for SessionTab {
    fn default() -> Self {
        Self {
            path: None,
            title: "Untitled".into(),
            text: String::new(),
            cursor: 0,
            metadata: vec![LineMetadata::default()],
            encoding: TextEncoding::Utf8,
            line_ending: LineEnding::Lf,
            had_bom: false,
            modified: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionData {
    pub tabs: Vec<SessionTab>,
    pub active_tab: usize,
    pub window: WindowState,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            tabs: vec![SessionTab::default()],
            active_tab: 0,
            window: WindowState::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            x: 80,
            y: 80,
            width: 1180,
            height: 760,
            maximized: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedFile {
    pub path: PathBuf,
    pub text: String,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    pub had_bom: bool,
}

impl LoadedFile {
    pub fn metadata_len(&self) -> usize {
        self.text.split('\n').count().max(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighlightStats {
    pub total_lines: usize,
    pub highlighted_lines: usize,
    pub counts: Vec<(LineColour, usize)>,
}
