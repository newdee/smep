//! Markdown syntax highlighting for the editor.
//!
//! The same `markdown` crate that renders the preview parses the buffer here,
//! so what the editor colours and what the preview shows never disagree.
//! Every node with a look becomes a span; where nodes nest, the innermost
//! wins (bold inside a heading is bold). The editor asks for non-overlapping
//! runs, which [`runs`] produces from the spans.

use std::ops::Range;
use std::rc::Rc;

use gpui_kit::component::input::{
    EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
    InputHighlighterFactory, Rope,
};
use gpui_kit::{Context, HighlightStyle, SharedString, Window};
use markdown::ParseOptions;
use markdown::mdast::Node;

/// A byte range and the theme name for its look.
pub type Span = (Range<usize>, &'static str);

/// The name the editor recognises as Markdown.
pub const LANGUAGE: &str = "markdown";

/// A factory the editor calls with its language; only Markdown gets a highlighter.
pub fn factory() -> InputHighlighterFactory {
    Rc::new(|language| {
        (language == LANGUAGE)
            .then(|| Box::new(MarkdownHighlighter::default()) as Box<dyn InputHighlighter>)
    })
}

#[derive(Default)]
pub struct MarkdownHighlighter {
    runs: Vec<Span>,
}

impl InputHighlighter for MarkdownHighlighter {
    fn language(&self) -> SharedString {
        LANGUAGE.into()
    }

    fn update(
        &mut self,
        _edit: Option<InputEdit>,
        text: &Rope,
        _folding: bool,
        _window: &mut Window,
        _cx: &mut Context<EditorState>,
    ) {
        self.runs = runs(spans(&text.to_string()));
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let mut out = Vec::new();
        let mut pos = range.start;
        let first = self.runs.partition_point(|(run, _)| run.end <= range.start);
        for (run, name) in &self.runs[first..] {
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

    #[test]
    fn plain_text_and_empty_input_have_no_runs() {
        assert!(names("").is_empty());
        assert!(names("just words\n\nmore words").is_empty());
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
        let highlighter = MarkdownHighlighter {
            runs: names("# A\n\ntext\n\n# B\n"),
        };
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
        let highlighter = MarkdownHighlighter {
            runs: names("# A\n\ntext\n"),
        };
        assert_eq!(
            highlighter.styles(&(5..9), &Named),
            vec![(5..9, HighlightStyle::default())]
        );
    }
}
