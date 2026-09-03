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
}

pub fn buttons(layout: Layout) -> Vec<(Button, ToolbarAction)> {
    let mut buttons = Vec::new();
    let mut x = 10;
    let y = layout.toolbar.y + 8;
    let items = [
        ("New", ToolbarAction::New, 54),
        ("Open", ToolbarAction::Open, 58),
        ("Save", ToolbarAction::Save, 58),
        ("Save as", ToolbarAction::SaveAs, 68),
        ("Undo", ToolbarAction::Undo, 58),
        ("Redo", ToolbarAction::Redo, 58),
        ("Find", ToolbarAction::Find, 58),
        ("Replace", ToolbarAction::Replace, 72),
        ("Extract", ToolbarAction::Extract, 70),
        ("Notes", ToolbarAction::Notes, 62),
        ("Wrap", ToolbarAction::Wrap, 58),
        ("A−", ToolbarAction::ZoomOut, 40),
        ("A+", ToolbarAction::ZoomIn, 40),
        ("Theme", ToolbarAction::Theme, 64),
        ("•", ToolbarAction::BulletList, 32),
        ("1.", ToolbarAction::NumberedList, 32),
        ("☐", ToolbarAction::Checklist, 32),
        ("⇤", ToolbarAction::Outdent, 32),
        ("⇥", ToolbarAction::Indent, 32),
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
    buttons
}
