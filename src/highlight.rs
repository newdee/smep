//! Markdown syntax highlighting for the editor.
//!
//! The same `markdown` crate that renders the preview parses the buffer here,
//! so what the editor colours and what the preview shows never disagree.
//! Every node with a look becomes a span; where nodes nest, the innermost
//! wins (bold inside a heading is bold). The editor asks for non-overlapping
//! runs, which [`runs`] produces from the spans.
//!
//! The parse is a whole-document one and grows faster than the document
//! (measured: 120 KB in 65 ms, 480 KB in 570 ms, 1 MB in 7 s), so it must
//! never sit between a keystroke and its frame. Small documents are parsed
//! in place; larger ones are parsed on a background thread after typing
//! pauses, and until that lands the runs already on screen follow the edit
//! (a run that holds the edit stretches with it, runs after it move).

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::input::{
    EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
    InputHighlighterFactory, Rope,
};
use gpui_kit::{AppContext as _, AsyncApp, Context, HighlightStyle, SharedString, Task, Window};
use markdown::ParseOptions;
use markdown::mdast::Node;

/// A byte range and the theme name for its look.
pub type Span = (Range<usize>, &'static str);

/// The name the editor recognises as Markdown.
pub const LANGUAGE: &str = "markdown";

/// Documents up to this size are parsed on the spot; the parse of one takes
/// a few milliseconds at most, and the highlighting never lags the text.
pub const SYNC_LIMIT: usize = 32 * 1024;

/// How long typing has to pause before a large document is re-parsed.
pub const DEBOUNCE: Duration = Duration::from_millis(120);

/// A factory the editor calls with its language; only Markdown gets a highlighter.
pub fn factory() -> InputHighlighterFactory {
    Rc::new(|language| {
        (language == LANGUAGE)
            .then(|| Box::new(MarkdownHighlighter::default()) as Box<dyn InputHighlighter>)
    })
}

#[derive(Default)]
pub struct MarkdownHighlighter {
    /// Shared with the background parse, which replaces it when done.
    runs: Rc<RefCell<Vec<Span>>>,
    /// The parse in flight for a large document; replaced (and so cancelled)
    /// by every further edit.
    pending: Option<Task<()>>,
}

impl InputHighlighter for MarkdownHighlighter {
    fn language(&self) -> SharedString {
        LANGUAGE.into()
    }

    fn update(
        &mut self,
        edit: Option<InputEdit>,
        text: &Rope,
        _folding: bool,
        _window: &mut Window,
        cx: &mut Context<EditorState>,
    ) {
        if text.len() <= SYNC_LIMIT {
            self.pending = None;
            *self.runs.borrow_mut() = runs(spans(&text.to_string()));
            return;
        }

        match edit {
            Some(edit) => shift(&mut self.runs.borrow_mut(), &edit),
            // A whole new text: nothing on screen is worth keeping.
            None => self.runs.borrow_mut().clear(),
        }

        let text = text.clone();
        let runs = self.runs.clone();
        self.pending = Some(cx.spawn(async move |editor, cx: &mut AsyncApp| {
            cx.background_executor().timer(DEBOUNCE).await;
            let parsed = cx
                .background_spawn(async move { self::runs(spans(&text.to_string())) })
                .await;
            // Not cancelled, so no edit came in between: these runs are for
            // the text as it is now.
            *runs.borrow_mut() = parsed;
            let _ = editor.update(cx, |_, cx| cx.notify());
        }));
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let runs = self.runs.borrow();
        let mut out = Vec::new();
        let mut pos = range.start;
        let first = runs.partition_point(|(run, _)| run.end <= range.start);
        for (run, name) in &runs[first..] {
            if run.start >= range.end {
                break;
            }
            let start = run.start.max(range.start);
            let end = run.end.min(range.end);
            if start > pos {
                out.push((pos..start, HighlightStyle::default()));
            }
            out.push((start..end, resolver.style(name).unwrap_or_default()));
            pos = end;
        }
        if pos < range.end {
            out.push((pos..range.end, HighlightStyle::default()));
        }
        out
    }

    fn fold_ranges(&self, _text: &Rope) -> Vec<FoldRange> {
        Vec::new()
    }
}

/// Move the runs along with an edit, so they keep pointing at the text
/// they coloured until the next parse: runs before the edit stay, runs
/// after it move by the edit's growth, a run holding the whole edit grows
/// with it, and a run the edit cuts into is clipped to the part it kept.
pub fn shift(runs: &mut Vec<Span>, edit: &InputEdit) {
    let (start, old_end, new_end) = (edit.start_byte, edit.old_end_byte, edit.new_end_byte);
    let moved = |offset: usize| offset + new_end - old_end;
    runs.retain_mut(|(run, _)| {
        if run.end <= start {
            true
        } else if run.start >= old_end {
            *run = moved(run.start)..moved(run.end);
            true
        } else if run.start <= start && run.end >= old_end {
            run.end = moved(run.end);
            run.start < run.end
        } else if run.start < start {
            run.end = start;
            true
        } else {
            *run = new_end..moved(run.end.max(old_end));
            run.start < run.end
        }
    });
}

/// Theme names for the nodes that get a look. `parent` is the enclosing
/// node's name, so link text can differ from the link's URL.
fn name_for(node: &Node, parent: Option<&'static str>) -> Option<&'static str> {
    Some(match node {
        Node::Heading(_) => "title",
        Node::Strong(_) => "emphasis.strong",
        Node::Emphasis(_) => "emphasis",
        Node::Delete(_) | Node::Html(_) => "comment",
        Node::InlineCode(_) | Node::Code(_) | Node::InlineMath(_) | Node::Math(_) => "string",
        Node::Link(_)
        | Node::Image(_)
        | Node::LinkReference(_)
        | Node::ImageReference(_)
        | Node::Definition(_)
        | Node::FootnoteReference(_)
        | Node::FootnoteDefinition(_) => "link_uri",
        Node::Text(_) if parent == Some("link_uri") => "link_text",
        Node::ThematicBreak(_) => "punctuation",
        _ => return None,
    })
}

/// The 0-based start line of every top-level block, in document order.
///
/// The preview renders one list item per top-level block, so this maps an
/// editor line to the preview item that holds it.
pub fn block_start_lines(text: &str) -> Vec<usize> {
    let Ok(root) = markdown::to_mdast(text, &ParseOptions::gfm()) else {
        return Vec::new();
    };
    root.children()
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| block.position().map(|p| p.start.line - 1))
                .collect()
        })
        .unwrap_or_default()
}

/// The byte range of every top-level block, in document order. Blank lines
/// between blocks belong to no block, and no block ends in a line break
/// (the parser lets a list keep the blank line after its nested list).
pub fn block_ranges(text: &str) -> Vec<Range<usize>> {
    let Ok(root) = markdown::to_mdast(text, &ParseOptions::gfm()) else {
        return Vec::new();
    };
    root.children()
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| block.position().map(|p| p.start.offset..p.end.offset))
                .map(|range| {
                    let trimmed = text[range.clone()].trim_end_matches(['\r', '\n']);
                    range.start..range.start + trimmed.len()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Every styled node in `text`, outer nodes before the nodes inside them.
pub fn spans(text: &str) -> Vec<Span> {
    let mut out = Vec::new();
    if let Ok(root) = markdown::to_mdast(text, &ParseOptions::gfm()) {
        walk(&root, None, &mut out);
    }
    out
}

fn walk(node: &Node, parent: Option<&'static str>, out: &mut Vec<Span>) {
    let name = name_for(node, parent);
    if let (Some(name), Some(position)) = (name, node.position()) {
        let range = position.start.offset..position.end.offset;
        if !range.is_empty() {
            out.push((range, name));
        }
    }
    if let Some(children) = node.children() {
        for child in children {
            walk(child, name.or(parent), out);
        }
    }
}

/// Flatten nested spans into sorted, non-overlapping runs; the innermost
/// span wins wherever spans nest. Assumes spans nest properly, as tree nodes do.
pub fn runs(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));

    let mut out: Vec<Span> = Vec::new();
    let mut stack: Vec<Span> = Vec::new();
    let mut pos = 0;
    let emit = |range: Range<usize>, name: &'static str, out: &mut Vec<Span>| {
        if !range.is_empty() {
            out.push((range, name));
        }
    };

    for span in spans {
        while let Some((top, name)) = stack.last() {
            if top.end <= span.0.start {
                emit(pos.max(top.start)..top.end, name, &mut out);
                pos = pos.max(top.end);
                stack.pop();
            } else {
                break;
            }
        }
        if let Some((top, name)) = stack.last() {
            emit(pos.max(top.start)..span.0.start, name, &mut out);
        }
        pos = pos.max(span.0.start);
        stack.push(span);
    }
    while let Some((top, name)) = stack.pop() {
        emit(pos.max(top.start)..top.end, name, &mut out);
        pos = pos.max(top.end);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &str) -> Vec<(Range<usize>, &'static str)> {
        runs(spans(text))
    }

    #[test]
    fn bold_inside_a_heading_wins_over_the_heading() {
        assert_eq!(
            names("# Hi **there**\n"),
            vec![(0..5, "title"), (5..14, "emphasis.strong")]
        );
    }

    #[test]
    fn link_text_differs_from_the_link_itself() {
        assert_eq!(
            names("[a](b)"),
            vec![(0..1, "link_uri"), (1..2, "link_text"), (2..6, "link_uri")]
        );
    }

    #[test]
    fn offsets_are_bytes_so_crlf_and_cjk_line_up() {
        let text = "# 标题\r\n**粗**";
        let runs = names(text);
        assert_eq!(runs[0], (0..8, "title"));
        assert_eq!(&text[0..8], "# 标题");
        assert_eq!(runs[1], (10..17, "emphasis.strong"));
        assert_eq!(&text[10..17], "**粗**");
    }

    #[test]
    fn runs_are_sorted_and_never_overlap() {
        let text = "# T *a **b** c*\n\n> q `x` [l](u) ~~d~~\n\n```rs\nfn main() {}\n```\n\n---\n";
        let runs = names(text);
        assert!(!runs.is_empty());
        for pair in runs.windows(2) {
            assert!(pair[0].0.end <= pair[1].0.start, "{pair:?}");
        }
        assert!(runs.iter().all(|(r, _)| !r.is_empty()));
    }

    #[test]
    fn block_start_lines_are_zero_based_and_skip_blank_lines() {
        assert_eq!(
            block_start_lines("# A\n\npara\nmore\n\n- x\n- y\n\n```\ncode\n```\n"),
            vec![0, 2, 5, 8]
        );
        assert_eq!(block_start_lines("a\r\n\r\nb"), vec![0, 2]);
        assert!(block_start_lines("").is_empty());
    }

    #[test]
    fn block_ranges_cover_each_block_without_the_gaps() {
        let text = "# A\n\npara\nmore\n\n- x\n- y\n";
        assert_eq!(block_ranges(text), vec![0..3, 5..14, 16..23]);
        assert_eq!(&text[0..3], "# A");
        assert_eq!(&text[5..14], "para\nmore");
        assert_eq!(&text[16..23], "- x\n- y");
        assert!(block_ranges("").is_empty());
        assert_eq!(block_ranges("a\r\n\r\nb"), vec![0..1, 5..6]);
    }

    #[test]
    fn a_list_with_a_nested_list_does_not_keep_the_blank_line_after_it() {
        let text = "- a\n  - b\n\n> q\n";
        let ranges = block_ranges(text);
        assert_eq!(&text[ranges[0].clone()], "- a\n  - b");
        assert_eq!(&text[ranges[1].clone()], "> q");
        let text = "- a\r\n  - b\r\n\r\n> q\r\n";
        assert_eq!(&text[block_ranges(text)[0].clone()], "- a\r\n  - b");
    }

    fn edit(start: usize, old_end: usize, new_end: usize) -> InputEdit {
        let at = |_: usize| gpui_kit::component::input::Point::new(0, 0);
        InputEdit {
            start_byte: start,
            old_end_byte: old_end,
            new_end_byte: new_end,
            start_position: at(start),
            old_end_position: at(old_end),
            new_end_position: at(new_end),
        }
    }

    #[test]
    fn runs_follow_an_edit_until_the_next_parse() {
        // "# Title" then "**b**": typing two chars inside the heading.
        let mut runs = vec![(0..7, "title"), (9..14, "emphasis.strong")];
        shift(&mut runs, &edit(3, 3, 5));
        assert_eq!(runs, vec![(0..9, "title"), (11..16, "emphasis.strong")]);

        // Deleting the heading's tail and into the gap clips the heading.
        let mut runs = vec![(0..7, "title"), (9..14, "emphasis.strong")];
        shift(&mut runs, &edit(5, 8, 5));
        assert_eq!(runs, vec![(0..5, "title"), (6..11, "emphasis.strong")]);

        // Deleting from the gap into the bold keeps what is left of it.
        let mut runs = vec![(0..7, "title"), (9..14, "emphasis.strong")];
        shift(&mut runs, &edit(8, 11, 8));
        assert_eq!(runs, vec![(0..7, "title"), (8..11, "emphasis.strong")]);

        // A run replaced entirely disappears; a run holding a deletion shrinks.
        let mut runs = vec![(0..7, "title"), (9..14, "emphasis.strong")];
        shift(&mut runs, &edit(9, 14, 9));
        assert_eq!(runs, vec![(0..7, "title")]);
        let mut runs = vec![(0..7, "title")];
        shift(&mut runs, &edit(2, 4, 2));
        assert_eq!(runs, vec![(0..5, "title")]);
        shift(&mut runs, &edit(0, 5, 0));
        assert!(runs.is_empty());
    }

    #[test]
    fn plain_text_and_empty_input_have_no_runs() {
        assert!(names("").is_empty());
        assert!(names("just words\n\nmore words").is_empty());
    }

    fn with_runs(runs: Vec<Span>) -> MarkdownHighlighter {
        MarkdownHighlighter {
            runs: Rc::new(RefCell::new(runs)),
            pending: None,
        }
    }

    struct Named;
    impl HighlightStyleResolver for Named {
        fn style(&self, name: &str) -> Option<HighlightStyle> {
            (name == "title").then(|| HighlightStyle {
                fade_out: Some(0.5),
                ..Default::default()
            })
        }
    }

    #[test]
    fn styles_cover_the_asked_range_exactly_with_defaults_in_the_gaps() {
        let highlighter = with_runs(names("# A\n\ntext\n\n# B\n"));
        // Ask across the gap between the two headings, cut mid-heading.
        let styles = highlighter.styles(&(1..12), &Named);
        let mut pos = 1;
        for (range, _) in &styles {
            assert_eq!(range.start, pos, "{styles:?}");
            pos = range.end;
        }
        assert_eq!(pos, 12);
        assert_eq!(styles[0].1.fade_out, Some(0.5), "inside the first heading");
        assert_eq!(styles[1].1, HighlightStyle::default(), "the paragraph");
        assert_eq!(
            styles.last().unwrap().1.fade_out,
            Some(0.5),
            "into the second"
        );
    }

    #[test]
    fn styles_outside_every_run_are_one_default_run() {
        let highlighter = with_runs(names("# A\n\ntext\n"));
        assert_eq!(
            highlighter.styles(&(5..9), &Named),
            vec![(5..9, HighlightStyle::default())]
        );
    }
}
