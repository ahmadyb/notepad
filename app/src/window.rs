use crate::controller::{AppController, EditorSnapshot, WindowCommand};
use crate::layout::{hit_test, HitTarget, Layout, Rect};
use crate::renderer::ChromeRenderer;
use crate::scintilla::ScintillaHost;
use crate::ui::extract_panel::ExtractPanelState;
use crate::ui::find_bar::{FindBarState, FindField};
use crate::ui::sidebar::{NoteAction, SidebarState};
use crate::ui::toolbar::ToolbarAction;
use notepad_core::{ColorOrder, LineColour, ListType};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
#[cfg(windows)]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(windows)]
use std::ffi::c_void;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Icon, Window, WindowId};

pub fn run(controller: Arc<AppController>) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|error| error.to_string())?;
    let mut application = NativeApplication::new(controller);
    event_loop
        .run_app(&mut application)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputFocus {
    Editor,
    Find,
    Replacement,
    Sidebar,
}

struct NativeApplication {
    controller: Arc<AppController>,
    window: Option<Arc<Window>>,
    _context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    scintilla: Option<ScintillaHost>,
    #[cfg(windows)]
    native_text: String,
    #[cfg(windows)]
    native_tab: usize,
    #[cfg(windows)]
    native_selection: (usize, usize),
    renderer: ChromeRenderer,
    find: FindBarState,
    sidebar: SidebarState,
    extract: ExtractPanelState,
    focus: InputFocus,
    modifiers: ModifiersState,
    pointer: (i32, i32),
    mouse_down: bool,
    drag_anchor: usize,
    scroll_line: usize,
    last_redraw: Instant,
    last_autosave: Instant,
}

impl NativeApplication {
    fn new(controller: Arc<AppController>) -> Self {
        Self {
            controller,
            window: None,
            _context: None,
            surface: None,
            scintilla: None,
            #[cfg(windows)]
            native_text: String::new(),
            #[cfg(windows)]
            native_tab: usize::MAX,
            #[cfg(windows)]
            native_selection: (0, 0),
            renderer: ChromeRenderer::new(),
            find: FindBarState::default(),
            sidebar: SidebarState::default(),
            extract: ExtractPanelState::default(),
            focus: InputFocus::Editor,
            modifiers: ModifiersState::empty(),
            pointer: (0, 0),
            mouse_down: false,
            drag_anchor: 0,
            scroll_line: 0,
            last_redraw: Instant::now(),
            last_autosave: Instant::now(),
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        if self.window.is_some() {
            return Ok(());
        }
        let state = self.controller.window_state();
        let icon = Icon::from_rgba(app_icon_pixels(), 32, 32).ok();
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_window_icon(icon)
                        .with_title("NotePad Pro")
                        .with_decorations(false)
                        .with_resizable(true)
                        .with_min_inner_size(LogicalSize::new(680_u32, 420_u32))
                        .with_inner_size(LogicalSize::new(state.width, state.height)),
                )
                .map_err(|error| error.to_string())?,
        );
        if state.maximized {
            window.set_maximized(true);
        }
        window.set_ime_allowed(true);
        let context = softbuffer::Context::new(window.clone())
            .map_err(|error| format!("softbuffer context: {error}"))?;
        let settings = self.controller.get_settings();
        let layout = Layout::compute_with_options(
            state.width,
            state.height,
            settings.sidebar_open,
            false,
            false,
            false,
        );
        let scintilla = attach_native_editor(&window, layout);
        let surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|error| format!("softbuffer surface: {error}"))?;
        self.window = Some(window);
        self._context = Some(context);
        self.surface = Some(surface);
        self.scintilla = scintilla;
        Ok(())
    }

    fn current_layout(&self, width: u32, height: u32) -> Layout {
        let settings = self.controller.get_settings();
        Layout::compute_with_options(
            width,
            height,
            settings.sidebar_open,
            self.find.open,
            self.find.open && self.find.show_replace,
            self.extract.open,
        )
    }

    fn refresh_sidebar(&mut self) {
        self.sidebar.set_notes(
            self.controller
                .get_notes_list((!self.sidebar.query.trim().is_empty()).then_some(self.sidebar.query.as_str()))
                .unwrap_or_default(),
        );
    }

    fn refresh_find(&mut self, select_first: bool) {
        if let Some(snapshot) = self.controller.active_snapshot() {
            let _ = self.find.refresh(&snapshot.text);
            if select_first {
                if let Some(found) = self.find.current_match().cloned() {
                    self.controller.select_find_match(&found);
                }
            }
        }
    }

    fn refresh_extract(&mut self) {
        let stats = self.controller.highlight_stats();
        self.extract.set_available(&stats.counts);
        self.extract.preview = self.controller.extract_by_colour(
            self.extract.selected_colours.clone(),
            self.extract.order,
        );
    }

    fn render_frame(&mut self) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if let Some(surface) = self.surface.as_mut() {
            resize_surface(surface, size.width, size.height);
        } else {
            return;
        }
        let settings = self.controller.get_settings();
        self.renderer.set_font_size(settings.font_size);
        let layout = self.current_layout(size.width, size.height);
        self.extract.bounds = layout.extract_panel;
        self.refresh_sidebar();
        self.refresh_extract();
        self.sync_native_editor(layout);
        let mut snapshot = self.controller.active_snapshot();
        if let Some(snapshot) = snapshot.as_mut() {
            snapshot.scroll_line = self.scroll_line;
        }
        let tabs = self.controller.tab_summaries();
        let Some(pixmap) = self.renderer.render(
            size.width,
            size.height,
            layout,
            self.controller.theme(),
            snapshot.as_ref(),
            &tabs,
            &self.sidebar.notes,
            &self.find,
            &self.extract,
            &self.sidebar.query,
            self.focus == InputFocus::Sidebar,
            if matches!(self.sidebar.sort, crate::ui::sidebar::NoteSort::Modified) {
                "Recent"
            } else {
                "A–Z"
            },
            settings.show_line_numbers,
            self.controller.word_wrap(),
            self.controller.active_tab_index(),
            self.pointer,
        ) else {
            return;
        };
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };
        #[allow(clippy::chunks_exact_to_as_chunks)]
        for (pixel, rgba) in buffer.iter_mut().zip(pixmap.data().chunks_exact(4)) {
            *pixel = (u32::from(rgba[0]) << 16)
                | (u32::from(rgba[1]) << 8)
                | u32::from(rgba[2]);
        }
        let _ = buffer.present();
        self.last_redraw = Instant::now();
    }

    fn resize(&mut self, width: u32, height: u32) {
        if let Some(surface) = self.surface.as_mut() {
            resize_surface(surface, width, height);
        }
    }

    fn focus_input(&mut self, focus: InputFocus) {
        self.focus = focus;
        match focus {
            InputFocus::Find => {
                self.find.focus = FindField::Query;
            }
            InputFocus::Replacement => {
                self.find.focus = FindField::Replacement;
            }
            InputFocus::Editor | InputFocus::Sidebar => {}
        }
        #[cfg(windows)]
        if let Some(host) = self.scintilla.as_ref() {
            if matches!(focus, InputFocus::Editor) {
                host.focus();
            } else {
                host.focus_parent();
            }
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn handle_mouse(&mut self, event_loop: &ActiveEventLoop, button: MouseButton) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        if button != MouseButton::Left {
            return;
        }
        let size = window.inner_size();
        let layout = self.current_layout(size.width, size.height);
        let x = self.pointer.0;
        let y = self.pointer.1;
        match hit_test(layout, x, y) {
            HitTarget::Minimize => self.controller.minimise(),
            HitTarget::Maximize => self.controller.toggle_maximise(),
            HitTarget::Close => {
                self.controller.close_window();
            }
            HitTarget::Drag => {
                let _ = window.drag_window();
            }
            HitTarget::Editor => self.handle_editor_click(layout, x, y),
            HitTarget::TabBar => self.handle_tab_click(layout, x, y),
            HitTarget::Toolbar => self.handle_toolbar_click(layout, x, y),
            HitTarget::FindBar => self.handle_find_click(layout, x, y),
            HitTarget::Sidebar => self.handle_sidebar_click(layout, x, y),
            HitTarget::ExtractPanel => self.handle_extract_click(layout, x, y),
            HitTarget::None => self.focus_input(InputFocus::Editor),
        }
        self.apply_command(event_loop);
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn handle_editor_click(&mut self, layout: Layout, x: i32, y: i32) {
        self.focus_input(InputFocus::Editor);
        let Some(snapshot) = self.controller.active_snapshot() else {
            return;
        };
        let settings = self.controller.get_settings();
        let line_height = (settings.font_size * 1.55).ceil() as i32;
        let line = ((y - layout.editor.y - 8).max(0) / line_height.max(1)) as usize;
        let margin = Layout::line_number_width(
            snapshot.text.split('\n').count(),
            (settings.font_size * 0.62) as i32,
        );
        let in_marker = x < layout.editor.x + margin;
        if in_marker
            && snapshot
                .metadata
                .get(line)
                .is_some_and(|metadata| metadata.list_type == ListType::Check)
        {
            self.controller.toggle_checkbox(line);
        } else {
            let cursor = hit_cursor(&snapshot, layout, x, y, settings.font_size, self.scroll_line);
            self.controller.set_active_cursor(cursor, false);
            self.drag_anchor = cursor;
            self.mouse_down = true;
        }
        #[cfg(windows)]
        if let Some(host) = self.scintilla.as_ref() {
            host.focus();
        }
    }

    fn handle_tab_click(&mut self, layout: Layout, x: i32, y: i32) {
        if layout.tab_plus_rect().contains(x, y) {
            self.controller.new_tab();
            self.focus_input(InputFocus::Editor);
            self.reset_native_tab();
            return;
        }
        let tabs = self.controller.tab_summaries();
        for (index, tab) in crate::ui::tab_bar::layout_tabs(layout, &tabs).iter().enumerate() {
            if !tab.rect.contains(x, y) {
                continue;
            }
            let close = Rect::new(tab.rect.right() - 31, tab.rect.y + 2, 27, tab.rect.h - 4);
            if close.contains(x, y) {
                self.request_close_tab(index);
            } else {
                self.controller.switch_tab(index);
                self.reset_native_tab();
                self.refresh_find(false);
            }
            return;
        }
    }

    fn request_close_tab(&mut self, index: usize) {
        if !self.controller.tab_is_dirty(index) {
            self.controller.close_tab(index);
            self.reset_native_tab();
            return;
        }
        let result = MessageDialog::new()
            .set_title("Unsaved changes")
            .set_description("Save changes in this tab before closing?")
            .set_buttons(MessageButtons::YesNoCancel)
            .show();
        match result {
            MessageDialogResult::Yes => {
                let current = self.controller.active_tab_index();
                let mut saved = false;
                if index == current {
                    saved = self.controller.save_active().unwrap_or(false);
                    if !saved {
                        saved = self.controller.save_active_as().ok().flatten().is_some();
                    }
                } else if self.controller.switch_tab(index) {
                    saved = self.controller.save_active().unwrap_or(false);
                    if !saved {
                        saved = self.controller.save_active_as().ok().flatten().is_some();
                    }
                    if current < self.controller.tab_count() {
                        self.controller.switch_tab(current);
                    }
                }
                if saved {
                    self.controller.close_tab(index);
                }
            }
            MessageDialogResult::No => {
                self.controller.discard_tab(index);
            }
            MessageDialogResult::Cancel
            | MessageDialogResult::Ok
            | MessageDialogResult::Custom(_) => {}
        }
        self.reset_native_tab();
    }

    fn handle_toolbar_click(&mut self, layout: Layout, x: i32, y: i32) {
        for (button, action) in crate::ui::toolbar::buttons(layout) {
            if !button.contains(x, y) {
                continue;
            }
            match action {
                ToolbarAction::New => {
                    self.controller.new_tab();
                    self.reset_native_tab();
                }
                ToolbarAction::Open => {
                    if let Ok(paths) = self.controller.open_file_dialog() {
                        for path in paths {
                            let _ = self.controller.open_document(path);
                        }
                        self.reset_native_tab();
                    }
                }
                ToolbarAction::Save => self.save_current(),
                ToolbarAction::SaveAs => {
                    let _ = self.controller.save_active_as();
                }
                ToolbarAction::Undo => {
                    self.controller.undo();
                    self.reset_native_tab();
                }
                ToolbarAction::Redo => {
                    self.controller.redo();
                    self.reset_native_tab();
                }
                ToolbarAction::Find => {
                    self.find.open = true;
                    self.find.show_replace = false;
                    self.focus_input(InputFocus::Find);
                    self.refresh_find(true);
                }
                ToolbarAction::Replace => {
                    self.find.open = true;
                    self.find.show_replace = true;
                    self.focus_input(InputFocus::Find);
                    self.refresh_find(true);
                }
                ToolbarAction::Extract => {
                    self.extract.open = !self.extract.open;
                    if self.extract.open {
                        self.refresh_extract();
                    }
                }
                ToolbarAction::Notes => {
                    self.controller.toggle_sidebar();
                }
                ToolbarAction::Wrap => {
                    self.controller.toggle_word_wrap();
                }
                ToolbarAction::ZoomOut => {
                    self.controller.adjust_font_size(-1.0);
                }
                ToolbarAction::ZoomIn => {
                    self.controller.adjust_font_size(1.0);
                }
                ToolbarAction::Theme => {
                    self.controller.cycle_theme();
                }
                ToolbarAction::BulletList => self.controller.apply_list_type(ListType::Bullet),
                ToolbarAction::NumberedList => self.controller.apply_list_type(ListType::Number),
                ToolbarAction::Checklist => self.controller.apply_list_type(ListType::Check),
                ToolbarAction::Outdent => self.controller.handle_tab(true),
                ToolbarAction::Indent => self.controller.handle_tab(false),
                ToolbarAction::Highlight(colour) => self.controller.apply_highlight(colour),
                ToolbarAction::ClearHighlight => self.controller.apply_highlight(LineColour::None),
            }
            break;
        }
    }

    fn save_current(&mut self) {
        if !self.controller.save_active().unwrap_or(false) {
            let _ = self.controller.save_active_as();
        }
    }

    fn handle_find_click(&mut self, layout: Layout, x: i32, y: i32) {
        let first_y = layout.find_bar.y + 8;
        let query = Rect::new(layout.find_bar.x + 14, first_y, 300, 28);
        let previous = Rect::new(query.right() + 112, first_y, 30, 28);
        let next = Rect::new(previous.right() + 4, first_y, 30, 28);
        let toggle_replace = Rect::new(next.right() + 10, first_y, 92, 28);
        let close = Rect::new(layout.find_bar.right() - 40, first_y, 28, 28);
        if query.contains(x, y) {
            self.focus_input(InputFocus::Find);
        } else if previous.contains(x, y) {
            if let Some(found) = self.find.previous_match().cloned() {
                self.controller.select_find_match(&found);
            }
        } else if next.contains(x, y) {
            if let Some(found) = self.find.next_match().cloned() {
                self.controller.select_find_match(&found);
            }
        } else if toggle_replace.contains(x, y) {
            self.find.show_replace = !self.find.show_replace;
            if self.find.show_replace {
                self.find.open = true;
            }
        } else if close.contains(x, y) {
            self.find.open = false;
            self.focus_input(InputFocus::Editor);
        } else {
            let check_y = if self.find.show_replace {
                layout.find_bar.y + 51
            } else {
                layout.find_bar.y + 12
            };
            let case_check = Rect::new(layout.find_bar.right() - 230, check_y, 20, 20);
            let regex_check = Rect::new(layout.find_bar.right() - 150, check_y, 20, 20);
            let whole_check = Rect::new(layout.find_bar.right() - 95, check_y, 20, 20);
            if case_check.contains(x, y) {
                self.find.options.case_sensitive = !self.find.options.case_sensitive;
                self.refresh_find(false);
            } else if regex_check.contains(x, y) {
                self.find.options.regex = !self.find.options.regex;
                self.refresh_find(false);
            } else if whole_check.contains(x, y) {
                self.find.options.whole_word = !self.find.options.whole_word;
                self.refresh_find(false);
            } else if self.find.show_replace {
                let replacement = Rect::new(layout.find_bar.x + 14, layout.find_bar.y + 48, 300, 28);
                let replace = Rect::new(replacement.right() + 14, replacement.y, 74, 28);
                let replace_all = Rect::new(replace.right() + 6, replacement.y, 88, 28);
                if replacement.contains(x, y) {
                    self.focus_input(InputFocus::Replacement);
                } else if replace.contains(x, y) {
                    self.replace_current();
                } else if replace_all.contains(x, y) {
                    let _ = self.controller.replace_all_active(
                        &self.find.query,
                        &self.find.replacement,
                        &self.find.options,
                    );
                    self.refresh_find(true);
                }
            }
        }
    }

    fn replace_current(&mut self) {
        let Some(found) = self.find.current_match().cloned() else {
            return;
        };
        if self
            .controller
            .replace_match_active(&found, &self.find.replacement)
            .unwrap_or(false)
        {
            self.refresh_find(true);
        }
    }

    fn handle_sidebar_click(&mut self, layout: Layout, x: i32, y: i32) {
        let sort = self.sidebar.sort_button(layout.sidebar);
        if sort.contains(x, y) {
            self.sidebar.toggle_sort();
            self.refresh_sidebar();
            return;
        }
        let search = Rect::new(
            layout.sidebar.x + 12,
            layout.sidebar.y + 40,
            layout.sidebar.w - 24,
            30,
        );
        if search.contains(x, y) {
            self.focus_input(InputFocus::Sidebar);
            return;
        }
        let Some((id, action)) = self.sidebar.note_at(layout.sidebar, x, y) else {
            return;
        };
        match action {
            NoteAction::Open => {
                if self.controller.open_note(id).is_ok() {
                    self.reset_native_tab();
                    self.focus_input(InputFocus::Editor);
                }
            }
            NoteAction::TogglePin => {
                let _ = self.controller.toggle_pin(id);
                self.refresh_sidebar();
            }
            NoteAction::Delete => {
                let result = MessageDialog::new()
                    .set_title("Delete note")
                    .set_description("Delete this note from the Notes library?")
                    .set_buttons(MessageButtons::YesNo)
                    .show();
                if matches!(result, MessageDialogResult::Yes | MessageDialogResult::Ok) {
                    let _ = self.controller.delete_note(id);
                    self.refresh_sidebar();
                }
            }
        }
    }

    fn handle_extract_click(&mut self, layout: Layout, x: i32, y: i32) {
        let bounds = layout.extract_panel;
        let close = Rect::new(bounds.right() - 42, bounds.y + 8, 32, 28);
        if close.contains(x, y) {
            self.extract.open = false;
            return;
        }
        for index in 0..self.extract.available_colours.len() {
            if self.extract.colour_row(index).contains(x, y) {
                if let Some((colour, _)) = self.extract.available_colours.get(index).copied() {
                    self.extract.toggle_colour(colour);
                    self.refresh_extract();
                }
                return;
            }
        }
        let order_y = bounds.y + 77;
        let document = Rect::new(bounds.x + 70, order_y - 15, 98, 24);
        let grouped = Rect::new(document.right() + 5, order_y - 15, 90, 24);
        if document.contains(x, y) {
            self.extract.order = ColorOrder::Document;
            self.refresh_extract();
            return;
        }
        if grouped.contains(x, y) {
            self.extract.order = ColorOrder::Grouped;
            self.refresh_extract();
            return;
        }
        let copy = Rect::new(bounds.x + 14, bounds.bottom() - 40, 72, 28);
        let new_tab = Rect::new(copy.right() + 6, bounds.bottom() - 40, 84, 28);
        let export = Rect::new(new_tab.right() + 6, bounds.bottom() - 40, 72, 28);
        if copy.contains(x, y) {
            let _ = self.controller.copy_to_clipboard(self.extract.preview.clone());
            self.extract.copied = true;
        } else if new_tab.contains(x, y) {
            self.controller.new_tab_with_text(&self.extract.preview);
            self.reset_native_tab();
        } else if export.contains(x, y) {
            let _ = self
                .controller
                .save_extracted_text(&self.extract.preview, "extracted.txt");
        }
    }

    fn insert_input_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        match self.focus {
            InputFocus::Editor => self.controller.insert_text(text),
            InputFocus::Find => {
                self.find.query.push_str(text);
                self.find.current = 0;
                self.refresh_find(true);
            }
            InputFocus::Replacement => self.find.replacement.push_str(text),
            InputFocus::Sidebar => {
                self.sidebar.query.push_str(text);
                self.refresh_sidebar();
            }
        }
    }

    fn backspace_input(&mut self) {
        match self.focus {
            InputFocus::Editor => self.controller.delete_backward(),
            InputFocus::Find => {
                self.find.query.pop();
                self.find.current = 0;
                self.refresh_find(true);
            }
            InputFocus::Replacement => {
                self.find.replacement.pop();
            }
            InputFocus::Sidebar => {
                self.sidebar.query.pop();
                self.refresh_sidebar();
            }
        }
    }

    fn delete_input(&mut self) {
        match self.focus {
            InputFocus::Editor => self.controller.delete_forward(),
            InputFocus::Find => {
                self.find.query.clear();
                self.find.current = 0;
                self.refresh_find(false);
            }
            InputFocus::Replacement => self.find.replacement.clear(),
            InputFocus::Sidebar => {
                self.sidebar.query.clear();
                self.refresh_sidebar();
            }
        }
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
        let control = self.modifiers.control_key() || self.modifiers.super_key();
        let shift = self.modifiers.shift_key();
        match key {
            Key::Named(NamedKey::Escape) if self.find.open => {
                self.find.open = false;
                self.focus_input(InputFocus::Editor);
            }
            Key::Named(NamedKey::Enter) if self.find.open && self.focus == InputFocus::Find => {
                let found = if shift {
                    self.find.previous_match().cloned()
                } else {
                    self.find.next_match().cloned()
                };
                if let Some(found) = found {
                    self.controller.select_find_match(&found);
                }
            }
            Key::Named(NamedKey::Enter)
                if self.find.open && self.focus == InputFocus::Replacement => {}
            Key::Named(NamedKey::Enter) => self.controller.handle_enter(),
            Key::Named(NamedKey::Backspace) => self.backspace_input(),
            Key::Named(NamedKey::Delete) => self.delete_input(),
            Key::Named(NamedKey::Tab) if self.focus == InputFocus::Editor => {
                self.controller.handle_tab(shift)
            }
            Key::Named(NamedKey::Tab) if self.find.open => {
                if self.focus == InputFocus::Find {
                    self.focus_input(InputFocus::Replacement);
                } else {
                    self.focus_input(InputFocus::Find);
                }
            }
            Key::Named(NamedKey::ArrowLeft) if self.focus == InputFocus::Editor => {
                self.controller.move_active_cursor(-1, false, shift)
            }
            Key::Named(NamedKey::ArrowRight) if self.focus == InputFocus::Editor => {
                self.controller.move_active_cursor(1, false, shift)
            }
            Key::Named(NamedKey::ArrowUp) if self.focus == InputFocus::Editor => {
                self.controller.move_active_cursor(-1, true, shift)
            }
            Key::Named(NamedKey::ArrowDown) if self.focus == InputFocus::Editor => {
                self.controller.move_active_cursor(1, true, shift)
            }
            Key::Named(NamedKey::Home) if self.focus == InputFocus::Editor => {
                self.controller.move_active_line_start(shift)
            }
            Key::Named(NamedKey::End) if self.focus == InputFocus::Editor => {
                self.controller.move_active_line_end(shift)
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("a") => {
                match self.focus {
                    InputFocus::Editor => self.controller.select_all(),
                    InputFocus::Find => {}
                    InputFocus::Replacement => {}
                    InputFocus::Sidebar => {}
                }
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("n") => {
                self.controller.new_tab();
                self.reset_native_tab();
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("w") => {
                self.request_close_tab(self.controller.active_tab_index());
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("o") => {
                if let Ok(paths) = self.controller.open_file_dialog() {
                    for path in paths {
                        let _ = self.controller.open_document(path);
                    }
                    self.reset_native_tab();
                }
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("s") => {
                self.save_current();
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("f") => {
                self.find.open = true;
                self.find.show_replace = false;
                self.focus_input(InputFocus::Find);
                self.refresh_find(true);
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("h") => {
                self.find.open = true;
                self.find.show_replace = true;
                self.focus_input(InputFocus::Find);
                self.refresh_find(true);
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("z") => {
                self.controller.undo();
                self.reset_native_tab();
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("y") => {
                self.controller.redo();
                self.reset_native_tab();
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("c") => {
                if let Some(text) = self.controller.selected_text() {
                    let _ = self.controller.copy_to_clipboard(text);
                }
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("x") => {
                if let Some(text) = self.controller.selected_text() {
                    let _ = self.controller.copy_to_clipboard(text);
                    self.controller.delete_backward();
                }
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("v") => {
                if let Ok(Some(text)) = self.controller.paste_from_clipboard() {
                    self.insert_input_text(&text);
                }
            }
            Key::Character(value) if control && (value.as_str() == "+" || value.as_str() == "=") => {
                self.controller.adjust_font_size(1.0);
            }
            Key::Character(value) if control && value.as_str() == "-" => {
                self.controller.adjust_font_size(-1.0);
            }
            Key::Character(value) if !control && !self.modifiers.alt_key() => {
                // KeyboardInput delivers ordinary printable characters even when
                // the platform has not sent an IME commit event.  The old code
                // gated this path on ime_enabled, which made every painted
                // search field appear read-only.  Keep IME commits below for
                // composed text, but never block direct native key input here.
                self.insert_input_text(value.as_str());
            }
            _ => {}
        }
        self.apply_command(event_loop);
    }

    fn reset_native_tab(&mut self) {
        self.scroll_line = 0;
        #[cfg(windows)]
        {
            self.native_tab = usize::MAX;
            self.native_selection = (0, 0);
            self.native_text.clear();
        }
    }

    fn sync_native_editor(&mut self, layout: Layout) {
        #[cfg(windows)]
        if let Some(host) = self.scintilla.as_ref() {
            host.resize(
                layout.editor.x,
                layout.editor.y,
                layout.editor.w.max(1),
                layout.editor.h.max(1),
            );
            let scintilla_theme = self.controller.theme().scintilla();
            let settings = self.controller.get_settings();
            host.direct().set_colours(
                (
                    scintilla_theme.foreground.r,
                    scintilla_theme.foreground.g,
                    scintilla_theme.foreground.b,
                ),
                (
                    scintilla_theme.background.r,
                    scintilla_theme.background.g,
                    scintilla_theme.background.b,
                ),
                (
                    scintilla_theme.caret.r,
                    scintilla_theme.caret.g,
                    scintilla_theme.caret.b,
                ),
            );
            host.direct().set_font(&settings.font_family, settings.font_size);
            host.direct().set_word_wrap(self.controller.word_wrap());
            let tab = self.controller.active_tab_index();
            let Some(snapshot) = self.controller.active_snapshot() else {
                return;
            };
            if self.native_tab != tab {
                host.direct().set_text(&snapshot.text);
                host.direct()
                    .set_selection(snapshot.selection.anchor, snapshot.selection.caret);
                self.native_selection = (snapshot.selection.anchor, snapshot.selection.caret);
                self.native_text = snapshot.text.clone();
                self.native_tab = tab;
            } else {
                let native = host.direct().get_text();
                if native != self.native_text {
                    self.controller.replace_active_text(&native);
                    self.native_text = native;
                    let native_selection = (
                        host.direct().get_anchor(),
                        host.direct().get_current_pos(),
                    );
                    self.controller
                        .set_active_selection(native_selection.0, native_selection.1);
                    self.native_selection = native_selection;
                } else if snapshot.text != self.native_text {
                    host.direct().set_text(&snapshot.text);
                    host.direct()
                        .set_selection(snapshot.selection.anchor, snapshot.selection.caret);
                    self.native_selection = (snapshot.selection.anchor, snapshot.selection.caret);
                    self.native_text = snapshot.text.clone();
                } else {
                    let core_selection = (snapshot.selection.anchor, snapshot.selection.caret);
                    let native_selection = (
                        host.direct().get_anchor(),
                        host.direct().get_current_pos(),
                    );
                    if core_selection != self.native_selection
                        && native_selection == self.native_selection
                    {
                        host.direct()
                            .set_selection(core_selection.0, core_selection.1);
                        self.native_selection = core_selection;
                    } else if native_selection != self.native_selection
                        && core_selection == self.native_selection
                    {
                        self.controller
                            .set_active_selection(native_selection.0, native_selection.1);
                        self.native_selection = native_selection;
                    } else if core_selection != self.native_selection {
                        host.direct()
                            .set_selection(core_selection.0, core_selection.1);
                        self.native_selection = core_selection;
                    }
                }
            }
            let length = host.direct().get_text().len();
            for slot in 0..7 {
                host.direct().clear_indicator(slot, 0, length);
            }
            for (line, metadata) in snapshot.metadata.iter().enumerate() {
                if let Some(rgb) = metadata.colour.rgb() {
                    let slot = indicator_slot(metadata.colour);
                    let line_colour = ((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
                    host.direct().configure_indicator(slot, line_colour, 105);
                    host.direct().set_indicator_range(
                        slot,
                        host.direct().line_start(line),
                        host.direct().line_length(line),
                    );
                }
            }
        }
        #[cfg(not(windows))]
        let _ = layout;
    }

    fn apply_command(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        match self.controller.take_window_command() {
            Some(WindowCommand::Minimize) => window.set_minimized(true),
            Some(WindowCommand::ToggleMaximize) => window.set_maximized(!window.is_maximized()),
            Some(WindowCommand::Close) => event_loop.exit(),
            Some(WindowCommand::ConfirmClose) => {
                let result = MessageDialog::new()
                    .set_title("Unsaved changes")
                    .set_description("Save changes before closing NotePad Pro?")
                    .set_buttons(MessageButtons::YesNoCancel)
                    .show();
                match result {
                    MessageDialogResult::Yes => {
                        let current = self.controller.active_tab_index();
                        let mut all_saved = true;
                        for index in 0..self.controller.tab_count() {
                            if !self.controller.tab_is_dirty(index) {
                                continue;
                            }
                            if self.controller.switch_tab(index) {
                                let mut saved = self.controller.save_active().unwrap_or(false);
                                if !saved {
                                    saved = self.controller.save_active_as().ok().flatten().is_some();
                                }
                                if !saved {
                                    all_saved = false;
                                    break;
                                }
                            }
                        }
                        self.controller.switch_tab(current.min(self.controller.tab_count().saturating_sub(1)));
                        if all_saved {
                            self.controller.confirm_close();
                        }
                    }
                    MessageDialogResult::No => self.controller.confirm_close(),
                    MessageDialogResult::Cancel
                    | MessageDialogResult::Ok
                    | MessageDialogResult::Custom(_) => {}
                }
                self.apply_command(event_loop);
            }
            None => {}
        }
    }
}

impl ApplicationHandler for NativeApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.create_window(event_loop) {
            eprintln!("could not create NotePad Pro window: {error}");
            event_loop.exit();
            return;
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.window.as_ref().map(|window| window.id()) != Some(id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.controller.close_window();
                self.apply_command(event_loop);
            }
            WindowEvent::Resized(size) => {
                self.resize(size.width, size.height);
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.render_frame(),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = (position.x as i32, position.y as i32);
                if self.mouse_down && self.focus == InputFocus::Editor {
                    let size = self.window.as_ref().map(|window| window.inner_size());
                    if let (Some(size), Some(snapshot)) = (size, self.controller.active_snapshot()) {
                        let layout = self.current_layout(size.width, size.height);
                        let settings = self.controller.get_settings();
                        let cursor = hit_cursor(
                            &snapshot,
                            layout,
                            self.pointer.0,
                            self.pointer.1,
                            settings.font_size,
                            self.scroll_line,
                        );
                        self.controller.set_active_selection(self.drag_anchor, cursor);
                    }
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let direction = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y.signum() as i32,
                    MouseScrollDelta::PixelDelta(position) => position.y.signum() as i32,
                };
                if direction > 0 {
                    self.scroll_line = self.scroll_line.saturating_sub(3);
                } else if direction < 0 {
                    self.scroll_line = self.scroll_line.saturating_add(3);
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => self.handle_mouse(event_loop, button),
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.mouse_down = false,
            WindowEvent::Ime(Ime::Commit(text)) => {
                if !text.is_empty() {
                    self.insert_input_text(&text);
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                self.handle_key(event_loop, &event.logical_key);
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let settings = self.controller.get_settings();
        if settings.autosave
            // Notes are cheap SQLite rows.  A short idle debounce makes the
            // sidebar useful immediately, while the setting still provides a
            // genuine opt-out for users who do not want background writes.
            && self.last_autosave.elapsed() >= Duration::from_millis(750)
        {
            let _ = self.controller.autosave();
            self.last_autosave = Instant::now();
        }
        if self.last_redraw.elapsed() >= Duration::from_millis(16) {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
    }
}

fn attach_native_editor(window: &Window, layout: Layout) -> Option<ScintillaHost> {
    #[cfg(windows)]
    {
        let handle = window.window_handle().ok()?.as_raw();
        let parent = match handle {
            RawWindowHandle::Win32(value) => value.hwnd.get() as *mut c_void,
            _ => return None,
        };
        unsafe {
            ScintillaHost::attach(
                parent,
                layout.editor.x,
                layout.editor.y,
                layout.editor.w.max(1),
                layout.editor.h.max(1),
            )
            .ok()
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (window, layout);
        None
    }
}

fn resize_surface(
    surface: &mut softbuffer::Surface<Arc<Window>, Arc<Window>>,
    width: u32,
    height: u32,
) {
    let _ = surface.resize(
        NonZeroU32::new(width.max(1)).expect("nonzero width"),
        NonZeroU32::new(height.max(1)).expect("nonzero height"),
    );
}

fn app_icon_pixels() -> Vec<u8> {
    let mut pixels = vec![0_u8; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let index = (y * 32 + x) * 4;
            let page = (7..26).contains(&x) && (4..29).contains(&y);
            let fold = x >= 20 && y < 10;
            let (red, green, blue, alpha) = if page {
                if fold {
                    (129, 140, 248, 255)
                } else {
                    (37, 40, 64, 255)
                }
            } else {
                (37, 40, 64, 0)
            };
            pixels[index..index + 4].copy_from_slice(&[red, green, blue, alpha]);
        }
    }
    pixels
}

#[cfg(windows)]
fn indicator_slot(colour: LineColour) -> usize {
    match colour {
        LineColour::Yellow => 0,
        LineColour::Green => 1,
        LineColour::Pink => 2,
        LineColour::Blue => 3,
        LineColour::Orange => 4,
        LineColour::Purple => 5,
        LineColour::Custom(_) => 6,
        LineColour::None => 0,
    }
}

fn hit_cursor(
    snapshot: &EditorSnapshot,
    layout: Layout,
    x: i32,
    y: i32,
    size: f32,
    scroll_line: usize,
) -> usize {
    let line_height = (size * 1.55).ceil() as i32;
    let margin = Layout::line_number_width(
        snapshot.text.split('\n').count(),
        (size * 0.62) as i32,
    );
    let lines: Vec<&str> = snapshot.text.split('\n').collect();
    let line = ((((y - layout.editor.y - 8).max(0) / line_height.max(1)) as usize)
        + scroll_line)
        .min(lines.len().saturating_sub(1));
    let start = lines
        .iter()
        .take(line)
        .map(|value| value.len() + 1)
        .sum::<usize>();
    let column = ((x - layout.editor.x - margin - 14).max(0) as f32
        / (size * 0.62).max(1.0)) as usize;
    let mut position = start;
    for (character_index, (index, character)) in lines[line].char_indices().enumerate() {
        if character_index >= column {
            break;
        }
        position = start + index + character.len_utf8();
    }
    position.min(snapshot.text.len())
}
