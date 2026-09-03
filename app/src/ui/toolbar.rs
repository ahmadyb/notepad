use crate::layout::{Layout, Rect};
use crate::ui::widgets::Button;
use notepad_core::LineColour;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    New,
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    Find,
    Replace,
    Extract,
    Notes,
    Wrap,
    ZoomOut,
    ZoomIn,
    Theme,
    BulletList,
    NumberedList,
    Checklist,
    Outdent,
    Indent,
    Highlight(LineColour),
    ClearHighlight,
}

pub fn buttons(layout: Layout) -> Vec<(Button, ToolbarAction)> {
    let mut buttons = Vec::new();
    let mut x = 8;
    let y = layout.toolbar.y + 8;
    let items = [
        ("New", ToolbarAction::New, 46),
        ("Open", ToolbarAction::Open, 48),
        ("Save", ToolbarAction::Save, 48),
        ("Save as", ToolbarAction::SaveAs, 58),
        ("Undo", ToolbarAction::Undo, 46),
        ("Redo", ToolbarAction::Redo, 46),
        ("Find", ToolbarAction::Find, 46),
        ("Replace", ToolbarAction::Replace, 62),
        ("Extract", ToolbarAction::Extract, 60),
        ("Notes", ToolbarAction::Notes, 52),
        ("Wrap", ToolbarAction::Wrap, 50),
        ("A−", ToolbarAction::ZoomOut, 34),
        ("A+", ToolbarAction::ZoomIn, 34),
        ("Theme", ToolbarAction::Theme, 52),
        ("•", ToolbarAction::BulletList, 28),
        ("1.", ToolbarAction::NumberedList, 28),
        ("☐", ToolbarAction::Checklist, 28),
        ("⇤", ToolbarAction::Outdent, 28),
        ("⇥", ToolbarAction::Indent, 28),
    ];
    for (label, action, width) in items {
        if x + width > layout.toolbar.right() - 8 {
            break;
        }
        buttons.push((Button::new(label, Rect::new(x, y, width, 26)), action));
        x += width + 5;
    }
    for colour in LineColour::BUILT_INS {
        if x + 25 > layout.toolbar.right() - 8 {
            break;
        }
        buttons.push((
            Button::new("", Rect::new(x, y + 3, 22, 20)),
            ToolbarAction::Highlight(colour),
        ));
        x += 27;
    }
    if x + 50 <= layout.toolbar.right() - 8 {
        buttons.push((
            Button::new("Clear", Rect::new(x, y, 50, 26)),
            ToolbarAction::ClearHighlight,
        ));
    }
    buttons
}
