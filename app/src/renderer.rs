use crate::controller::EditorSnapshot;
use crate::layout::{Layout, Rect};
use crate::theme::{Rgba, Theme};
use crate::ui::extract_panel::ExtractPanelState;
use crate::ui::{self, FindBarState};
use fontdue::{Font, FontSettings};
use notepad_core::{ColorOrder, LineColour, ListType, Note};
use std::path::Path;
use std::time::Instant;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

pub struct ChromeRenderer {
    font: Option<Font>,
    font_size: f32,
    started: Instant,
}

impl Default for ChromeRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ChromeRenderer {
    pub fn new() -> Self {
        Self {
            font: load_font(),
            font_size: 15.0,
            started: Instant::now(),
        }
    }

    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size.clamp(8.0, 96.0);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        width: u32,
        height: u32,
        layout: Layout,
        theme: Theme,
        snapshot: Option<&EditorSnapshot>,
        tabs: &[(String, bool)],
        notes: &[Note],
        find: &FindBarState,
        extract: &ExtractPanelState,
        sidebar_query: &str,
        sidebar_focused: bool,
        sidebar_sort: &str,
        show_line_numbers: bool,
        word_wrap: bool,
        active_tab: usize,
        pointer: (i32, i32),
    ) -> Option<Pixmap> {
        let mut pixmap = Pixmap::new(width.max(1), height.max(1))?;
        fill_rect(
            &mut pixmap,
            Rect::new(0, 0, width as i32, height as i32),
            theme.window_bg,
        );
        self.background(&mut pixmap, width, height, theme);
        self.title(&mut pixmap, layout, theme);
        self.tabs(&mut pixmap, layout, theme, tabs, active_tab, pointer);
        self.toolbar(&mut pixmap, layout, theme, pointer);
        if find.open {
            self.find(&mut pixmap, layout, theme, find, pointer);
        }
        self.sidebar(
            &mut pixmap,
            layout,
            theme,
            notes,
            sidebar_query,
            sidebar_focused,
            sidebar_sort,
            pointer,
        );
        self.editor(
            &mut pixmap,
            layout,
            theme,
            snapshot,
            find,
            show_line_numbers,
            word_wrap,
        );
        if extract.open && layout.extract_open {
            self.extract(&mut pixmap, layout, theme, extract, pointer);
        }
        self.status(&mut pixmap, layout, theme, snapshot);
        Some(pixmap)
    }

    fn background(&self, pixmap: &mut Pixmap, width: u32, height: u32, theme: Theme) {
        let time = self.started.elapsed().as_secs_f32();
        for (index, (x, y, radius)) in
            [(0.72, 0.46, 360.0), (0.18, 0.72, 290.0), (0.88, 0.88, 210.0)]
                .into_iter()
                .enumerate()
        {
            let x = width as f32 * x + time.sin() * radius * 0.05;
            let y = height as f32 * y + (time * 0.8).cos() * radius * 0.04;
            fill_circle(
                pixmap,
                x,
                y,
                radius,
                if index % 2 == 0 {
                    theme.accent.with_alpha(10)
                } else {
                    theme.accent_alt.with_alpha(8)
                },
            );
        }
    }

    fn title(&self, pixmap: &mut Pixmap, layout: Layout, theme: Theme) {
        fill_rect(pixmap, layout.titlebar, theme.surface);
        draw_logo(
            pixmap,
            Rect::new(layout.titlebar.x + 14, layout.titlebar.y + 7, 20, 19),
            theme.accent,
            theme.editor_bg,
        );
        self.text(pixmap, 43, 21, "NotePad Pro", theme.text, 14.0);
        self.text(pixmap, 130, 20, "1.0.2", theme.muted_text, 10.0);
        let (minimise, maximise, close) = layout.titlebar_button_rects();
        fill_rect(pixmap, minimise, theme.surface_alt.with_alpha(70));
        fill_rect(pixmap, maximise, theme.surface_alt.with_alpha(70));
        fill_rect(pixmap, close, theme.accent.with_alpha(24));
        self.text(pixmap, minimise.x + 18, 21, "—", theme.text, 14.0);
        self.text(pixmap, maximise.x + 17, 21, "□", theme.text, 12.0);
        self.text(pixmap, close.x + 18, 21, "×", theme.text, 15.0);
    }

    fn tabs(
        &self,
        pixmap: &mut Pixmap,
        layout: Layout,
        theme: Theme,
        tabs: &[(String, bool)],
        active_tab: usize,
        pointer: (i32, i32),
    ) {
        fill_rect(pixmap, layout.tab_bar, theme.editor_bg);
        let tab_views = ui::tab_bar::layout_tabs(layout, tabs);
        for (index, view) in tab_views.iter().enumerate() {
            let active = index == active_tab;
            let hover = view.rect.contains(pointer.0, pointer.1);
            fill_rect(
                pixmap,
                view.rect,
                if active {
                    theme.surface
                } else if hover {
                    theme.surface_alt.with_alpha(150)
                } else {
                    theme.surface.with_alpha(85)
                },
            );
            if active {
                fill_rect(
                    pixmap,
                    Rect::new(view.rect.x, view.rect.bottom() - 2, view.rect.w, 2),
                    theme.accent,
                );
            }
            let title = if view.dirty {
                format!("{} •", view.title)
            } else {
                view.title.clone()
            };
            self.text(
                pixmap,
                view.rect.x + 12,
                view.rect.y + 18,
                &clip_text(&title, 22),
                if active { theme.text } else { theme.muted_text },
                11.5,
            );
            let close_rect = Rect::new(view.rect.right() - 30, view.rect.y + 3, 25, 24);
            if close_rect.contains(pointer.0, pointer.1) || active {
                self.text(
                    pixmap,
                    close_rect.x + 8,
                    close_rect.y + 17,
                    "×",
                    if close_rect.contains(pointer.0, pointer.1) {
                        theme.text
                    } else {
                        theme.muted_text
                    },
                    14.0,
                );
            }
        }
        let plus = layout.tab_plus_rect();
        fill_rect(
            pixmap,
            plus,
            if plus.contains(pointer.0, pointer.1) {
                theme.accent.with_alpha(35)
            } else {
                theme.surface.with_alpha(80)
            },
        );
        self.text(pixmap, plus.x + 16, plus.y + 26, "+", theme.accent, 20.0);
    }

    fn toolbar(&self, pixmap: &mut Pixmap, layout: Layout, theme: Theme, pointer: (i32, i32)) {
        fill_rect(pixmap, layout.toolbar, theme.surface.with_alpha(235));
        for (button, action) in ui::toolbar::buttons(layout) {
            let hover = button.contains(pointer.0, pointer.1);
            let background = if hover {
                theme.accent.with_alpha(45)
            } else {
                theme.surface
            };
            if let ui::toolbar::ToolbarAction::Highlight(colour) = action {
                fill_rect(pixmap, button.rect, line_colour(colour));
                fill_rect(
                    pixmap,
                    button.rect.inset(2),
                    line_colour(colour).with_alpha(if hover { 235 } else { 190 }),
                );
            } else {
                fill_rect(pixmap, button.rect, background);
                self.text(
                    pixmap,
                    button.rect.x + 9,
                    button.rect.y + 18,
                    &button.label,
                    theme.text,
                    10.5,
                );
            }
        }
        fill_rect(
            pixmap,
            Rect::new(layout.toolbar.x, layout.toolbar.bottom() - 1, layout.toolbar.w, 1),
            theme.border,
        );
    }

    fn find(
        &self,
        pixmap: &mut Pixmap,
        layout: Layout,
        theme: Theme,
        find: &FindBarState,
        pointer: (i32, i32),
    ) {
        fill_rect(pixmap, layout.find_bar, theme.surface_alt);
        let first_y = layout.find_bar.y + 8;
        let query_rect = Rect::new(layout.find_bar.x + 14, first_y, 300, 28);
        input_box(
            pixmap,
            query_rect,
            theme,
            find.focus == ui::find_bar::FindField::Query,
        );
        let query_text = if find.query.is_empty() {
            "Find in document…".to_owned()
        } else {
            clip_text(&find.query, 34)
        };
        self.text(
            pixmap,
            query_rect.x + 10,
            query_rect.y + 19,
            &query_text,
            if find.query.is_empty() {
                theme.muted_text
            } else {
                theme.text
            },
            11.0,
        );
        self.text(
            pixmap,
            query_rect.right() + 12,
            first_y + 19,
            &find.counter(),
            theme.muted_text,
            10.5,
        );
        let previous = Rect::new(query_rect.right() + 112, first_y, 30, 28);
        let next = Rect::new(previous.right() + 4, first_y, 30, 28);
        let replace_toggle = Rect::new(next.right() + 10, first_y, 92, 28);
        let close = Rect::new(layout.find_bar.right() - 40, first_y, 28, 28);
        small_button(pixmap, previous, theme, previous.contains(pointer.0, pointer.1));
        small_button(pixmap, next, theme, next.contains(pointer.0, pointer.1));
        self.text(pixmap, previous.x + 10, previous.y + 19, "‹", theme.text, 17.0);
        self.text(pixmap, next.x + 10, next.y + 19, "›", theme.text, 17.0);
        small_button(
            pixmap,
            replace_toggle,
            theme,
            replace_toggle.contains(pointer.0, pointer.1),
        );
        self.text(
            pixmap,
            replace_toggle.x + 9,
            replace_toggle.y + 18,
            if find.show_replace { "Hide replace" } else { "Replace" },
            theme.text,
            10.0,
        );
        self.text(pixmap, close.x + 8, close.y + 19, "×", theme.text, 15.0);

        if find.show_replace {
            let second_y = layout.find_bar.y + 48;
            let replacement = Rect::new(layout.find_bar.x + 14, second_y, 300, 28);
            input_box(
                pixmap,
                replacement,
                theme,
                find.focus == ui::find_bar::FindField::Replacement,
            );
            let replacement_text = if find.replacement.is_empty() {
                "Replace with…".to_owned()
            } else {
                clip_text(&find.replacement, 34)
            };
            self.text(
                pixmap,
                replacement.x + 10,
                replacement.y + 19,
                &replacement_text,
                if find.replacement.is_empty() {
                    theme.muted_text
                } else {
                    theme.text
                },
                11.0,
            );
            let replace = Rect::new(replacement.right() + 14, second_y, 74, 28);
            let replace_all = Rect::new(replace.right() + 6, second_y, 88, 28);
            small_button(pixmap, replace, theme, replace.contains(pointer.0, pointer.1));
            small_button(
                pixmap,
                replace_all,
                theme,
                replace_all.contains(pointer.0, pointer.1),
            );
            self.text(pixmap, replace.x + 11, second_y + 19, "Replace", theme.text, 10.0);
            self.text(
                pixmap,
                replace_all.x + 10,
                second_y + 19,
                "Replace all",
                theme.text,
                10.0,
            );
        }
        let check_y = if find.show_replace {
            layout.find_bar.y + 51
        } else {
            layout.find_bar.y + 12
        };
        let options = [
            (layout.find_bar.right() - 230, find.options.case_sensitive, "Match case"),
            (layout.find_bar.right() - 150, find.options.regex, "Regex"),
            (layout.find_bar.right() - 95, find.options.whole_word, "Whole"),
        ];
        for (x, checked, label) in options {
            let check = Rect::new(x, check_y, 20, 20);
            checkbox(pixmap, check, theme, checked);
            self.text(
                pixmap,
                check.right() + 5,
                check.y + 15,
                label,
                theme.muted_text,
                9.0,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sidebar(
        &self,
        pixmap: &mut Pixmap,
        layout: Layout,
        theme: Theme,
        notes: &[Note],
        query: &str,
        focused: bool,
        sort_label: &str,
        pointer: (i32, i32),
    ) {
        if !layout.sidebar_open || layout.sidebar.w <= 0 {
            return;
        }
        fill_rect(pixmap, layout.sidebar, theme.surface);
        self.text(
            pixmap,
            layout.sidebar.x + 16,
            layout.sidebar.y + 27,
            "Notes",
            theme.text,
            16.0,
        );
        self.text(
            pixmap,
            layout.sidebar.right() - 74,
            layout.sidebar.y + 26,
            &format!("{}", notes.len()),
            theme.muted_text,
            10.0,
        );
        let sort = Rect::new(layout.sidebar.right() - 94, layout.sidebar.y + 8, 78, 22);
        small_button(pixmap, sort, theme, sort.contains(pointer.0, pointer.1));
        self.text(pixmap, sort.x + 9, sort.y + 15, sort_label, theme.muted_text, 9.0);
        let search = Rect::new(
            layout.sidebar.x + 12,
            layout.sidebar.y + 40,
            layout.sidebar.w - 24,
            30,
        );
        input_box(pixmap, search, theme, focused);
        let search_text = if query.is_empty() {
            "⌕  Search notes…".to_owned()
        } else {
            format!("⌕  {}", clip_text(query, 28))
        };
        self.text(
            pixmap,
            search.x + 10,
            search.y + 20,
            &search_text,
            if query.is_empty() {
                theme.muted_text
            } else {
                theme.text
            },
            10.5,
        );
        for (index, note) in notes.iter().take(40).enumerate() {
            let card = Rect::new(
                layout.sidebar.x + 12,
                layout.sidebar.y + 82 + index as i32 * 74,
                layout.sidebar.w - 24,
                64,
            );
            let hover = card.contains(pointer.0, pointer.1);
            fill_rect(
                pixmap,
                card,
                if hover {
                    theme.accent.with_alpha(38)
                } else if note.pinned {
                    theme.accent.with_alpha(27)
                } else {
                    theme.surface_alt.with_alpha(125)
                },
            );
            let title = if note.title.is_empty() {
                "Untitled"
            } else {
                &note.title
            };
            self.text(
                pixmap,
                card.x + 10,
                card.y + 21,
                &clip_text(title, 22),
                theme.text,
                10.8,
            );
            if note.pinned {
                self.text(pixmap, card.right() - 52, card.y + 20, "★", theme.accent, 11.0);
            }
            self.text(
                pixmap,
                card.x + 10,
                card.y + 42,
                &clip_text(note.content.lines().next().unwrap_or(""), 30),
                theme.muted_text,
                9.5,
            );
            self.text(pixmap, card.right() - 28, card.y + 20, "×", theme.muted_text, 12.0);
        }
        fill_rect(
            pixmap,
            Rect::new(layout.sidebar.right() - 1, layout.sidebar.y, 1, layout.sidebar.h),
            theme.border,
        );
    }

    fn editor(
        &self,
        pixmap: &mut Pixmap,
        layout: Layout,
        theme: Theme,
        snapshot: Option<&EditorSnapshot>,
        find: &FindBarState,
        show_line_numbers: bool,
        word_wrap: bool,
    ) {
        fill_rect(pixmap, layout.editor, theme.editor_bg);
        let Some(snapshot) = snapshot else {
            return;
        };
        let lines: Vec<&str> = snapshot.text.split('\n').collect();
        let line_height = (self.font_size * 1.55).ceil() as i32;
        let margin = if show_line_numbers {
            Layout::line_number_width(lines.len(), (self.font_size * 0.62) as i32)
        } else {
            12
        };
        let max_chars = ((layout.editor.w - margin - 24).max(1) as f32
            / (self.font_size * 0.62).max(1.0)) as usize;
        let visual_lines = make_visual_lines(&lines, max_chars, word_wrap);
        if show_line_numbers {
            fill_rect(
                pixmap,
                Rect::new(layout.editor.x, layout.editor.y, margin, layout.editor.h),
                theme.surface.with_alpha(105),
            );
        }
        let cursor = snapshot.cursor.min(snapshot.text.len());
        let cursor_line = snapshot.text[..cursor]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        let line_start = snapshot.text[..cursor]
            .rfind('\n')
            .map_or(0, |position| position + 1);
        let cursor_column = snapshot.text[line_start..cursor].chars().count();
        let cursor_visual_line = visual_lines
            .iter()
            .position(|line| {
                line.logical == cursor_line
                    && cursor_column >= line.start_column
                    && cursor_column <= line.start_column + line.text.chars().count()
            })
            .unwrap_or_else(|| visual_lines.len().saturating_sub(1));
        let selected = snapshot.selection.range();
        let selection_start_line = snapshot
            .text
            .get(..selected.start)
            .unwrap_or("")
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        let selection_end_line = snapshot
            .text
            .get(..selected.end)
            .unwrap_or("")
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        let visible = ((layout.editor.h - 18).max(0) / line_height.max(1)) as usize + 1;
        let scroll_line = snapshot
            .scroll_line
            .min(visual_lines.len().saturating_sub(visible));
        for (visual_index, visual_line) in visual_lines
            .iter()
            .enumerate()
            .skip(scroll_line)
            .take(visible)
        {
            let logical_index = visual_line.logical;
            let line = visual_line.text;
            let screen_index = visual_index - scroll_line;
            let baseline = layout.editor.y + 24 + screen_index as i32 * line_height;
            let row = Rect::new(
                layout.editor.x + margin,
                baseline - line_height + 5,
                (layout.editor.w - margin - 8).max(0),
                line_height,
            );
            if visual_index == cursor_visual_line {
                fill_rect(pixmap, row, theme.accent.with_alpha(13));
            }
            if !selected.is_empty()
                && logical_index >= selection_start_line
                && logical_index <= selection_end_line
            {
                fill_rect(pixmap, row, theme.accent.with_alpha(48));
            }
            if let Some(metadata) = snapshot.metadata.get(logical_index) {
                if metadata.colour != LineColour::None {
                    fill_rect(pixmap, row, line_colour(metadata.colour).with_alpha(64));
                    fill_rect(
                        pixmap,
                        Rect::new(row.x, row.y, 3, row.h),
                        line_colour(metadata.colour),
                    );
                }
                let marker = match metadata.list_type {
                    ListType::Bullet => "•",
                    ListType::Check if metadata.checked => "☑",
                    ListType::Check => "☐",
                    ListType::Number => "·",
                    ListType::None => "",
                };
                if !marker.is_empty() && visual_line.start_column == 0 {
                    self.text(
                        pixmap,
                        layout.editor.x + 6 + metadata.indent as i32 * 14,
                        baseline,
                        marker,
                        theme.accent,
                        13.0,
                    );
                }
            }
            if show_line_numbers && visual_line.start_column == 0 {
                self.text(
                    pixmap,
                    layout.editor.x + margin - 10,
                    baseline,
                    &(logical_index + 1).to_string(),
                    theme.muted_text,
                    10.0,
                );
            }
            self.text(
                pixmap,
                layout.editor.x + margin + 14,
                baseline,
                &clip_text(line, 180),
                theme.text,
                self.font_size,
            );
            for found in &find.matches {
                if found.line == logical_index
                    && found.column >= visual_line.start_column
                    && found.column < visual_line.start_column + line.chars().count()
                {
                    let x = layout.editor.x
                        + margin
                        + 14
                        + ((found.column - visual_line.start_column) as f32
                            * self.font_size
                            * 0.62) as i32;
                    fill_rect(
                        pixmap,
                        Rect::new(x, baseline + 2, (found.end - found.start).max(2) as i32 * 7, 2),
                        theme.accent,
                    );
                }
            }
        }
        let column = cursor_column;
        fill_rect(
            pixmap,
            Rect::new(
                layout.editor.x
                    + margin
                    + 14
                    + ((column.saturating_sub(
                        visual_lines
                            .get(cursor_visual_line)
                            .map_or(0, |line| line.start_column),
                    )) as f32
                        * self.font_size
                        * 0.62) as i32,
                layout.editor.y
                    + 10
                    + cursor_visual_line.saturating_sub(scroll_line) as i32 * line_height,
                2,
                (line_height - 5).max(3),
            ),
            theme.accent,
        );
    }

    fn extract(
        &self,
        pixmap: &mut Pixmap,
        layout: Layout,
        theme: Theme,
        extract: &ExtractPanelState,
        pointer: (i32, i32),
    ) {
        let bounds = layout.extract_panel;
        fill_rect(pixmap, bounds, theme.surface);
        fill_rect(
            pixmap,
            Rect::new(bounds.x, bounds.y, 2, bounds.h),
            theme.accent,
        );
        self.text(pixmap, bounds.x + 16, bounds.y + 27, "Extract by Color", theme.text, 15.0);
        self.text(
            pixmap,
            bounds.right() - 30,
            bounds.y + 27,
            "×",
            theme.muted_text,
            15.0,
        );
        self.text(
            pixmap,
            bounds.x + 16,
            bounds.y + 49,
            "Choose line colors to collect",
            theme.muted_text,
            10.0,
        );
        for (index, (colour, count)) in extract.available_colours.iter().enumerate() {
            let row = extract.colour_row(index);
            let hover = row.contains(pointer.0, pointer.1);
            fill_rect(
                pixmap,
                row,
                if hover {
                    theme.surface_alt.with_alpha(170)
                } else {
                    theme.surface_alt.with_alpha(80)
                },
            );
            checkbox(
                pixmap,
                Rect::new(row.x + 6, row.y + 3, 20, 20),
                theme,
                extract.selected(*colour),
            );
            fill_rect(
                pixmap,
                Rect::new(row.x + 36, row.y + 7, 13, 13),
                line_colour(*colour),
            );
            self.text(
                pixmap,
                row.x + 58,
                row.y + 17,
                &format!("{}  ({count})", colour.name()),
                theme.text,
                10.5,
            );
        }
        let order_y = bounds.y + 77;
        self.text(pixmap, bounds.x + 16, order_y, "Order", theme.muted_text, 10.0);
        let document = Rect::new(bounds.x + 70, order_y - 15, 98, 24);
        let grouped = Rect::new(document.right() + 5, order_y - 15, 90, 24);
        small_button(pixmap, document, theme, document.contains(pointer.0, pointer.1));
        small_button(pixmap, grouped, theme, grouped.contains(pointer.0, pointer.1));
        if matches!(extract.order, ColorOrder::Document) {
            fill_rect(pixmap, document.inset(2), theme.accent.with_alpha(80));
        } else {
            fill_rect(pixmap, grouped.inset(2), theme.accent.with_alpha(80));
        }
        self.text(pixmap, document.x + 9, document.y + 16, "Document", theme.text, 9.5);
        self.text(pixmap, grouped.x + 10, grouped.y + 16, "By color", theme.text, 9.5);

        let preview = extract.preview_rect();
        fill_rect(pixmap, preview, theme.editor_bg);
        self.text(pixmap, preview.x + 10, preview.y + 19, "Live preview", theme.muted_text, 10.0);
        let mut y = preview.y + 38;
        for line in extract.preview.lines().take(12) {
            self.text(pixmap, preview.x + 10, y, &clip_text(line, 38), theme.text, 10.0);
            y += 17;
            if y > preview.bottom() - 22 {
                break;
            }
        }
        let copy = Rect::new(bounds.x + 14, bounds.bottom() - 40, 72, 28);
        let new_tab = Rect::new(copy.right() + 6, bounds.bottom() - 40, 84, 28);
        let export = Rect::new(new_tab.right() + 6, bounds.bottom() - 40, 72, 28);
        for button in [copy, new_tab, export] {
            small_button(pixmap, button, theme, button.contains(pointer.0, pointer.1));
        }
        self.text(pixmap, copy.x + 12, copy.y + 19, "Copy", theme.text, 10.0);
        self.text(pixmap, new_tab.x + 12, new_tab.y + 19, "New tab", theme.text, 10.0);
        self.text(pixmap, export.x + 11, export.y + 19, "Export", theme.text, 10.0);
        if extract.copied {
            self.text(pixmap, bounds.x + 16, bounds.bottom() - 51, "Copied", theme.accent, 9.0);
        }
    }

    fn status(
        &self,
        pixmap: &mut Pixmap,
        layout: Layout,
        theme: Theme,
        snapshot: Option<&EditorSnapshot>,
    ) {
        let track_height = layout.editor.h.max(1) as f32;
        let total_lines = snapshot.map_or(1, |value| value.text.split('\n').count().max(1));
        let visible_lines = ((layout.editor.h as f32) / (self.font_size * 1.55).max(1.0))
            .max(1.0) as usize;
        let thumb_height = (track_height
            * visible_lines.min(total_lines) as f32
            / total_lines as f32)
            .clamp(24.0_f32.min(track_height), track_height);
        let max_scroll = total_lines.saturating_sub(visible_lines);
        let scroll_ratio = snapshot.map_or(0.0, |value| {
            if max_scroll == 0 {
                0.0
            } else {
                value.scroll_line.min(max_scroll) as f32 / max_scroll as f32
            }
        });
        fill_rect(
            pixmap,
            Rect::new(layout.editor.right() - 8, layout.editor.y, 4, layout.editor.h),
            theme.surface_alt.with_alpha(150),
        );
        fill_rect(
            pixmap,
            Rect::new(
                layout.editor.right() - 8,
                layout.editor.y + ((track_height - thumb_height) * scroll_ratio) as i32,
                4,
                thumb_height as i32,
            ),
            theme.accent.with_alpha(190),
        );
        fill_rect(pixmap, layout.status_bar, theme.surface);
        let (location, right) = if let Some(snapshot) = snapshot {
            let position = snapshot.cursor.min(snapshot.text.len());
            let line = snapshot.text[..position]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            let start = snapshot.text[..position]
                .rfind('\n')
                .map_or(0, |index| index + 1);
            (
                format!(
                    "Ln {line}, Col {}  ·  {} words",
                    snapshot.text[start..position].chars().count() + 1,
                    snapshot.text.split_whitespace().count()
                ),
                format!(
                    "{}  ·  {}{}",
                    snapshot.encoding.label(),
                    snapshot.line_ending.label(),
                    if snapshot.dirty { "  ·  Modified" } else { "" }
                ),
            )
        } else {
            ("Ln 1, Col 1".into(), "UTF-8  ·  LF".into())
        };
        self.text(pixmap, 14, layout.status_bar.y + 18, &location, theme.text, 10.0);
        self.text(
            pixmap,
            layout.status_bar.right() - (right.chars().count() as i32 * 6 + 22),
            layout.status_bar.y + 18,
            &right,
            theme.muted_text,
            10.0,
        );
    }

    fn text(&self, pixmap: &mut Pixmap, x: i32, baseline: i32, text: &str, colour: Rgba, size: f32) {
        let Some(font) = self.font.as_ref() else {
            return;
        };
        let mut pen = x as f32;
        for character in text.chars() {
            let (metrics, bitmap) = font.rasterize(character, size);
            let top = baseline as f32 - metrics.ymin as f32 - metrics.height as f32;
            for y in 0..metrics.height {
                for xx in 0..metrics.width {
                    let coverage = bitmap[y * metrics.width + xx];
                    if coverage > 0 {
                        blend(
                            pixmap,
                            pen as i32 + metrics.xmin + xx as i32,
                            top as i32 + y as i32,
                            colour,
                            (u16::from(coverage) * u16::from(colour.a) / 255) as u8,
                        );
                    }
                }
            }
            pen += metrics.advance_width;
        }
    }
}

struct VisualLine<'a> {
    logical: usize,
    start_column: usize,
    text: &'a str,
}

fn make_visual_lines<'a>(
    lines: &[&'a str],
    max_chars: usize,
    wrap: bool,
) -> Vec<VisualLine<'a>> {
    let max_chars = max_chars.max(1);
    let mut visual = Vec::new();
    for (logical, line) in lines.iter().copied().enumerate() {
        let characters: Vec<(usize, char)> = line.char_indices().collect();
        if !wrap || characters.len() <= max_chars {
            visual.push(VisualLine {
                logical,
                start_column: 0,
                text: line,
            });
            continue;
        }
        let mut start = 0;
        while start < characters.len() {
            let end = (start + max_chars).min(characters.len());
            let byte_start = characters[start].0;
            let byte_end = if end < characters.len() {
                characters[end].0
            } else {
                line.len()
            };
            visual.push(VisualLine {
                logical,
                start_column: start,
                text: &line[byte_start..byte_end],
            });
            start = end;
        }
    }
    if visual.is_empty() {
        visual.push(VisualLine {
            logical: 0,
            start_column: 0,
            text: "",
        });
    }
    visual
}

fn load_font() -> Option<Font> {
    [
        std::env::var_os("NOTEPAD_PRO_FONT").map(std::path::PathBuf::from),
        Some(Path::new("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf").into()),
        Some(Path::new("C:\\Windows\\Fonts\\consola.ttf").into()),
        Some(Path::new("/System/Library/Fonts/Menlo.ttc").into()),
    ]
    .into_iter()
    .flatten()
    .find_map(|path| Font::from_bytes(std::fs::read(path).ok()?, FontSettings::default()).ok())
}

fn clip_text(text: &str, maximum: usize) -> String {
    let mut result: String = text.chars().take(maximum).collect();
    if text.chars().count() > maximum {
        result.push('…');
    }
    result
}

fn line_colour(line_colour: LineColour) -> Rgba {
    let value = line_colour.rgb().unwrap_or(0);
    Rgba::rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn input_box(pixmap: &mut Pixmap, rect: Rect, theme: Theme, focused: bool) {
    fill_rect(pixmap, rect, theme.editor_bg);
    fill_rect(
        pixmap,
        Rect::new(rect.x, rect.bottom() - if focused { 2 } else { 1 }, rect.w, if focused { 2 } else { 1 }),
        if focused { theme.accent } else { theme.border },
    );
}

fn small_button(pixmap: &mut Pixmap, rect: Rect, theme: Theme, hover: bool) {
    fill_rect(
        pixmap,
        rect,
        if hover {
            theme.accent.with_alpha(48)
        } else {
            theme.surface.with_alpha(210)
        },
    );
    fill_rect(
        pixmap,
        Rect::new(rect.x, rect.bottom() - 1, rect.w, 1),
        theme.border,
    );
}

fn checkbox(pixmap: &mut Pixmap, rect: Rect, theme: Theme, checked: bool) {
    fill_rect(pixmap, rect, theme.editor_bg);
    fill_rect(
        pixmap,
        Rect::new(rect.x, rect.y, rect.w, 1),
        if checked { theme.accent } else { theme.border },
    );
    fill_rect(
        pixmap,
        Rect::new(rect.x, rect.bottom() - 1, rect.w, 1),
        if checked { theme.accent } else { theme.border },
    );
    fill_rect(
        pixmap,
        Rect::new(rect.x, rect.y, 1, rect.h),
        if checked { theme.accent } else { theme.border },
    );
    fill_rect(
        pixmap,
        Rect::new(rect.right() - 1, rect.y, 1, rect.h),
        if checked { theme.accent } else { theme.border },
    );
    if checked {
        fill_rect(
            pixmap,
            Rect::new(rect.x + 5, rect.y + 5, rect.w - 10, rect.h - 10),
            theme.accent,
        );
    }
}

fn draw_logo(pixmap: &mut Pixmap, rect: Rect, accent: Rgba, page: Rgba) {
    fill_rect(pixmap, rect, accent);
    fill_rect(
        pixmap,
        Rect::new(rect.x + 4, rect.y + 3, rect.w - 8, rect.h - 6),
        page,
    );
    fill_rect(
        pixmap,
        Rect::new(rect.x + 7, rect.y + 7, rect.w - 12, 2),
        accent,
    );
    fill_rect(
        pixmap,
        Rect::new(rect.x + 7, rect.y + 12, rect.w - 10, 2),
        accent.with_alpha(190),
    );
    fill_rect(
        pixmap,
        Rect::new(rect.right() - 6, rect.y, 6, 6),
        accent.with_alpha(210),
    );
}

fn fill_rect(pixmap: &mut Pixmap, rect: Rect, colour: Rgba) {
    if rect.w <= 0 || rect.h <= 0 {
        return;
    }
    let mut builder = PathBuilder::new();
    builder.move_to(rect.x as f32, rect.y as f32);
    builder.line_to(rect.right() as f32, rect.y as f32);
    builder.line_to(rect.right() as f32, rect.bottom() as f32);
    builder.line_to(rect.x as f32, rect.bottom() as f32);
    builder.close();
    if let Some(path) = builder.finish() {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(colour.r, colour.g, colour.b, colour.a));
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn fill_circle(pixmap: &mut Pixmap, x: f32, y: f32, radius: f32, colour: Rgba) {
    let mut builder = PathBuilder::new();
    builder.push_circle(x, y, radius);
    if let Some(path) = builder.finish() {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(colour.r, colour.g, colour.b, colour.a));
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn blend(pixmap: &mut Pixmap, x: i32, y: i32, colour: Rgba, alpha: u8) {
    if x < 0 || y < 0 || x >= pixmap.width() as i32 || y >= pixmap.height() as i32 {
        return;
    }
    let index = (y as usize * pixmap.width() as usize + x as usize) * 4;
    let data = pixmap.data_mut();
    let inverse = 255 - u16::from(alpha);
    data[index] = ((u16::from(colour.r) * u16::from(alpha)
        + u16::from(data[index]) * inverse)
        / 255) as u8;
    data[index + 1] = ((u16::from(colour.g) * u16::from(alpha)
        + u16::from(data[index + 1]) * inverse)
        / 255) as u8;
    data[index + 2] = ((u16::from(colour.b) * u16::from(alpha)
        + u16::from(data[index + 2]) * inverse)
        / 255) as u8;
    data[index + 3] = alpha.max(data[index + 3]);
}
