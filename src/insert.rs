//! Block snippets offered by the insert menu on an empty line.
//!
//! The menu is the Feishu-style "+" on an empty line (or `/` typed on one).
//! Choosing an entry replaces that line with a Markdown skeleton and places
//! the cursor where typing continues.

use std::ops::Range;

// The helpers work on any multi-line input: the source editor and the
// rendered view's block editors are different kinds of input.
use gpui_kit::base::input::{InputBaseState, MultiLineMode, RopeExt as _};
use gpui_kit::{Context, Window};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Heading1,
    Heading2,
    Heading3,
    BulletList,
    NumberedList,
    TaskList,
    Quote,
    CodeBlock,
    Table,
    Divider,
    Image,
    Link,
}

/// The menu, top to bottom. `None` is a separator.
pub const MENU: &[Option<BlockKind>] = &[
    Some(BlockKind::Heading1),
    Some(BlockKind::Heading2),
    Some(BlockKind::Heading3),
    None,
    Some(BlockKind::BulletList),
    Some(BlockKind::NumberedList),
    Some(BlockKind::TaskList),
    None,
    Some(BlockKind::Quote),
    Some(BlockKind::CodeBlock),
    Some(BlockKind::Table),
    Some(BlockKind::Divider),
    None,
    Some(BlockKind::Image),
    Some(BlockKind::Link),
];

impl BlockKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Heading1 => "Heading 1",
            Self::Heading2 => "Heading 2",
            Self::Heading3 => "Heading 3",
            Self::BulletList => "Bullet list",
            Self::NumberedList => "Numbered list",
            Self::TaskList => "Task list",
            Self::Quote => "Quote",
            Self::CodeBlock => "Code block",
            Self::Table => "Table",
            Self::Divider => "Divider",
            Self::Image => "Image",
            Self::Link => "Link",
        }
    }

    /// The text that replaces the line, and the byte offset inside it where
    /// the cursor goes.
    pub fn snippet(self) -> (&'static str, usize) {
        match self {
            Self::Heading1 => ("# ", 2),
            Self::Heading2 => ("## ", 3),
            Self::Heading3 => ("### ", 4),
            Self::BulletList => ("- ", 2),
            Self::NumberedList => ("1. ", 3),
            Self::TaskList => ("- [ ] ", 6),
            Self::Quote => ("> ", 2),
            Self::CodeBlock => ("```\n\n```", 4),
            Self::Table => ("| Column | Column |\n| --- | --- |\n|  |  |", 2),
            Self::Divider => ("---\n", 4),
            Self::Image => ("![](", 2),
            Self::Link => ("[](", 1),
        }
    }
}

/// Byte range of the cursor's line, without its line break (`\r\n` included).
pub fn cursor_line_range<M: MultiLineMode>(state: &InputBaseState<M>) -> Range<usize> {
    let row = state.cursor_position().line as usize;
    let text = state.text();
    let start = text.line_start_offset(row);
    let mut end = text.line_end_offset(row);
    if end > start && text.char_at(end - 1) == Some('\r') {
        end -= 1;
    }
    start..end
}

/// What the cursor's line holds, with surrounding whitespace removed.
pub fn cursor_line_trimmed<M: MultiLineMode>(state: &InputBaseState<M>) -> String {
    let range = cursor_line_range(state);
    state.text().slice(range).to_string().trim().to_string()
}

/// Put the block's skeleton at the cursor's line and place the cursor.
///
/// A blank line (or the `/` that opened the menu) is replaced. A line with
/// content keeps it: the skeleton goes on a new line below, with the
/// document's own line ending.
pub fn insert_block<M: MultiLineMode>(
    state: &mut InputBaseState<M>,
    kind: BlockKind,
    window: &mut Window,
    cx: &mut Context<InputBaseState<M>>,
) {
    let line = cursor_line_range(state);
    let (snippet, cursor) = kind.snippet();
    let trimmed = cursor_line_trimmed(state);

    let (target, text, cursor) = if trimmed.is_empty() || trimmed == "/" {
        let cursor = line.start + cursor;
        (line, snippet.to_string(), cursor)
    } else {
        let newline = if state.text().char_at(line.end) == Some('\r') {
            "\r\n"
        } else {
            "\n"
        };
        let text = format!("{newline}{snippet}");
        let cursor = line.end + newline.len() + cursor;
        (line.end..line.end, text, cursor)
    };

    state.set_selected_range(target, cx);
    state.replace(text, window, cx);
    state.set_selected_range(cursor..cursor, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_snippet_cursor_sits_on_a_char_boundary_inside_the_snippet() {
        for kind in MENU.iter().flatten() {
            let (snippet, cursor) = kind.snippet();
            assert!(cursor <= snippet.len(), "{kind:?}");
            assert!(snippet.is_char_boundary(cursor), "{kind:?}");
        }
    }

    #[test]
    fn the_menu_has_no_leading_trailing_or_doubled_separators() {
        assert!(MENU.first().unwrap().is_some());
        assert!(MENU.last().unwrap().is_some());
        assert!(!MENU.windows(2).any(|w| w[0].is_none() && w[1].is_none()));
    }
}
