//! Export: the document as a PDF (typeset by Typst) or as an HTML page.
//!
//! For PDF the Markdown syntax tree is rewritten as Typst markup, which
//! Typst compiles with the system's fonts (so CJK text sets in a CJK face
//! wherever one is installed) plus the fonts embedded in the binary for
//! Latin text. The markup is plain text, so the rewrite is testable on its
//! own and a compile failure names the construct that caused it.

// Without the `pdf-export` feature nothing compiles the markup, but the
// writer stays (and stays tested): it is the part of this module that is
// smep's own.
#![cfg_attr(not(feature = "pdf-export"), allow(dead_code))]

use std::fmt::Write as _;
use std::path::Path;

use anyhow::Result;
use markdown::ParseOptions;
use markdown::mdast::{AlignKind, Node};

/// Fonts to try, in order, for body text: the system faces of each
/// platform, Latin first and CJK after. Typst takes the first installed one
/// and falls back glyph by glyph through the rest, then through every font
/// on the system, so a missing name costs nothing.
const BODY_FONTS: &[&str] = &[
    "Noto Serif",
    "Georgia",
    "Times New Roman",
    "Times",
    "DejaVu Serif",
    "Liberation Serif",
    "Noto Serif CJK SC",
    "Noto Sans CJK SC",
    "Source Han Serif SC",
    "Source Han Sans SC",
    "PingFang SC",
    "Songti SC",
    "Hiragino Sans GB",
    "Microsoft YaHei",
    "SimSun",
];

const CODE_FONTS: &[&str] = &[
    "Menlo",
    "Consolas",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans Mono",
    "Courier New",
    "Noto Sans Mono CJK SC",
    "Microsoft YaHei",
    "PingFang SC",
];

/// The Markdown as Typst markup, ready to compile.
pub fn to_typst(markdown: &str) -> String {
    let root = markdown::to_mdast(markdown, &ParseOptions::gfm()).unwrap_or(Node::Root(
        markdown::mdast::Root {
            children: Vec::new(),
            position: None,
        },
    ));
    let mut writer = Writer {
        out: String::new(),
        definitions: Vec::new(),
        footnotes: Vec::new(),
    };
    writer.collect(&root);
    writer.preamble();
    writer.blocks(root.children().map(Vec::as_slice).unwrap_or(&[]), 0);
    writer.out
}

/// The Markdown typeset as a PDF. `base` is the document's directory, where
/// relative image paths are looked up; without one images are left out.
#[cfg(feature = "pdf-export")]
pub fn to_pdf(markdown: &str, base: Option<&Path>) -> Result<Vec<u8>> {
    use anyhow::anyhow;
    use typst_as_lib::TypstEngine;
    use typst_as_lib::typst_kit_options::TypstKitFontOptions;
    use typst_layout::PagedDocument;

    let source = to_typst(markdown);
    let mut builder = TypstEngine::builder()
        .main_file(source)
        .search_fonts_with(TypstKitFontOptions::default());
    if let Some(base) = base {
        builder = builder.with_file_system_resolver(base);
    }
    let engine = builder.build();
    let document: PagedDocument = engine
        .compile()
        .output
        .map_err(|err| anyhow!("Typst could not typeset the document: {err}"))?;
    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|errors| anyhow!("could not write the PDF: {errors:?}"))
}

#[cfg(not(feature = "pdf-export"))]
pub fn to_pdf(_markdown: &str, _base: Option<&Path>) -> Result<Vec<u8>> {
    anyhow::bail!("this build of smep was made without PDF export (the `pdf-export` feature)")
}

/// The Markdown as a standalone HTML page: GFM rendering with a small
/// stylesheet, the document's first heading (or `title`) as the title.
pub fn to_html(markdown: &str, title: &str) -> String {
    let options = markdown::Options {
        compile: markdown::CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: false,
            ..markdown::CompileOptions::gfm()
        },
        ..markdown::Options::gfm()
    };
    let body = markdown::to_html_with_options(markdown, &options).unwrap_or_default();
    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>{HTML_STYLE}</style>\n</head>\n<body>\n<main>\n{body}</main>\n</body>\n</html>\n",
        html_escape(title)
    )
}

const HTML_STYLE: &str = "\
body{margin:0;font:16px/1.6 -apple-system,'Segoe UI',Roboto,'Helvetica Neue',Arial,'Noto Sans','PingFang SC','Microsoft YaHei',sans-serif;color:#1f2328;background:#fff}\
main{max-width:46em;margin:0 auto;padding:2em 1.5em}\
h1,h2,h3,h4,h5,h6{line-height:1.25;margin:1.4em 0 .6em}h1{font-size:2em;border-bottom:1px solid #d1d9e0;padding-bottom:.3em}h2{font-size:1.5em;border-bottom:1px solid #d1d9e0;padding-bottom:.3em}\
a{color:#0969da}pre,code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,'Liberation Mono',monospace;font-size:.9em}\
code{background:#f6f8fa;padding:.15em .35em;border-radius:4px}pre{background:#f6f8fa;padding:1em;border-radius:6px;overflow:auto}pre code{background:none;padding:0}\
blockquote{margin:0;padding:0 1em;color:#59636e;border-left:.25em solid #d1d9e0}\
table{border-collapse:collapse;margin:1em 0}th,td{border:1px solid #d1d9e0;padding:.4em .8em}th{background:#f6f8fa}\
img{max-width:100%}hr{border:0;border-top:1px solid #d1d9e0;margin:2em 0}";

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The first heading's text, for a title.
pub fn first_heading(markdown: &str) -> Option<String> {
    let root = markdown::to_mdast(markdown, &ParseOptions::gfm()).ok()?;
    root.children()?.iter().find_map(|node| match node {
        Node::Heading(_) => {
            let text = plain_text(node);
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    })
}

fn plain_text(node: &Node) -> String {
    match node {
        Node::Text(t) => t.value.clone(),
        Node::InlineCode(c) => c.value.clone(),
        _ => node
            .children()
            .map(|children| children.iter().map(plain_text).collect::<String>())
            .unwrap_or_default(),
    }
}

struct Writer {
    out: String,
    /// Link reference definitions: identifier → url.
    definitions: Vec<(String, String)>,
    /// Footnote definitions: identifier → their blocks.
    footnotes: Vec<(String, Vec<Node>)>,
}

impl Writer {
    /// Definitions can sit anywhere in the document, so they are gathered
    /// before anything is written.
    fn collect(&mut self, node: &Node) {
        match node {
            Node::Definition(d) => self
                .definitions
                .push((d.identifier.to_lowercase(), d.url.clone())),
            Node::FootnoteDefinition(f) => self
                .footnotes
                .push((f.identifier.to_lowercase(), f.children.clone())),
            _ => {}
        }
        if let Some(children) = node.children() {
            for child in children {
                self.collect(child);
            }
        }
    }

    fn preamble(&mut self) {
        let fonts = |list: &[&str]| {
            list.iter()
                .map(|font| format!("{font:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(
            self.out,
            "#set page(paper: \"a4\", margin: 2cm, numbering: \"1\")\n\
             #set text(font: ({}), size: 11pt)\n\
             #set par(justify: false, leading: 0.7em)\n\
             #show raw: set text(font: ({}))\n\
             #show link: set text(fill: rgb(\"#0969da\"))\n\
             #show heading.where(level: 1): set text(size: 22pt)\n\
             #show heading.where(level: 2): set text(size: 17pt)\n\
             #show heading.where(level: 3): set text(size: 14pt)\n\
             #show table: set text(size: 10pt)\n",
            fonts(BODY_FONTS),
            fonts(CODE_FONTS)
        );
    }

    /// Top-level and nested blocks, `indent` spaces deep (inside lists).
    fn blocks(&mut self, nodes: &[Node], indent: usize) {
        let mut first = true;
        for node in nodes {
            if matches!(node, Node::Definition(_) | Node::FootnoteDefinition(_)) {
                continue;
            }
            if !first {
                self.out.push('\n');
            }
            first = false;
            self.block(node, indent);
        }
    }

    fn block(&mut self, node: &Node, indent: usize) {
        let pad = " ".repeat(indent);
        match node {
            Node::Heading(h) => {
                let _ = writeln!(
                    self.out,
                    "{pad}{} {}",
                    "=".repeat(h.depth as usize),
                    self.inline(&h.children)
                );
            }
            Node::Paragraph(p) => {
                let _ = writeln!(self.out, "{pad}{}", self.inline(&p.children));
            }
            Node::Code(code) => {
                let lang = code
                    .lang
                    .as_deref()
                    .map(|lang| format!(", lang: {}", typst_string(lang)))
                    .unwrap_or_default();
                let _ = writeln!(
                    self.out,
                    "{pad}#raw(block: true{lang}, {})",
                    typst_string(&code.value)
                );
            }
            Node::Math(math) => {
                let _ = writeln!(
                    self.out,
                    "{pad}#raw(block: true, {})",
                    typst_string(&math.value)
                );
            }
            Node::Blockquote(quote) => {
                let _ = writeln!(self.out, "{pad}#quote(block: true)[");
                self.blocks(&quote.children, indent + 2);
                let _ = writeln!(self.out, "{pad}]");
            }
            Node::List(list) => {
                let mut number = list.start.unwrap_or(1);
                for item in &list.children {
                    let Node::ListItem(item) = item else {
                        continue;
                    };
                    let marker = if list.ordered {
                        // An explicit number keeps the start the author set.
                        format!("{number}. ")
                    } else {
                        "- ".to_string()
                    };
                    number += 1;
                    let task = match item.checked {
                        Some(true) => "☑ ",
                        Some(false) => "☐ ",
                        None => "",
                    };
                    // The first paragraph goes on the marker's line; any
                    // further blocks are indented under it.
                    let mut children = item.children.iter();
                    match children.next() {
                        Some(Node::Paragraph(p)) => {
                            let _ = writeln!(
                                self.out,
                                "{pad}{marker}{task}{}",
                                self.inline(&p.children)
                            );
                        }
                        Some(other) => {
                            let _ = writeln!(self.out, "{pad}{marker}{task}");
                            self.block(other, indent + 2);
                        }
                        None => {
                            let _ = writeln!(self.out, "{pad}{marker}{task}");
                        }
                    }
                    let rest: Vec<Node> = children.cloned().collect();
                    if !rest.is_empty() {
                        self.blocks(&rest, indent + 2);
                    }
                }
            }
            Node::Table(table) => self.table(table, indent),
            Node::ThematicBreak(_) => {
                let _ = writeln!(self.out, "{pad}#line(length: 100%, stroke: 0.5pt + gray)");
            }
            Node::Html(html) => {
                // Raw HTML has no Typst counterpart; its text is kept so
                // nothing silently disappears.
                let text = strip_tags(&html.value);
                if !text.trim().is_empty() {
                    let _ = writeln!(self.out, "{pad}{}", escape(text.trim()));
                }
            }
            Node::Definition(_) | Node::FootnoteDefinition(_) => {}
            other => {
                // Inline content at block level (rare): as a paragraph.
                let _ = writeln!(
                    self.out,
                    "{pad}{}",
                    self.inline(std::slice::from_ref(other))
                );
            }
        }
    }

    fn table(&mut self, table: &markdown::mdast::Table, indent: usize) {
        let pad = " ".repeat(indent);
        let columns = table
            .children
            .iter()
            .filter_map(|row| row.children().map(Vec::len))
            .max()
            .unwrap_or(0);
        if columns == 0 {
            return;
        }
        let align: Vec<&str> = (0..columns)
            .map(|ix| match table.align.get(ix) {
                Some(AlignKind::Center) => "center",
                Some(AlignKind::Right) => "right",
                _ => "left",
            })
            .collect();
        let _ = writeln!(
            self.out,
            "{pad}#table(\n{pad}  columns: {columns},\n{pad}  align: ({},),\n{pad}  stroke: 0.5pt + gray,",
            align.join(", ")
        );
        for (ix, row) in table.children.iter().enumerate() {
            let Some(cells) = row.children() else {
                continue;
            };
            let mut line = String::new();
            for column in 0..columns {
                let content = cells
                    .get(column)
                    .and_then(|cell| cell.children())
                    .map(|inline| self.inline(inline))
                    .unwrap_or_default();
                let _ = write!(line, "[{content}], ");
            }
            if ix == 0 {
                let _ = writeln!(self.out, "{pad}  table.header({line}),");
            } else {
                let _ = writeln!(self.out, "{pad}  {line}");
            }
        }
        let _ = writeln!(self.out, "{pad})");
    }

    /// Inline nodes as one line of markup.
    fn inline(&self, nodes: &[Node]) -> String {
        let mut out = String::new();
        for node in nodes {
            self.inline_into(&mut out, node);
        }
        out
    }

    fn inline_into(&self, out: &mut String, node: &Node) {
        match node {
            Node::Text(t) => out.push_str(&escape(&t.value)),
            Node::Emphasis(e) => {
                let _ = write!(out, "#emph[{}]", self.inline(&e.children));
            }
            Node::Strong(s) => {
                let _ = write!(out, "#strong[{}]", self.inline(&s.children));
            }
            Node::Delete(d) => {
                let _ = write!(out, "#strike[{}]", self.inline(&d.children));
            }
            Node::InlineCode(c) => {
                let _ = write!(out, "#raw({})", typst_string(&c.value));
            }
            Node::InlineMath(m) => {
                let _ = write!(out, "#raw({})", typst_string(&m.value));
            }
            Node::Break(_) => out.push_str(" \\\n"),
            Node::Link(link) => {
                let _ = write!(
                    out,
                    "#link({})[{}]",
                    typst_string(&link.url),
                    self.inline(&link.children)
                );
            }
            Node::LinkReference(link) => {
                let text = self.inline(&link.children);
                match self.definition(&link.identifier) {
                    Some(url) => {
                        let _ = write!(out, "#link({})[{text}]", typst_string(&url));
                    }
                    None => out.push_str(&text),
                }
            }
            Node::Image(image) => self.image(out, &image.url, &image.alt),
            Node::ImageReference(image) => match self.definition(&image.identifier) {
                Some(url) => self.image(out, &url, &image.alt),
                None => out.push_str(&escape(&image.alt)),
            },
            Node::FootnoteReference(footnote) => {
                let content = self
                    .footnotes
                    .iter()
                    .find(|(id, _)| *id == footnote.identifier.to_lowercase())
                    .map(|(_, blocks)| {
                        blocks
                            .iter()
                            .map(|block| match block {
                                Node::Paragraph(p) => self.inline(&p.children),
                                other => self.inline(std::slice::from_ref(other)),
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default();
                let _ = write!(out, "#footnote[{content}]");
            }
            Node::Html(html) => out.push_str(&escape(&strip_tags(&html.value))),
            other => {
                if let Some(children) = other.children() {
                    for child in children {
                        self.inline_into(out, child);
                    }
                }
            }
        }
    }

    /// A local, relative image is shown; anything else (a URL, an absolute
    /// path, a path climbing out of the document's folder) is named.
    fn image(&self, out: &mut String, url: &str, alt: &str) {
        let local = !url.contains("://")
            && !url.starts_with('/')
            && !url.starts_with('\\')
            && !url.contains("..")
            && !url.get(1..2).is_some_and(|c| c == ":");
        if local && !url.is_empty() {
            let _ = write!(out, "#image({}, width: 100%)", typst_string(url));
        } else if !alt.is_empty() {
            let _ = write!(out, "#emph[{}]", escape(alt));
        }
    }

    fn definition(&self, identifier: &str) -> Option<String> {
        let identifier = identifier.to_lowercase();
        self.definitions
            .iter()
            .find(|(id, _)| *id == identifier)
            .map(|(_, url)| url.clone())
    }
}

/// Text as Typst markup: every character that would otherwise be read as
/// syntax gets a backslash.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '#' | '*' | '_' | '`' | '$' | '<' | '>' | '@' | '[' | ']' | '/' | '~' | '\''
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// A Typst string literal.
fn typst_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// The text of raw HTML, tags removed.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The markup after the preamble.
    fn body(markdown: &str) -> String {
        let source = to_typst(markdown);
        let (_, body) = source.split_once("\n\n").unwrap();
        body.to_string()
    }

    #[test]
    fn headings_paragraphs_and_inline_marks() {
        assert_eq!(
            body("# Title\n\nSome **bold**, *it*, ~~gone~~, `code` and [a link](https://x.y).\n"),
            "= Title\n\nSome #strong[bold], #emph[it], #strike[gone], #raw(\"code\") and #link(\"https://x.y\")[a link].\n"
        );
    }

    #[test]
    fn syntax_characters_in_text_are_escaped() {
        assert_eq!(
            body("Price: $5 #1 a_b *c* @d [e] // f\n"),
            "Price: \\$5 \\#1 a\\_b #emph[c] \\@d \\[e\\] \\/\\/ f\n"
        );
        assert_eq!(typst_string("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
    }

    #[test]
    fn lists_nest_and_keep_their_numbers_and_checks() {
        assert_eq!(
            body("3. three\n4. four\n   - nested\n\n- [x] done\n- [ ] todo\n"),
            "3. three\n4. four\n  - nested\n\n- ☑ done\n- ☐ todo\n"
        );
    }

    #[test]
    fn code_blocks_quotes_rules_and_tables() {
        assert_eq!(
            body("```rust\nfn main() {}\n```\n\n> quoted\n\n---\n"),
            "#raw(block: true, lang: \"rust\", \"fn main() {}\")\n\n#quote(block: true)[\n  quoted\n]\n\n#line(length: 100%, stroke: 0.5pt + gray)\n"
        );
        assert_eq!(
            body("| a | b |\n|:-:|--:|\n| 1 | 2 |\n"),
            "#table(\n  columns: 2,\n  align: (center, right,),\n  stroke: 0.5pt + gray,\n  table.header([a], [b], ),\n  [1], [2], \n)\n"
        );
    }

    #[test]
    fn references_footnotes_and_images() {
        assert_eq!(
            body("See [docs][d] and note[^1].\n\n[d]: https://d.io\n[^1]: The note.\n"),
            "See #link(\"https://d.io\")[docs] and note#footnote[The note.].\n"
        );
        assert_eq!(
            body("![local](pic.png) ![remote](https://x/y.png) ![up](../a.png)\n"),
            "#image(\"pic.png\", width: 100%) #emph[remote] #emph[up]\n"
        );
    }

    #[test]
    fn html_keeps_its_text_and_empty_input_is_just_the_preamble() {
        assert_eq!(body("<div>kept <b>text</b></div>\n"), "kept text\n");
        let source = to_typst("");
        assert!(source.starts_with("#set page("));
        assert_eq!(body(""), "", "{source:?}");
    }

    #[test]
    fn the_first_heading_names_the_document() {
        assert_eq!(
            first_heading("intro\n\n## Sub *title*\n"),
            Some("Sub title".into())
        );
        assert_eq!(first_heading("no heading"), None);
    }

    #[test]
    fn html_export_is_a_whole_page() {
        let html = to_html("# Hi <b>\n\n中文 & more\n", "T & <x>");
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<title>T &amp; &lt;x&gt;</title>"));
        assert!(
            html.contains("<h1>Hi <b></h1>"),
            "raw HTML passes through: {html}"
        );
        assert!(html.contains("<p>中文 &amp; more</p>"));
    }

    #[cfg(all(feature = "pdf-export", feature = "pdf-import"))]
    #[test]
    fn a_document_compiles_to_a_pdf_whose_text_reads_back() {
        let markdown = "# 标题 Title\n\nSome *text* with 中文, `code` and a list:\n\n- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```rust\nfn main() {}\n```\n\nnote[^1]\n\n[^1]: The footnote.\n";
        let pdf = to_pdf(markdown, None).unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "{:?}", &pdf[..8]);

        // The same reader the import uses gets the text back out: the fonts
        // are embedded and the words are where a reader finds them.
        let text = crate::convert::from_bytes(&pdf, crate::convert::Format::Pdf).unwrap();
        for expected in [
            "Title", "Some", "text", "code", "one", "two", "fn main", "footnote",
        ] {
            assert!(
                text.contains(expected),
                "{expected:?} missing from {text:?}"
            );
        }
        // CJK needs a system font: Windows and macOS ship one, and CI
        // installs Noto CJK on Linux.
        assert!(text.contains("中文"), "{text:?}");
    }

    #[cfg(feature = "pdf-export")]
    #[test]
    fn a_broken_construct_is_an_error_not_a_panic() {
        // An image that does not exist next to a base directory.
        let dir = std::env::temp_dir().join(format!("smep-export-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let err = to_pdf("![x](missing.png)\n", Some(&dir)).unwrap_err();
        assert!(err.to_string().contains("could not typeset"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
