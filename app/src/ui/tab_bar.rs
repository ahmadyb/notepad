use crate::layout::{Layout, Rect};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabView {
    pub title: String,
    pub dirty: bool,
    pub rect: Rect,
}

pub fn layout_tabs(layout: Layout, tabs: &[(String, bool)]) -> Vec<TabView> {
    let available = (layout.tab_bar.w - 60).max(0);
    let width = if tabs.is_empty() {
        0
    } else {
        ((available - 5 * tabs.len().saturating_sub(1) as i32) / tabs.len() as i32).clamp(94, 220)
    };
    let mut x = 10;
    tabs.iter()
        .map(|(title, dirty)| {
            let view = TabView {
                title: title.clone(),
                dirty: *dirty,
                rect: Rect::new(x, layout.tab_bar.y + 5, width, layout.tab_bar.h - 10),
            };
            x += width + 5;
            view
        })
        .collect()
}
