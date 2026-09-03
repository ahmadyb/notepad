use crate::types::{LineMetadata, ListType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPrefix {
    pub list_type: ListType,
    pub indent: u8,
    pub marker_len: usize,
    pub number: Option<u32>,
    pub checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnterResult {
    pub next_metadata: LineMetadata,
    pub exits_list: bool,
    pub next_prefix: String,
}

pub fn parse_list_prefix(line: &str, tab_width: usize) -> Option<ListPrefix> {
    let tab_width = tab_width.max(1);
    let mut bytes = 0;
    let mut columns = 0;
    for character in line.chars() {
        match character {
            ' ' => {
                bytes += 1;
                columns += 1;
            }
            '\t' => {
                bytes += 1;
                columns += tab_width;
            }
            _ => break,
        }
    }
    let indent = (columns / tab_width).min(5) as u8;
    let rest = &line[bytes..];
    let (list_type, marker_len, number, checked) = if rest.starts_with("[] ") {
        (ListType::Check, 3, None, false)
    } else if rest.starts_with("[x] ") || rest.starts_with("[X] ") {
        (ListType::Check, 4, None, true)
    } else if rest.starts_with("- ") || rest.starts_with("* ") {
        (ListType::Bullet, 2, None, false)
    } else {
        let dot = rest.find(". ")?;
        if dot == 0 || !rest[..dot].chars().all(|character| character.is_ascii_digit()) {
            return None;
        }
        (
            ListType::Number,
            dot + 2,
            Some(rest[..dot].parse().ok()?),
            false,
        )
    };
    Some(ListPrefix {
        list_type,
        indent,
        marker_len,
        number,
        checked,
    })
}

pub fn format_number_prefix(number: u32, indent: u8) -> String {
    format!("{}{}. ", "    ".repeat(indent as usize), number)
}

pub fn format_list_prefix(metadata: &LineMetadata, number: u32) -> String {
    let indent = "    ".repeat(metadata.indent as usize);
    match metadata.list_type {
        ListType::None => String::new(),
        ListType::Bullet => format!("{indent}- "),
        ListType::Number => format!("{indent}{number}. "),
        ListType::Check => {
            if metadata.checked {
                format!("{indent}[x] ")
            } else {
                format!("{indent}[] ")
            }
        }
    }
}

pub fn handle_enter(line: &str, metadata: &LineMetadata) -> EnterResult {
    if !metadata.is_list() {
        return EnterResult {
            next_metadata: LineMetadata::default(),
            exits_list: false,
            next_prefix: String::new(),
        };
    }
    if line.trim().is_empty() && metadata.indent == 0 {
        return EnterResult {
            next_metadata: LineMetadata::default(),
            exits_list: true,
            next_prefix: String::new(),
        };
    }
    let mut next = metadata.clone();
    if line.trim().is_empty() {
        next.indent = next.indent.saturating_sub(1);
    }
    EnterResult {
        next_prefix: format_list_prefix(&next, 1),
        next_metadata: next,
        exits_list: false,
    }
}

pub fn adjust_indent(metadata: &mut LineMetadata, increase: bool) {
    if increase {
        metadata.indent = (metadata.indent + 1).min(LineMetadata::MAX_INDENT);
    } else {
        metadata.indent = metadata.indent.saturating_sub(1);
    }
}
