#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(self) -> i32 {
        self.y + self.h
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn inset(self, amount: i32) -> Self {
        Self::new(
            self.x + amount,
            self.y + amount,
            (self.w - amount * 2).max(0),
            (self.h - amount * 2).max(0),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub titlebar: Rect,
    pub tab_bar: Rect,
    pub toolbar: Rect,
    pub find_bar: Rect,
    pub status_bar: Rect,
    pub sidebar: Rect,
    pub editor: Rect,
    pub extract_panel: Rect,
    pub sidebar_open: bool,
    pub find_open: bool,
    pub replace_open: bool,
    pub extract_open: bool,
}

impl Layout {
    pub const TITLEBAR_HEIGHT: i32 = 32;
    pub const TAB_HEIGHT: i32 = 38;
    pub const TOOLBAR_HEIGHT: i32 = 42;
    pub const FIND_HEIGHT: i32 = 42;
    pub const REPLACE_FIND_HEIGHT: i32 = 78;
    pub const STATUS_HEIGHT: i32 = 26;
    pub const SIDEBAR_WIDTH: i32 = 276;
    pub const EXTRACT_WIDTH: i32 = 320;

    /// Compatibility entry point used by the small layout tests and by callers
    /// that do not need the optional replace/extraction panels.
    pub fn compute(width: u32, height: u32, sidebar_open: bool, find_open: bool) -> Self {
        Self::compute_with_options(width, height, sidebar_open, find_open, false, false)
    }

    pub fn compute_with_options(
        width: u32,
        height: u32,
        sidebar_open: bool,
        find_open: bool,
        replace_open: bool,
        extract_open: bool,
    ) -> Self {
        let width = width.min(i32::MAX as u32) as i32;
        let height = height.min(i32::MAX as u32) as i32;
        let titlebar = Rect::new(0, 0, width, Self::TITLEBAR_HEIGHT);
        let tab_bar = Rect::new(0, titlebar.bottom(), width, Self::TAB_HEIGHT);
        let toolbar = Rect::new(0, tab_bar.bottom(), width, Self::TOOLBAR_HEIGHT);
        let find_height = if find_open {
            if replace_open {
                Self::REPLACE_FIND_HEIGHT
            } else {
                Self::FIND_HEIGHT
            }
        } else {
            0
        };
        let find_bar = Rect::new(0, toolbar.bottom(), width, find_height);
        let status_bar = Rect::new(
            0,
            height.saturating_sub(Self::STATUS_HEIGHT),
            width,
            Self::STATUS_HEIGHT,
        );
        let top = find_bar.bottom();
        let available_height = (status_bar.y - top).max(0);
        let sidebar_width = if sidebar_open {
            Self::SIDEBAR_WIDTH.min((width / 3).max(0))
        } else {
            0
        };
        let extract_width = if extract_open {
            Self::EXTRACT_WIDTH.min((width / 2).max(0))
        } else {
            0
        };
        let sidebar = Rect::new(0, top, sidebar_width, available_height);
        let extract_panel = Rect::new(
            (width - extract_width).max(sidebar_width),
            top,
            extract_width,
            available_height,
        );
        let editor_width = (width - sidebar_width - extract_width).max(0);
        let editor = Rect::new(sidebar_width, top, editor_width, available_height);
        Self {
            titlebar,
            tab_bar,
            toolbar,
            find_bar,
            status_bar,
            sidebar,
            editor,
            extract_panel,
            sidebar_open,
            find_open,
            replace_open,
            extract_open,
        }
    }

    pub fn titlebar_button_rects(self) -> (Rect, Rect, Rect) {
        let close = Rect::new(self.titlebar.right() - 46, 0, 46, Self::TITLEBAR_HEIGHT);
        let maximise = Rect::new(close.x - 46, 0, 46, Self::TITLEBAR_HEIGHT);
        let minimise = Rect::new(maximise.x - 46, 0, 46, Self::TITLEBAR_HEIGHT);
        (minimise, maximise, close)
    }

    pub fn tab_plus_rect(self) -> Rect {
        Rect::new(self.tab_bar.right() - 48, self.tab_bar.y, 48, self.tab_bar.h)
    }

    pub fn tab_close_rect(self, tab: usize, count: usize) -> Rect {
        if count == 0 {
            return Rect::default();
        }
        let tab_width = ((self.tab_bar.w - 48).max(0) / count as i32).clamp(116, 230);
        let x = self.tab_bar.x + tab as i32 * tab_width;
        Rect::new(x + tab_width - 30, self.tab_bar.y + 7, 24, 24)
    }

    pub fn line_number_width(line_count: usize, char_width: i32) -> i32 {
        ((line_count.max(1).to_string().len() as i32) + 2) * char_width.max(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    None,
    Drag,
    Minimize,
    Maximize,
    Close,
    Editor,
    Sidebar,
    Toolbar,
    FindBar,
    ExtractPanel,
    TabBar,
}

pub fn hit_test(layout: Layout, x: i32, y: i32) -> HitTarget {
    let (minimise, maximise, close) = layout.titlebar_button_rects();
    if minimise.contains(x, y) {
        HitTarget::Minimize
    } else if maximise.contains(x, y) {
        HitTarget::Maximize
    } else if close.contains(x, y) {
        HitTarget::Close
    } else if layout.tab_bar.contains(x, y) {
        HitTarget::TabBar
    } else if layout.extract_panel.contains(x, y) {
        HitTarget::ExtractPanel
    } else if layout.editor.contains(x, y) {
        HitTarget::Editor
    } else if layout.sidebar.contains(x, y) {
        HitTarget::Sidebar
    } else if layout.find_bar.contains(x, y) {
        HitTarget::FindBar
    } else if layout.toolbar.contains(x, y) {
        HitTarget::Toolbar
    } else if layout.titlebar.contains(x, y) {
        HitTarget::Drag
    } else {
        HitTarget::None
    }
}
