use crate::theme::{Theme, ThemeId};
use notepad_core::editor::EditorBuffer;
use notepad_core::{
    AppInfo, ColorOrder, FileManager, FindMatch, FindOptions, HighlightStats, LineColour,
    LineEnding, LineMetadata, ListType, LoadedFile, Note, NoteData, NotesStore, Result,
    SessionData, SessionStore, SessionTab, Settings, SettingsStore, TextEncoding, WindowState,
};
use rfd::FileDialog;
use serde_json::Value;
use std::fs;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommand {
    Minimize,
    ToggleMaximize,
    ConfirmClose,
    Close,
}

#[derive(Debug, Clone)]
pub struct DocumentState {
    pub path: Option<PathBuf>,
    pub title: String,
    pub buffer: EditorBuffer,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    pub had_bom: bool,
    pub dirty: bool,
}

impl Default for DocumentState {
    fn default() -> Self {
        Self {
            path: None,
            title: "Untitled".into(),
            buffer: EditorBuffer::default(),
            encoding: TextEncoding::Utf8,
            line_ending: LineEnding::Lf,
            had_bom: false,
            dirty: false,
        }
    }
}

impl DocumentState {
    fn loaded(file: LoadedFile) -> Self {
        let title = file
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled")
            .into();
        Self {
            path: Some(file.path),
            title,
            buffer: EditorBuffer::new(file.text),
            encoding: file.encoding,
            line_ending: file.line_ending,
            had_bom: file.had_bom,
            dirty: false,
        }
    }

    fn from_note(note: Note) -> Self {
        let mut metadata = note.list_structure;
        if metadata.is_empty() {
            metadata = note
                .highlights
                .into_iter()
                .map(|colour| LineMetadata {
                    colour,
                    ..LineMetadata::default()
                })
                .collect();
        } else {
            for (line, colour) in note.highlights.into_iter().enumerate() {
                if let Some(metadata) = metadata.get_mut(line) {
                    metadata.colour = colour;
                }
            }
        }
        let mut buffer = EditorBuffer::from_parts(note.content, metadata, 0);
        buffer.recognise_list_prefixes(4);
        Self {
            path: None,
            title: if note.title.is_empty() {
                "Untitled".into()
            } else {
                note.title
            },
            buffer,
            encoding: TextEncoding::Utf8,
            line_ending: LineEnding::Lf,
            had_bom: false,
            dirty: false,
        }
    }

    fn session(&self) -> SessionTab {
        SessionTab {
            path: self.path.clone(),
            title: self.title.clone(),
            text: self.buffer.text().into(),
            cursor: self.buffer.cursor(),
            metadata: self.buffer.metadata().to_vec(),
            encoding: self.encoding,
            line_ending: self.line_ending,
            had_bom: self.had_bom,
            modified: self.dirty,
        }
    }

    fn from_session(tab: SessionTab) -> Self {
        Self {
            path: tab.path,
            title: tab.title,
            buffer: EditorBuffer::from_parts(tab.text, tab.metadata, tab.cursor),
            encoding: tab.encoding,
            line_ending: tab.line_ending,
            had_bom: tab.had_bom,
            dirty: tab.modified,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub tabs: Vec<DocumentState>,
    pub active_tab: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            tabs: vec![DocumentState::default()],
            active_tab: 0,
        }
    }
}

impl AppState {
    fn active(&self) -> Option<&DocumentState> {
        self.tabs.get(self.active_tab)
    }

    fn active_mut(&mut self) -> Option<&mut DocumentState> {
        self.tabs.get_mut(self.active_tab)
    }

    fn dirty(&self) -> bool {
        self.tabs.iter().any(|tab| tab.dirty)
    }
}

#[derive(Debug, Clone)]
pub struct EditorSnapshot {
    pub title: String,
    pub path: Option<PathBuf>,
    pub text: String,
    pub metadata: Vec<LineMetadata>,
    pub cursor: usize,
    pub scroll_line: usize,
    pub selection: notepad_core::Selection,
    pub dirty: bool,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
}

pub struct AppController {
    data_dir: PathBuf,
    settings_store: SettingsStore,
    session_store: SessionStore,
    notes: NotesStore,
    settings: RwLock<Settings>,
    word_wrap: RwLock<bool>,
    recent: Mutex<Vec<PathBuf>>,
    startup: Mutex<Vec<PathBuf>>,
    state: Mutex<AppState>,
    note_ids: Mutex<Vec<Option<i64>>>,
    note_fingerprints: Mutex<Vec<Option<u64>>>,
    window: RwLock<WindowState>,
    command: Mutex<Option<WindowCommand>>,
}

impl AppController {
    pub fn new() -> Result<Self> {
        let data_dir = std::env::var_os("NOTEPAD_PRO_DATA_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                directories::ProjectDirs::from("com", "NotePadPro", "NotePad Pro")
                    .map(|paths| paths.data_dir().to_path_buf())
            })
            .unwrap_or_else(|| std::env::temp_dir().join("notepad-pro"));
        Self::with_data_dir(data_dir)
    }

    pub fn with_data_dir(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        let settings_store = SettingsStore::new(dir.join("settings.json"));
        let session_store = SessionStore::new(dir.join("session.json"));
        let settings = settings_store.load().unwrap_or_default();
        let recent = fs::read(dir.join("recent.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let word_wrap = fs::read(dir.join("word-wrap.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(false);
        let window = session_store
            .load()
            .map(|session| session.window)
            .unwrap_or_default();
        Ok(Self {
            data_dir: dir.clone(),
            settings_store,
            session_store,
            notes: NotesStore::open(dir.join("notes.sqlite"))?,
            settings: RwLock::new(settings),
            word_wrap: RwLock::new(word_wrap),
            recent: Mutex::new(recent),
            startup: Mutex::new(Vec::new()),
            state: Mutex::new(AppState::default()),
            note_ids: Mutex::new(vec![None]),
            note_fingerprints: Mutex::new(vec![None]),
            window: RwLock::new(window),
            command: Mutex::new(None),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn ping(&self) -> String {
        "pong".into()
    }

    pub fn app_info(&self) -> AppInfo {
        AppInfo::default()
    }

    pub fn get_settings(&self) -> Settings {
        self.settings
            .read()
            .map(|settings| settings.clone())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: Settings) -> Result<()> {
        self.settings_store.save(&settings)?;
        if let Ok(mut current) = self.settings.write() {
            *current = settings.clone();
        }
        self.trim_recent(settings.recent_files_limit);
        Ok(())
    }

    pub fn update_settings(&self, key: &str, value: Value) -> Result<Settings> {
        let settings = self.settings_store.update(key, value)?;
        if let Ok(mut current) = self.settings.write() {
            *current = settings.clone();
        }
        Ok(settings)
    }

    pub fn theme(&self) -> Theme {
        Theme::from_name(&self.get_settings().theme)
    }

    pub fn set_theme(&self, theme: ThemeId) -> Settings {
        let mut settings = self.get_settings();
        settings.theme = theme.name().into();
        let _ = self.save_settings(settings.clone());
        settings
    }

    pub fn cycle_theme(&self) -> Settings {
        let current = self.theme().id;
        let index = ThemeId::ALL.iter().position(|theme| *theme == current).unwrap_or(0);
        self.set_theme(ThemeId::ALL[(index + 1) % ThemeId::ALL.len()])
    }

    pub fn adjust_font_size(&self, delta: f32) -> Settings {
        let mut settings = self.get_settings();
        settings.font_size = (settings.font_size + delta).clamp(8.0, 96.0);
        let _ = self.save_settings(settings.clone());
        settings
    }

    pub fn set_sidebar_open(&self, open: bool) -> Settings {
        let mut settings = self.get_settings();
        settings.sidebar_open = open;
        let _ = self.save_settings(settings.clone());
        settings
    }

    pub fn toggle_sidebar(&self) -> bool {
        self.set_sidebar_open(!self.get_settings().sidebar_open)
            .sidebar_open
    }

    pub fn set_show_line_numbers(&self, show: bool) -> Settings {
        let mut settings = self.get_settings();
        settings.show_line_numbers = show;
        let _ = self.save_settings(settings.clone());
        settings
    }

    pub fn word_wrap(&self) -> bool {
        self.word_wrap.read().map(|value| *value).unwrap_or(false)
    }

    pub fn toggle_word_wrap(&self) -> bool {
        let mut value = self.word_wrap.write().expect("word-wrap lock poisoned");
        *value = !*value;
        let _ = fs::write(
            self.data_dir.join("word-wrap.json"),
            serde_json::to_vec(&*value).unwrap_or_default(),
        );
        *value
    }

    pub fn get_recent_files(&self) -> Vec<String> {
        self.recent
            .lock()
            .map(|paths| {
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn clear_recent_files(&self) {
        if let Ok(mut recent) = self.recent.lock() {
            recent.clear();
        }
        let _ = fs::remove_file(self.data_dir.join("recent.json"));
    }

    pub fn save_session(&self, mut session: SessionData) -> Result<()> {
        if let Ok(state) = self.state.lock() {
            session.tabs = state.tabs.iter().map(DocumentState::session).collect();
            session.active_tab = state.active_tab;
        }
        session.window = self.window_state();
        self.session_store.save(&session)
    }

    pub fn load_session(&self) -> Result<SessionData> {
        self.session_store.load()
    }

    pub fn restore_session(&self, session: SessionData) {
        let tabs = if session.tabs.is_empty() {
            vec![DocumentState::default()]
        } else {
            session
                .tabs
                .into_iter()
                .map(DocumentState::from_session)
                .collect()
        };
        if let Ok(mut state) = self.state.lock() {
            state.tabs = tabs;
            state.active_tab = session.active_tab.min(state.tabs.len().saturating_sub(1));
        }
        if let Ok(mut ids) = self.note_ids.lock() {
            *ids = vec![None; self.tab_count()];
        }
        if let Ok(mut fingerprints) = self.note_fingerprints.lock() {
            *fingerprints = vec![None; self.tab_count()];
        }
        self.set_window_state(session.window);
    }

    pub fn open_file_dialog(&self) -> Result<Vec<PathBuf>> {
        Ok(FileDialog::new()
            .set_title("Open files")
            .pick_files()
            .unwrap_or_default())
    }

    pub fn load_file(&self, path: impl AsRef<Path>) -> Result<LoadedFile> {
        FileManager::load_file(path)
    }

    pub fn save_file(
        &self,
        path: impl AsRef<Path>,
        text: &str,
        encoding: TextEncoding,
        line_ending: LineEnding,
    ) -> Result<()> {
        FileManager::save_file(path, text, encoding, line_ending)
    }

    pub fn save_file_as(&self, text: &str, name: &str) -> Result<Option<PathBuf>> {
        let path = FileDialog::new()
            .set_title("Save note")
            .set_file_name(name)
            .save_file();
        if let Some(path) = path {
            self.save_file(&path, text, TextEncoding::Utf8, LineEnding::Lf)?;
            Ok(Some(path))
        } else {
            Ok(None)
        }
    }

    pub fn file_exists(&self, path: impl AsRef<Path>) -> bool {
        path.as_ref().is_file()
    }

    pub fn save_extracted_text(&self, text: &str, name: &str) -> Result<Option<PathBuf>> {
        let path = FileDialog::new()
            .set_title("Export extracted text")
            .set_file_name(name)
            .save_file();
        if let Some(path) = path {
            FileManager::save_extracted_text(&path, text)?;
            Ok(Some(path))
        } else {
            Ok(None)
        }
    }

    pub fn set_startup_files(&self, paths: Vec<String>) {
        if let Ok(mut startup) = self.startup.lock() {
            *startup = paths.into_iter().map(PathBuf::from).collect();
        }
    }

    pub fn get_startup_files(&self) -> Vec<String> {
        self.startup
            .lock()
            .map(|mut startup| {
                startup
                    .drain(..)
                    .map(|path| path.display().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_notes_list(&self, query: Option<&str>) -> Result<Vec<Note>> {
        self.notes.get_notes_list(query)
    }

    pub fn get_note(&self, id: i64) -> Result<NoteData> {
        self.notes
            .get_note(id)?
            .map(NoteData::from)
            .ok_or_else(|| notepad_core::CoreError::InvalidValue(format!("note {id} does not exist")))
    }

    pub fn save_note(&self, note: NoteData) -> Result<i64> {
        self.notes.save_note(&note)
    }

    pub fn delete_note(&self, id: i64) -> Result<()> {
        self.notes.delete_note(id)?;
        if let Ok(mut ids) = self.note_ids.lock() {
            for value in ids.iter_mut() {
                if *value == Some(id) {
                    *value = None;
                }
            }
        }
        if let Ok(mut fingerprints) = self.note_fingerprints.lock() {
            for value in fingerprints.iter_mut() {
                *value = None;
            }
        }
        Ok(())
    }

    pub fn toggle_pin(&self, id: i64) -> Result<bool> {
        self.notes.toggle_pin(id)
    }

    pub fn highlight_stats(&self) -> HighlightStats {
        self.state
            .lock()
            .ok()
            .and_then(|state| {
                state
                    .active()
                    .map(|document| notepad_core::highlight_stats(document.buffer.text(), document.buffer.metadata()))
            })
            .unwrap_or_else(|| notepad_core::highlight_stats("", &[]))
    }

    pub fn extract_by_colour(&self, colours: Vec<LineColour>, order: ColorOrder) -> String {
        self.state
            .lock()
            .ok()
            .and_then(|state| {
                state.active().map(|document| {
                    notepad_core::extract_by_colour(
                        document.buffer.text(),
                        document.buffer.metadata(),
                        &colours,
                        order,
                    )
                })
            })
            .unwrap_or_default()
    }

    pub fn copy_to_clipboard(&self, text: String) -> Result<()> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|error| notepad_core::CoreError::InvalidValue(error.to_string()))?;
        clipboard
            .set_text(text)
            .map_err(|error| notepad_core::CoreError::InvalidValue(error.to_string()))
    }

    pub fn paste_from_clipboard(&self) -> Result<Option<String>> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|error| notepad_core::CoreError::InvalidValue(error.to_string()))?;
        Ok(clipboard.get_text().ok())
    }

    pub fn window_state(&self) -> WindowState {
        self.window
            .read()
            .map(|window| *window)
            .unwrap_or_default()
    }

    pub fn set_window_state(&self, window: WindowState) {
        if let Ok(mut current) = self.window.write() {
            *current = window;
        }
    }

    pub fn minimise(&self) {
        self.set_command(WindowCommand::Minimize);
    }

    pub fn toggle_maximise(&self) {
        self.set_command(WindowCommand::ToggleMaximize);
    }

    pub fn confirm_close(&self) {
        self.set_command(WindowCommand::Close);
    }

    pub fn close_window(&self) -> bool {
        let dirty = self.state.lock().map(|state| state.dirty()).unwrap_or(false);
        if dirty {
            self.set_command(WindowCommand::ConfirmClose);
            false
        } else {
            self.set_command(WindowCommand::Close);
            true
        }
    }

    pub fn take_window_command(&self) -> Option<WindowCommand> {
        self.command.lock().ok()?.take()
    }

    pub fn active_snapshot(&self) -> Option<EditorSnapshot> {
        self.state.lock().ok()?.active().map(|document| EditorSnapshot {
            title: document.title.clone(),
            path: document.path.clone(),
            text: document.buffer.text().into(),
            metadata: document.buffer.metadata().to_vec(),
            cursor: document.buffer.cursor(),
            scroll_line: 0,
            selection: document.buffer.selection(),
            dirty: document.dirty,
            encoding: document.encoding,
            line_ending: document.line_ending,
        })
    }

    pub fn tab_summaries(&self) -> Vec<(String, bool)> {
        self.state
            .lock()
            .map(|state| {
                state
                    .tabs
                    .iter()
                    .map(|document| (document.title.clone(), document.dirty))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn active_tab_index(&self) -> usize {
        self.state.lock().map(|state| state.active_tab).unwrap_or(0)
    }

    pub fn tab_count(&self) -> usize {
        self.state.lock().map(|state| state.tabs.len()).unwrap_or(0)
    }

    pub fn tab_is_dirty(&self, index: usize) -> bool {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.tabs.get(index).map(|tab| tab.dirty))
            .unwrap_or(false)
    }

    pub fn open_document(&self, path: impl AsRef<Path>) -> Result<()> {
        let file = self.load_file(path)?;
        self.remember(&file.path);
        let mut document = DocumentState::loaded(file);
        document
            .buffer
            .recognise_list_prefixes(self.get_settings().tab_width as usize);
        let mut replaced_empty = false;
        if let Ok(mut state) = self.state.lock() {
            let replace_empty = state.active().is_some_and(|current| {
                current.path.is_none() && current.buffer.text().is_empty() && !current.dirty
            });
            if replace_empty {
                *state.active_mut().expect("active document") = document;
                replaced_empty = true;
            } else {
                state.tabs.push(document);
                state.active_tab = state.tabs.len() - 1;
                if let Ok(mut ids) = self.note_ids.lock() {
                    ids.push(None);
                }
                if let Ok(mut fingerprints) = self.note_fingerprints.lock() {
                    fingerprints.push(None);
                }
            }
        }
        if replaced_empty {
            self.set_active_note_id(None);
            self.set_active_fingerprint(None);
        }
        Ok(())
    }

    pub fn open_note(&self, id: i64) -> Result<bool> {
        let note = self
            .notes
            .get_note(id)?
            .ok_or_else(|| notepad_core::CoreError::InvalidValue(format!("note {id} does not exist")))?;
        let document = DocumentState::from_note(note);
        let mut replaced_empty = false;
        if let Ok(mut state) = self.state.lock() {
            let replace_empty = state.active().is_some_and(|current| {
                current.path.is_none() && current.buffer.text().is_empty() && !current.dirty
            });
            if replace_empty {
                *state.active_mut().expect("active document") = document;
                replaced_empty = true;
            } else {
                state.tabs.push(document);
                state.active_tab = state.tabs.len() - 1;
                if let Ok(mut ids) = self.note_ids.lock() {
                    ids.push(Some(id));
                }
                if let Ok(mut fingerprints) = self.note_fingerprints.lock() {
                    fingerprints.push(None);
                }
            }
        } else {
            return Ok(false);
        }
        if replaced_empty {
            self.set_active_note_id(Some(id));
            self.set_active_fingerprint(None);
        }
        Ok(true)
    }

    pub fn new_tab(&self) {
        self.new_tab_with_text("");
    }

    pub fn new_tab_with_text(&self, text: &str) {
        if let Ok(mut state) = self.state.lock() {
            let mut document = DocumentState::default();
            if !text.is_empty() {
                document.buffer.insert_text(text);
                document.dirty = true;
                update_title(&mut document);
            }
            state.tabs.push(document);
            state.active_tab = state.tabs.len() - 1;
            if let Ok(mut ids) = self.note_ids.lock() {
                ids.push(None);
            }
            if let Ok(mut fingerprints) = self.note_fingerprints.lock() {
                fingerprints.push(None);
            }
        }
    }

    pub fn switch_tab(&self, index: usize) -> bool {
        if let Ok(mut state) = self.state.lock() {
            if index < state.tabs.len() {
                state.active_tab = index;
                return true;
            }
        }
        false
    }

    pub fn close_tab(&self, index: usize) -> bool {
        if self.tab_is_dirty(index) {
            return false;
        }
        self.remove_tab(index)
    }

    pub fn discard_tab(&self, index: usize) -> bool {
        self.remove_tab(index)
    }

    pub fn insert_text(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.buffer.insert_text(text);
                document.dirty = true;
                update_title(document);
            }
        }
    }

    pub fn replace_active_text(&self, text: &str) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                if document.buffer.text() != text {
                    let end = document.buffer.text().len();
                    document.buffer.replace_range(0..end, text);
                    document.dirty = true;
                    update_title(document);
                }
            }
        }
    }

    pub fn replace_selection(&self, text: &str) {
        self.insert_text(text);
    }

    pub fn selected_text(&self) -> Option<String> {
        let state = self.state.lock().ok()?;
        let document = state.active()?;
        let range = document.buffer.selection().range();
        document.buffer.text().get(range).map(str::to_owned)
    }

    pub fn set_active_selection(&self, anchor: usize, caret: usize) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.buffer.set_selection(anchor, caret);
            }
        }
    }

    pub fn select_all(&self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.buffer.select_all();
            }
        }
    }

    pub fn move_active_cursor(&self, direction: i32, vertical: bool, extend: bool) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                if vertical {
                    document.buffer.move_vertical(direction, extend);
                } else {
                    document.buffer.move_horizontal(direction, extend);
                }
            }
        }
    }

    pub fn move_active_line_start(&self, extend: bool) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.buffer.move_line_start(extend);
            }
        }
    }

    pub fn move_active_line_end(&self, extend: bool) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.buffer.move_line_end(extend);
            }
        }
    }

    pub fn find_active(&self, query: &str, options: &FindOptions) -> Result<Vec<FindMatch>> {
        let state = self
            .state
            .lock()
            .map_err(|_| notepad_core::CoreError::InvalidValue("state lock poisoned".into()))?;
        let Some(document) = state.active() else {
            return Ok(Vec::new());
        };
        notepad_core::find_all(document.buffer.text(), query, options)
    }

    pub fn select_find_match(&self, find: &FindMatch) {
        self.set_active_selection(find.start, find.end);
    }

    pub fn replace_match_active(&self, find: &FindMatch, replacement: &str) -> Result<bool> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| notepad_core::CoreError::InvalidValue("state lock poisoned".into()))?;
        let Some(document) = state.active_mut() else {
            return Ok(false);
        };
        if find.end > document.buffer.text().len() || find.start > find.end {
            return Ok(false);
        }
        document.buffer.replace_range(find.start..find.end, replacement);
        document.dirty = true;
        update_title(document);
        Ok(true)
    }

    pub fn apply_list_type(&self, list_type: ListType) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.buffer.set_list_type(list_type);
                document.dirty = true;
            }
        }
    }

    pub fn replace_all_active(
        &self,
        query: &str,
        replacement: &str,
        options: &FindOptions,
    ) -> Result<usize> {
        let (old_text, new_text, count) = {
            let state = self
                .state
                .lock()
                .map_err(|_| notepad_core::CoreError::InvalidValue("state lock poisoned".into()))?;
            let Some(document) = state.active() else {
                return Ok(0);
            };
            let (new_text, count) =
                notepad_core::replace_all(document.buffer.text(), query, replacement, options)?;
            (document.buffer.text().to_owned(), new_text, count)
        };
        if old_text == new_text {
            return Ok(0);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| notepad_core::CoreError::InvalidValue("state lock poisoned".into()))?;
        let Some(document) = state.active_mut() else {
            return Ok(0);
        };
        document.buffer.replace_range(0..old_text.len(), &new_text);
        document.dirty = true;
        update_title(document);
        Ok(count)
    }

    pub fn replace_first_active(
        &self,
        query: &str,
        replacement: &str,
        options: &FindOptions,
    ) -> Result<bool> {
        let (old_text, new_text, changed) = {
            let state = self
                .state
                .lock()
                .map_err(|_| notepad_core::CoreError::InvalidValue("state lock poisoned".into()))?;
            let Some(document) = state.active() else {
                return Ok(false);
            };
            let (new_text, changed) =
                notepad_core::replace_first(document.buffer.text(), query, replacement, options)?;
            (document.buffer.text().to_owned(), new_text, changed)
        };
        if !changed {
            return Ok(false);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| notepad_core::CoreError::InvalidValue("state lock poisoned".into()))?;
        let Some(document) = state.active_mut() else {
            return Ok(false);
        };
        document.buffer.replace_range(0..old_text.len(), &new_text);
        document.dirty = true;
        update_title(document);
        Ok(true)
    }

    pub fn delete_backward(&self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                let before = document.buffer.text().to_owned();
                document.buffer.delete_backward();
                if before != document.buffer.text() {
                    document.dirty = true;
                    update_title(document);
                }
            }
        }
    }

    pub fn delete_forward(&self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                let before = document.buffer.text().to_owned();
                document.buffer.delete_forward();
                if before != document.buffer.text() {
                    document.dirty = true;
                    update_title(document);
                }
            }
        }
    }

    pub fn set_active_cursor(&self, position: usize, extend: bool) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.buffer.set_cursor(position, extend);
            }
        }
    }

    pub fn undo(&self) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let Some(document) = state.active_mut() else {
            return false;
        };
        let changed = document.buffer.undo();
        if changed {
            document.dirty = true;
            update_title(document);
        }
        changed
    }

    pub fn redo(&self) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let Some(document) = state.active_mut() else {
            return false;
        };
        let changed = document.buffer.redo();
        if changed {
            document.dirty = true;
            update_title(document);
        }
        changed
    }

    pub fn apply_highlight(&self, colour: LineColour) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.buffer.apply_colour(colour);
                document.dirty = true;
            }
        }
    }

    pub fn apply_custom_highlight(&self, rgb: u32) {
        self.apply_highlight(LineColour::Custom(rgb & 0x00FF_FFFF));
    }

    pub fn handle_enter(&self) {
        let tab_width = self.get_settings().tab_width as usize;
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.buffer.handle_enter(tab_width);
                document.dirty = true;
                update_title(document);
            }
        }
    }

    pub fn handle_tab(&self, shift: bool) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.buffer.adjust_indent(!shift);
                document.dirty = true;
            }
        }
    }

    pub fn toggle_checkbox(&self, line: usize) -> Option<bool> {
        self.state.lock().ok()?.active_mut().and_then(|document| {
            let result = document.buffer.toggle_checkbox(line);
            if result.is_some() {
                document.dirty = true;
            }
            result
        })
    }

    pub fn autosave(&self) -> Result<bool> {
        if !self.get_settings().autosave {
            return Ok(false);
        }
        let original = self.active_tab_index();
        let mut saved_any = false;
        for index in 0..self.tab_count() {
            if !self.tab_is_dirty(index) {
                continue;
            }
            let fingerprint = self.document_fingerprint(index);
            if fingerprint.is_some() && self.note_fingerprint(index) == fingerprint {
                continue;
            }
            if !self.switch_tab(index) {
                continue;
            }
            if self.save_active_note().is_ok() {
                saved_any = true;
            }
        }
        if self.tab_count() > 0 {
            self.switch_tab(original.min(self.tab_count() - 1));
        }
        Ok(saved_any)
    }

    pub fn save_active_note(&self) -> Result<i64> {
        let (index, id, data) = {
            let state = self
                .state
                .lock()
                .map_err(|_| notepad_core::CoreError::InvalidValue("state lock poisoned".into()))?;
            let Some(document) = state.active() else {
                return Err(notepad_core::CoreError::InvalidValue("no active document".into()));
            };
            let index = state.active_tab;
            let id = self.note_id(index).unwrap_or(0);
            let highlights = document
                .buffer
                .metadata()
                .iter()
                .map(|metadata| metadata.colour)
                .collect();
            let data = NoteData {
                id,
                title: note_title(document),
                content: document.buffer.text().to_owned(),
                highlights,
                list_structure: document.buffer.metadata().to_vec(),
                pinned: false,
            };
            (index, id, data)
        };
        let pinned = if id > 0 {
            self.notes
                .get_note(id)?
                .map(|note| note.pinned)
                .unwrap_or(false)
        } else {
            false
        };
        let mut data = data;
        data.pinned = pinned;
        let saved_id = self.notes.save_note(&data)?;
        let fingerprint = self.document_fingerprint(index);
        if let Ok(mut ids) = self.note_ids.lock() {
            if let Some(slot) = ids.get_mut(index) {
                *slot = Some(saved_id);
            }
        }
        if let Ok(mut fingerprints) = self.note_fingerprints.lock() {
            if let Some(slot) = fingerprints.get_mut(index) {
                *slot = fingerprint;
            }
        }
        Ok(saved_id)
    }

    pub fn save_active(&self) -> Result<bool> {
        let (path, text, encoding, line_ending, had_bom) = {
            let state = self
                .state
                .lock()
                .map_err(|_| notepad_core::CoreError::InvalidValue("state lock poisoned".into()))?;
            let Some(document) = state.active() else {
                return Ok(false);
            };
            let Some(path) = document.path.clone() else {
                return Ok(false);
            };
            (
                path,
                document.buffer.text().to_owned(),
                document.encoding,
                document.line_ending,
                document.had_bom,
            )
        };
        FileManager::save_file_with_bom(&path, &text, encoding, line_ending, had_bom)?;
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.dirty = false;
            }
        }
        self.remember(&path);
        Ok(true)
    }

    pub fn save_active_as(&self) -> Result<Option<PathBuf>> {
        let (text, name) = {
            let state = self
                .state
                .lock()
                .map_err(|_| notepad_core::CoreError::InvalidValue("state lock poisoned".into()))?;
            let Some(document) = state.active() else {
                return Ok(None);
            };
            (document.buffer.text().to_owned(), document.title.clone())
        };
        let Some(path) = self.save_file_as(&text, &name)? else {
            return Ok(None);
        };
        self.remember(&path);
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.active_mut() {
                document.path = Some(path.clone());
                document.title = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Untitled")
                    .into();
                document.dirty = false;
            }
        }
        Ok(Some(path))
    }

    pub fn save_tab(&self, index: usize) -> Result<bool> {
        let (path, text, encoding, line_ending, had_bom) = {
            let state = self
                .state
                .lock()
                .map_err(|_| notepad_core::CoreError::InvalidValue("state lock poisoned".into()))?;
            let Some(document) = state.tabs.get(index) else {
                return Ok(false);
            };
            let Some(path) = document.path.clone() else {
                return Ok(false);
            };
            (
                path,
                document.buffer.text().to_owned(),
                document.encoding,
                document.line_ending,
                document.had_bom,
            )
        };
        FileManager::save_file_with_bom(&path, &text, encoding, line_ending, had_bom)?;
        if let Ok(mut state) = self.state.lock() {
            if let Some(document) = state.tabs.get_mut(index) {
                document.dirty = false;
            }
        }
        self.remember(&path);
        Ok(true)
    }

    fn document_fingerprint(&self, index: usize) -> Option<u64> {
        self.state.lock().ok().and_then(|state| {
            state.tabs.get(index).map(|document| {
                fingerprint(document.buffer.text(), document.buffer.metadata())
            })
        })
    }

    fn note_fingerprint(&self, index: usize) -> Option<u64> {
        self.note_fingerprints
            .lock()
            .ok()
            .and_then(|fingerprints| fingerprints.get(index).copied().flatten())
    }

    fn set_active_fingerprint(&self, fingerprint: Option<u64>) {
        let index = self.active_tab_index();
        if let Ok(mut fingerprints) = self.note_fingerprints.lock() {
            if fingerprints.len() <= index {
                fingerprints.resize(index + 1, None);
            }
            fingerprints[index] = fingerprint;
        }
    }

    fn note_id(&self, index: usize) -> Option<i64> {
        self.note_ids
            .lock()
            .ok()
            .and_then(|ids| ids.get(index).copied().flatten())
    }

    fn set_active_note_id(&self, id: Option<i64>) {
        let index = self.active_tab_index();
        if let Ok(mut ids) = self.note_ids.lock() {
            if ids.len() <= index {
                ids.resize(index + 1, None);
            }
            ids[index] = id;
        }
    }

    fn remove_tab(&self, index: usize) -> bool {
        if let Ok(mut state) = self.state.lock() {
            if index >= state.tabs.len() {
                return false;
            }
            state.tabs.remove(index);
            if state.tabs.is_empty() {
                state.tabs.push(DocumentState::default());
            }
            if state.active_tab > index {
                state.active_tab -= 1;
            } else if state.active_tab >= state.tabs.len() {
                state.active_tab = state.tabs.len() - 1;
            }
            if let Ok(mut ids) = self.note_ids.lock() {
                if index < ids.len() {
                    ids.remove(index);
                }
                if ids.is_empty() {
                    ids.push(None);
                }
            }
            if let Ok(mut fingerprints) = self.note_fingerprints.lock() {
                if index < fingerprints.len() {
                    fingerprints.remove(index);
                }
                if fingerprints.is_empty() {
                    fingerprints.push(None);
                }
            }
            true
        } else {
            false
        }
    }

    fn set_command(&self, command: WindowCommand) {
        if let Ok(mut current) = self.command.lock() {
            *current = Some(command);
        }
    }

    fn remember(&self, path: &Path) {
        if let Ok(mut recent) = self.recent.lock() {
            recent.retain(|item| item != path);
            recent.insert(0, path.into());
            recent.truncate(self.get_settings().recent_files_limit);
            let _ = fs::write(
                self.data_dir.join("recent.json"),
                serde_json::to_vec_pretty(&*recent).unwrap_or_default(),
            );
        }
    }

    fn trim_recent(&self, limit: usize) {
        if let Ok(mut recent) = self.recent.lock() {
            recent.truncate(limit);
            let _ = fs::write(
                self.data_dir.join("recent.json"),
                serde_json::to_vec_pretty(&*recent).unwrap_or_default(),
            );
        }
    }
}

fn update_title(document: &mut DocumentState) {
    if document.path.is_some() {
        return;
    }
    let title = document
        .buffer
        .text()
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Untitled");
    document.title = title.chars().take(32).collect();
}

fn fingerprint(text: &str, metadata: &[LineMetadata]) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    serde_json::to_vec(metadata)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

fn note_title(document: &DocumentState) -> String {
    if document.title == "Untitled" {
        document
            .buffer
            .text()
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("Untitled")
            .chars()
            .take(80)
            .collect()
    } else {
        document.title.clone()
    }
}
