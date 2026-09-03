use crate::controller::{AppController, EditorSnapshot, WindowCommand};
use crate::layout::{hit_test, HitTarget, Layout};
use crate::renderer::ChromeRenderer;
use crate::scintilla::ScintillaHost;
use crate::ui::find_bar::FindBarState;
use crate::ui::toolbar::ToolbarAction;
use rfd::{MessageButtons,MessageDialog,MessageDialogResult};
#[cfg(windows)]use raw_window_handle::{HasWindowHandle,RawWindowHandle};
#[cfg(windows)]use std::ffi::c_void;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

pub fn run(controller: Arc<AppController>) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|error| error.to_string())?;
    let mut application = NativeApplication::new(controller);
    event_loop
        .run_app(&mut application)
        .map_err(|error| error.to_string())
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
    renderer: ChromeRenderer,
    find: FindBarState,
    modifiers: ModifiersState,
    ime_enabled: bool,
    pointer: (i32, i32),
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
            renderer: ChromeRenderer::new(),
            find: FindBarState::default(),
            modifiers: ModifiersState::empty(),
            ime_enabled: true,
            pointer: (0, 0),
            last_redraw: Instant::now(),
            last_autosave: Instant::now(),
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        if self.window.is_some() {
            return Ok(());
        }
        let state = self.controller.window_state();
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
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
        let layout = Layout::compute(state.width,state.height,self.controller.get_settings().sidebar_open,false);
        let scintilla = attach_native_editor(&window, layout);
        let surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|error| format!("softbuffer surface: {error}"))?;
        self.window = Some(window);
        self._context = Some(context);
        self.surface = Some(surface);
        self.scintilla = scintilla;
        Ok(())
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
        let layout = Layout::compute(size.width, size.height, settings.sidebar_open, self.find.open);
        self.sync_native_editor(layout);
        let snapshot = self.controller.active_snapshot();
        let tabs = self.controller.tab_summaries();
        let notes = self.controller.get_notes_list(None).unwrap_or_default();
        let Some(pixmap) = self.renderer.render(
            size.width,
            size.height,
            layout,
            self.controller.theme(),
            snapshot.as_ref(),
            &tabs,
            &notes,
            &self.find,
            settings.show_line_numbers,
            self.controller.active_tab_index(),
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

    fn handle_mouse(&mut self, event_loop: &ActiveEventLoop, button: MouseButton) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        let size = window.inner_size();
        let settings = self.controller.get_settings();
        let layout = Layout::compute(size.width, size.height, settings.sidebar_open, self.find.open);
        match hit_test(layout, self.pointer.0, self.pointer.1) {
            HitTarget::Minimize if button == MouseButton::Left => self.controller.minimise(),
            HitTarget::Maximize if button == MouseButton::Left => self.controller.toggle_maximise(),
            HitTarget::Close if button == MouseButton::Left => {
                self.controller.close_window();
            }
            HitTarget::Drag if button == MouseButton::Left => {
                let _ = window.drag_window();
            }
            HitTarget::Editor if button == MouseButton::Left => {
                if let Some(snapshot) = self.controller.active_snapshot() {
                    let line_height = (settings.font_size * 1.55).ceil() as i32;
                    let line = ((self.pointer.1 - layout.editor.y - 8).max(0)
                        / line_height.max(1)) as usize;
                    let margin = Layout::line_number_width(
                        snapshot.text.split('\n').count(),
                        (settings.font_size * 0.62) as i32,
                    );
                    let in_marker = self.pointer.0 < layout.editor.x + margin;
                    if in_marker && snapshot.metadata.get(line).is_some_and(|metadata| {
                        metadata.list_type == notepad_core::ListType::Check
                    }) {
                        self.controller.toggle_checkbox(line);
                    } else {
                        let cursor = hit_cursor(
                            &snapshot,
                            layout,
                            self.pointer.0,
                            self.pointer.1,
                            settings.font_size,
                        );
                        self.controller.set_active_cursor(cursor, false);
                    }
                }
            }
            HitTarget::TabBar if button == MouseButton::Left => {
                for (index, tab) in crate::ui::tab_bar::layout_tabs(layout, &self.controller.tab_summaries()).iter().enumerate() {
                    if tab.rect.contains(self.pointer.0, self.pointer.1) {
                        self.controller.switch_tab(index);
                        break;
                    }
                }
            }
            HitTarget::Toolbar if button == MouseButton::Left => {
                for (button, action) in crate::ui::toolbar::buttons(layout) {
                    if button.contains(self.pointer.0, self.pointer.1) {
                        match action {
                            ToolbarAction::New => self.controller.new_tab(),
                            ToolbarAction::Open => {
                                if let Ok(paths) = self.controller.open_file_dialog() {
                                    for path in paths { let _ = self.controller.open_document(path); }
                                }
                            }
                            ToolbarAction::Save => {
                                if !self.controller.save_active().unwrap_or(false) { let _ = self.controller.save_active_as(); }
                            }
                            ToolbarAction::Undo => { self.controller.undo(); }
                            ToolbarAction::Redo => { self.controller.redo(); }
                            ToolbarAction::Find => { self.find.open = true; }
                            ToolbarAction::Highlight(colour) => self.controller.apply_highlight(colour),
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
        self.apply_command(event_loop);
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
        let control = self.modifiers.control_key() || self.modifiers.super_key();
        let shift = self.modifiers.shift_key();
        match key {
            Key::Named(NamedKey::Escape) => self.find.open = false,
            Key::Named(NamedKey::Enter) if self.find.open => {
                if let Some(snapshot) = self.controller.active_snapshot() {
                    let _ = self.find.refresh(&snapshot.text);
                }
            }
            Key::Named(NamedKey::Enter) => self.controller.handle_enter(),
            Key::Named(NamedKey::Backspace) if self.find.open => {
                self.find.query.pop();
                if let Some(snapshot) = self.controller.active_snapshot() {
                    let _ = self.find.refresh(&snapshot.text);
                }
            }
            Key::Named(NamedKey::Delete) if self.find.open => {
                self.find.query.clear();
                if let Some(snapshot) = self.controller.active_snapshot() {
                    let _ = self.find.refresh(&snapshot.text);
                }
            }
            Key::Named(NamedKey::Backspace) => self.controller.delete_backward(),
            Key::Named(NamedKey::Delete) => self.controller.delete_forward(),
            Key::Named(NamedKey::Tab) => self.controller.handle_tab(shift),
            Key::Named(NamedKey::ArrowLeft) => {
                self.controller.move_active_cursor(-1, false, shift)
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.controller.move_active_cursor(1, false, shift)
            }
            Key::Named(NamedKey::ArrowUp) => self.controller.move_active_cursor(-1, true, shift),
            Key::Named(NamedKey::ArrowDown) => self.controller.move_active_cursor(1, true, shift),
            Key::Named(NamedKey::Home) => self.controller.move_active_line_start(shift),
            Key::Named(NamedKey::End) => self.controller.move_active_line_end(shift),
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("a") => {
                self.controller.select_all();
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("n") => {
                self.controller.new_tab();
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("o") => {
                if let Ok(paths) = self.controller.open_file_dialog() {
                    for path in paths {
                        let _ = self.controller.open_document(path);
                    }
                }
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("s") => {
                if !self.controller.save_active().unwrap_or(false) {
                    let _ = self.controller.save_active_as();
                }
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("f") => {
                self.find.open = true;
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("z") => {
                self.controller.undo();
            }
            Key::Character(value) if control && value.as_str().eq_ignore_ascii_case("y") => {
                self.controller.redo();
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
                    self.controller.replace_selection(&text);
                }
            }
            Key::Character(value) if !control && !self.modifiers.alt_key() && !self.ime_enabled => {
                if self.find.open {
                    self.find.query.push_str(value.as_str());
                    if let Some(snapshot) = self.controller.active_snapshot() {
                        let _ = self.find.refresh(&snapshot.text);
                    }
                } else {
                    self.controller.insert_text(value.as_str());
                }
            }
            _ => {}
        }
        self.apply_command(event_loop);
    }

    fn sync_native_editor(&mut self, layout: Layout) {
        #[cfg(windows)]
        if let Some(host) = self.scintilla.as_ref() {
            host.resize(layout.editor.x,layout.editor.y,layout.editor.w.max(1),layout.editor.h.max(1));
            let theme=self.controller.theme().scintilla();
            host.direct().set_colours((theme.foreground.r,theme.foreground.g,theme.foreground.b),(theme.background.r,theme.background.g,theme.background.b),(theme.caret.r,theme.caret.g,theme.caret.b));
            let tab=self.controller.active_tab_index();
            let Some(snapshot)=self.controller.active_snapshot() else{return};
            if self.native_tab!=tab {
                host.direct().set_text(&snapshot.text);
                self.native_text=snapshot.text.clone();
                self.native_tab=tab;
            } else {
                let native=host.direct().get_text();
                if native!=self.native_text {
                    self.controller.replace_active_text(&native);
                    self.native_text=native;
                } else if snapshot.text!=self.native_text {
                    host.direct().set_text(&snapshot.text);
                    self.native_text=snapshot.text.clone();
                }
            }
            host.direct().configure_indicator(0,(theme.caret.r,theme.caret.g,theme.caret.b),105);
            let length=host.direct().get_text().len();
            host.direct().clear_indicator(0,0,length);
            for (line,metadata) in snapshot.metadata.iter().enumerate(){
                if let Some(rgb)=metadata.colour.rgb(){
                    let colour=((rgb>>16)as u8,(rgb>>8)as u8,rgb as u8);
                    host.direct().configure_indicator(0,colour,105);
                    host.direct().set_indicator_range(0,host.direct().line_start(line),host.direct().line_length(line));
                }
            }
        }
        #[cfg(not(windows))]
        let _=layout;
    }

    fn apply_command(&self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        match self.controller.take_window_command() {
            Some(WindowCommand::Minimize) => window.set_minimized(true),
            Some(WindowCommand::ToggleMaximize) => window.set_maximized(!window.is_maximized()),
            Some(WindowCommand::Close) => event_loop.exit(),
            Some(WindowCommand::ConfirmClose) => {let result=MessageDialog::new().set_title("Unsaved changes").set_description("Save changes before closing NotePad Pro?").set_buttons(MessageButtons::YesNoCancel).show();match result{MessageDialogResult::Yes=>{let saved=self.controller.save_active().unwrap_or(false);if saved||self.controller.save_active_as().ok().flatten().is_some(){self.controller.confirm_close()}}MessageDialogResult::No=>self.controller.confirm_close(),MessageDialogResult::Cancel|MessageDialogResult::Ok|MessageDialogResult::Custom(_)=>{}}self.apply_command(event_loop)}
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
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => self.handle_mouse(event_loop, button),
            WindowEvent::Ime(Ime::Enabled) => self.ime_enabled = true,
            WindowEvent::Ime(Ime::Disabled) => self.ime_enabled = false,
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.ime_enabled = true;
                if self.find.open {
                    self.find.query.push_str(&text);
                    if let Some(snapshot) = self.controller.active_snapshot() {
                        let _ = self.find.refresh(&snapshot.text);
                    }
                } else if !self.modifiers.control_key() && !self.modifiers.alt_key() {
                    self.controller.insert_text(&text);
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
        let settings=self.controller.get_settings();
        if settings.autosave&&self.last_autosave.elapsed()>=Duration::from_secs(settings.autosave_seconds){let _=self.controller.autosave();self.last_autosave=Instant::now();}
        if self.last_redraw.elapsed() >= Duration::from_millis(16) {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
    }
}

fn attach_native_editor(window:&Window,layout:Layout)->Option<ScintillaHost>{
    #[cfg(windows)]
    {
        let handle=window.window_handle().ok()?.as_raw();
        let parent=match handle{RawWindowHandle::Win32(value)=>value.hwnd.get() as *mut c_void,_=>return None};
        ScintillaHost::attach(parent,layout.editor.x,layout.editor.y,layout.editor.w.max(1),layout.editor.h.max(1)).ok()
    }
    #[cfg(not(windows))]
    {let _=(window,layout);None}
}

fn resize_surface(surface: &mut softbuffer::Surface<Arc<Window>, Arc<Window>>, width: u32, height: u32) {
    let _ = surface.resize(
        NonZeroU32::new(width.max(1)).expect("nonzero width"),
        NonZeroU32::new(height.max(1)).expect("nonzero height"),
    );
}

fn hit_cursor(snapshot: &EditorSnapshot, layout: Layout, x: i32, y: i32, size: f32) -> usize {
    let line_height = (size * 1.55).ceil() as i32;
    let margin = Layout::line_number_width(snapshot.text.split('\n').count(), (size * 0.62) as i32);
    let lines: Vec<&str> = snapshot.text.split('\n').collect();
    let line = (((y - layout.editor.y - 8).max(0) / line_height.max(1)) as usize)
        .min(lines.len().saturating_sub(1));
    let start = lines.iter().take(line).map(|value| value.len() + 1).sum::<usize>();
    let column = ((x - layout.editor.x - margin - 14).max(0) as f32 / (size * 0.62).max(1.0)) as usize;
    let mut position = start;
    for (character_index,(index,character)) in lines[line].char_indices().enumerate() {
        if character_index >= column {break;}
        position = start + index + character.len_utf8();
    }
    position.min(snapshot.text.len())
}
