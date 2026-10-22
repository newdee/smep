//! Export: the document as a PDF (typeset by Typst) or as an HTML page.
//!
//! Both wear the design of a [`Palette`]: the HTML page carries a
//! stylesheet written from it ([`stylesheet`]), the PDF a Typst preamble
//! ([`preamble`]). Either can be replaced by a file in the config
//! directory, `export.css` or `export.typ` ([`user_stylesheet`],
//! [`user_preamble`]); `smep --css` and `smep --typst` print the ones to
//! start from.
//!
//! For PDF the Markdown syntax tree is rewritten as Typst markup, which
//! Typst compiles with the fonts installed on the machine (none are
//! embedded, so CJK text sets in a CJK face wherever one is installed and
//! a machine without any font cannot export). The markup is plain text, so
//! the rewrite is testable on its own and a compile failure names the
//! construct that caused it. The markup carries no look of its own: every
//! `set` and `show` rule is in the preamble, which is what makes the
//! preamble replaceable.

// Without the `pdf-export` feature nothing compiles the markup, but the
// writer stays (and stays tested): it is the part of this module that is
// smep's own.
#![cfg_attr(not(feature = "pdf-export"), allow(dead_code))]

use std::fmt::Write as _;
use std::path::Path;

use anyhow::Result;
use markdown::ParseOptions;
use markdown::mdast::{AlignKind, Node};

use crate::convert::LineStart;
use crate::theme::{self, CODE_SCALE, Palette, Rule};

/// The user's own stylesheet for HTML pages, in the config directory.
pub const STYLESHEET_FILE: &str = "export.css";

/// The user's own Typst preamble for PDFs, in the config directory.
pub const PREAMBLE_FILE: &str = "export.typ";

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

/// The Markdown as Typst markup, ready to compile: `preamble` (see
/// [`preamble`]), a blank line, the document.
pub fn to_typst(markdown: &str, preamble: &str) -> String {
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
    writer.out.push_str(preamble.trim_end());
    writer.out.push_str("\n\n");
    writer.blocks(root.children().map(Vec::as_slice).unwrap_or(&[]), 0);
    writer.out
}

/// The Markdown typeset as a PDF with `preamble`. `base` is the document's
/// directory, where relative image paths are looked up; without one images
/// are left out.
#[cfg(feature = "pdf-export")]
pub fn to_pdf(markdown: &str, base: Option<&Path>, preamble: &str) -> Result<Vec<u8>> {
    use anyhow::anyhow;
    use typst_as_lib::TypstEngine;
    use typst_as_lib::typst_kit_options::TypstKitFontOptions;
    use typst_layout::PagedDocument;

    let source = to_typst(markdown, preamble);
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
pub fn to_pdf(_markdown: &str, _base: Option<&Path>, _preamble: &str) -> Result<Vec<u8>> {
    anyhow::bail!("this build of smep was made without PDF export (the `pdf-export` feature)")
}

/// The Markdown as a standalone HTML page: GFM rendering with `stylesheet`
/// (see [`stylesheet`]) in its head, the document's first heading (or
/// `title`) as the title.
pub fn to_html(markdown: &str, title: &str, stylesheet: &str) -> String {
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
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>\n{}\n</style>\n</head>\n<body>\n<main>\n{body}</main>\n</body>\n</html>\n",
        html_escape(title),
        stylesheet.trim_end()
    )
}

/// The user's stylesheet, `export.css` in `config_dir`, if there is one.
pub fn user_stylesheet(config_dir: Option<&Path>) -> Option<String> {
    std::fs::read_to_string(config_dir?.join(STYLESHEET_FILE)).ok()
}

/// The user's Typst preamble, `export.typ` in `config_dir`, if there is
/// one.
pub fn user_preamble(config_dir: Option<&Path>) -> Option<String> {
    std::fs::read_to_string(config_dir?.join(PREAMBLE_FILE)).ok()
}

/// The stylesheet of an HTML page in `palette`: the window's design in
/// CSS, with what the window cannot draw (a rule under the first two
/// heading levels is there too; here the quote wears the accent and a
/// wash, table rows alternate, images sit centred).
pub fn stylesheet(palette: &Palette) -> String {
    let paper = theme::hex(palette.background);
    let ink = theme::hex(palette.foreground);
    let soft = theme::hex(palette.muted);
    let accent = theme::hex(palette.link);
    let surface = theme::hex(palette.code_background);
    let line = theme::hex(palette.border);
    let body_font = if palette.serif {
        "Georgia,'Noto Serif','Songti SC','Noto Serif CJK SC',serif"
    } else {
        "-apple-system,'Segoe UI',Roboto,'Helvetica Neue',Arial,'Noto Sans','PingFang SC','Microsoft YaHei',sans-serif"
    };
    let heading = |level: u8| -> String {
        let scale = theme::heading_scale(level);
        let weight = theme::heading_weight(level).0 as u32;
        let colour = if level >= 6 { &soft } else { &ink };
        let rule = match palette.heading_rule(level) {
            Some(Rule { width, colour }) => format!(
                "padding-bottom:{:.2}em;border-bottom:{width}px solid {}",
                theme::HEADING_RULE_GAP,
                theme::hex(colour)
            ),
            None => String::new(),
        };
        format!("h{level}{{font-size:{scale}em;font-weight:{weight};color:{colour};{rule}}}\n")
    };
    let headings: String = (1..=6).map(heading).collect();
    format!(
        "/* smep: the {name} look. Save a copy as export.css in smep's config directory to make it your own. */\n\
:root{{color-scheme:{scheme};--paper:{paper};--ink:{ink};--soft:{soft};--accent:{accent};--surface:{surface};--line:{line}}}\n\
body{{margin:0;background:var(--paper);color:var(--ink);font:16px/1.7 {body_font};-webkit-font-smoothing:antialiased}}\n\
main{{max-width:44em;margin:0 auto;padding:3em 1.5em 4em}}\n\
h1,h2,h3,h4,h5,h6{{line-height:1.3;margin:1.6em 0 .6em;overflow-wrap:anywhere}}\n\
h1:first-child{{margin-top:0}}\n\
{headings}\
p,ul,ol,blockquote,pre,table,figure{{margin:0 0 .95em}}\n\
li{{margin:.2em 0}}\n\
li>p{{margin:0}}\n\
a{{color:var(--accent);text-decoration:none;border-bottom:1px solid transparent}}\n\
a:hover{{border-bottom-color:var(--accent)}}\n\
strong{{font-weight:600}}\n\
code,pre,kbd{{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,'Liberation Mono','Noto Sans Mono',monospace;font-size:{code}em}}\n\
code{{background:var(--surface);padding:.1em .35em;border-radius:4px;overflow-wrap:anywhere}}\n\
pre{{background:var(--surface);border:1px solid var(--line);border-radius:8px;padding:.75em 1em;overflow:auto;line-height:1.5}}\n\
pre code{{background:none;padding:0;font-size:1em;border-radius:0}}\n\
blockquote{{padding:.5em 1em;color:var(--soft);background:var(--surface);border-left:3px solid var(--accent);border-radius:0 6px 6px 0}}\n\
blockquote>:last-child{{margin-bottom:0}}\n\
table{{border-collapse:collapse;width:100%;border:1px solid var(--line);font-size:.95em}}\n\
th,td{{padding:.45em .8em;text-align:left;vertical-align:top;border-bottom:1px solid var(--line)}}\n\
th{{background:var(--ink);color:var(--paper);font-weight:600}}\n\
th a{{color:inherit}}\n\
tr:nth-child(even) td{{background:var(--surface)}}\n\
img{{max-width:100%;height:auto;display:block;margin:1em auto;border-radius:6px}}\n\
hr{{border:0;border-top:1px solid var(--line);margin:2em 0}}\n\
input[type=checkbox]{{accent-color:var(--accent);margin-right:.4em}}\n\
.task-list-item{{list-style:none;margin-left:-1.4em}}\n\
.footnotes{{margin-top:3em;padding-top:1em;border-top:1px solid var(--line);font-size:.9em;color:var(--soft)}}\n\
.footnotes ol{{padding-left:1.4em}}\n\
.sr-only{{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0)}}\n\
sup a{{border-bottom:0;font-weight:600}}\n\
@media print{{body{{background:#fff}}main{{max-width:none;padding:0}}a{{color:inherit}}pre,blockquote,tr{{break-inside:avoid}}h1,h2,h3{{break-after:avoid}}}}\n",
        name = palette.name,
        scheme = if palette.dark { "dark" } else { "light" },
        code = CODE_SCALE,
    )
}

/// The Typst preamble of a PDF in `palette`: the page, the fonts, and the
/// same design as the stylesheet in `set` and `show` rules. The markup
/// after it is plain, so replacing the preamble replaces the whole look.
pub fn preamble(palette: &Palette) -> String {
    let fonts = |list: &[&str]| {
        list.iter()
            .map(|font| format!("{font:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let rule = |level: u8| -> String {
        let scale = theme::heading_scale(level);
        let weight = match theme::heading_weight(level) {
            gpui_kit::FontWeight::BOLD => "bold",
            _ => "semibold",
        };
        match palette.heading_rule(level) {
            Some(Rule { width, colour }) => format!(
                "#show heading.where(level: {level}): it => block(width: 100%, above: 1.4em, below: 0.9em, inset: (bottom: {gap}em), stroke: (bottom: {stroke}pt + rgb({colour:?})), text(size: {scale}em, weight: {weight:?}, it.body))\n",
                gap = theme::HEADING_RULE_GAP,
                stroke = width * 0.75,
                colour = theme::hex(colour),
            ),
            None if level >= 6 => format!(
                "#show heading.where(level: {level}): set text(size: {scale}em, weight: {weight:?}, fill: soft)\n"
            ),
            None => format!(
                "#show heading.where(level: {level}): set text(size: {scale}em, weight: {weight:?})\n"
            ),
        }
    };
    let headings: String = (1..=6).map(rule).collect();
    format!(
        "// smep: the {name} look. Save a copy as export.typ in smep's config directory to make it your own.\n\
#let paper = rgb({paper:?})\n\
#let ink = rgb({ink:?})\n\
#let soft = rgb({soft:?})\n\
#let accent = rgb({accent:?})\n\
#let surface = rgb({surface:?})\n\
#let rule = rgb({line:?})\n\
#set page(paper: \"a4\", margin: 2cm, numbering: \"1\", fill: paper)\n\
#set text(font: ({body_fonts}), size: 11pt, fill: ink)\n\
#set par(justify: false, leading: 0.7em)\n\
#show link: set text(fill: accent)\n\
#show raw: set text(font: ({code_fonts}), size: {code}em)\n\
#show raw.where(block: true): it => block(width: 100%, fill: surface, stroke: 0.5pt + rule, radius: 4pt, inset: 8pt, it)\n\
#show raw.where(block: false): it => box(fill: surface, inset: (x: 3pt), outset: (y: 3pt), radius: 2pt, it)\n\
{headings}\
#show quote.where(block: true): it => block(width: 100%, fill: surface, stroke: (left: 2pt + accent), inset: (left: 1em, right: 1em, y: 0.6em), radius: (right: 4pt), text(fill: soft, it.body))\n\
#set table(stroke: (x, y) => (bottom: 0.5pt + rule), fill: (x, y) => if y == 0 {{ ink }} else if calc.even(y) {{ surface }} else {{ none }}, inset: (x: 0.6em, y: 0.45em))\n\
#show table: set text(size: 10pt)\n\
#show table.cell.where(y: 0): set text(fill: paper, weight: \"semibold\")\n\
#set line(stroke: 0.5pt + rule)\n",
        name = palette.name,
        paper = theme::hex(palette.background),
        ink = theme::hex(palette.foreground),
        soft = theme::hex(palette.muted),
        accent = theme::hex(palette.link),
        surface = theme::hex(palette.code_background),
        line = theme::hex(palette.border),
        body_fonts = fonts(BODY_FONTS),
        code_fonts = fonts(CODE_FONTS),
        code = CODE_SCALE,
    )
}

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
                let _ = writeln!(
                    self.out,
                    "{pad}{}",
                    indented(&self.inline(&p.children), &pad)
                );
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
                                indented(&self.inline(&p.children), &format!("{pad}  "))
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
                // Its stroke is the preamble's (`set line`).
                let _ = writeln!(self.out, "{pad}#line(length: 100%)");
            }
            Node::Html(html) => {
                // Raw HTML has no Typst counterpart; its text is kept so
                // nothing silently disappears.
                let text = strip_tags(&html.value);
                if !text.trim().is_empty() {
                    let _ = writeln!(self.out, "{pad}{}", indented(&escape(text.trim()), &pad));
                }
            }
            Node::Definition(_) | Node::FootnoteDefinition(_) => {}
            other => {
                // Inline content at block level (rare): as a paragraph.
                let _ = writeln!(
                    self.out,
                    "{pad}{}",
                    indented(&self.inline(std::slice::from_ref(other)), &pad)
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
        // Strokes, fills and the header's type are the preamble's
        // (`set table`, `show table.cell`).
        let _ = writeln!(
            self.out,
            "{pad}#table(\n{pad}  columns: {columns},\n{pad}  align: ({},),",
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
/// syntax gets a backslash, and so does a line start that Typst reads as a
/// heading (`=`), a list (`-`, `+`) or a numbered item (`1.`).
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut line = LineStart::default();
    for ch in text.chars() {
        let escaped = matches!(
            ch,
            '\\' | '#' | '*' | '_' | '`' | '$' | '<' | '>' | '@' | '[' | ']' | '/' | '~' | '\''
        ) || (line.blank() && matches!(ch, '=' | '-' | '+'))
            || (ch == '.' && line.after_number());
        if escaped {
            out.push('\\');
        }
        out.push(ch);
        line.next(ch);
    }
    out
}

/// Markup that may span lines, with every line after the first indented by
/// `pad`: Typst ends a list item at the first line indented no deeper than
/// its marker.
fn indented(markup: &str, pad: &str) -> String {
    if pad.is_empty() {
        return markup.to_string();
    }
    markup.replace('\n', &format!("\n{pad}"))
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

    use crate::theme::PreviewTheme;

    fn paper() -> Palette {
        PreviewTheme::Paper.palette().unwrap()
    }

    fn paper_preamble() -> String {
        preamble(&paper())
    }

    fn pdf(markdown: &str, base: Option<&Path>) -> Result<Vec<u8>> {
        to_pdf(markdown, base, &paper_preamble())
    }

    /// The markup after the preamble.
    fn body(markdown: &str) -> String {
        let source = to_typst(markdown, "#set text(size: 11pt)");
        let (preamble, body) = source.split_once("\n\n").unwrap();
        assert_eq!(preamble, "#set text(size: 11pt)");
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
    fn line_starts_that_typst_reads_as_markup_are_escaped() {
        // Plain text in Markdown, a heading or a list in Typst.
        assert_eq!(
            body("Total\n= 5\n\n\\- not a list\n\n\\+ nor this\n\n2\\. nor this\n"),
            "Total\n\\= 5\n\n\\- not a list\n\n\\+ nor this\n\n2\\. nor this\n"
        );
        assert_eq!(body("a = b - c + 1. d\n"), "a = b - c + 1. d\n");
    }

    #[test]
    fn continuation_lines_stay_inside_their_list_item() {
        // Typst ends an item at the first line indented no deeper than its
        // marker.
        assert_eq!(
            body("- one\n  two\n- three  \n  four\n"),
            "- one\n  two\n- three \\\n  four\n"
        );
        assert_eq!(body("1. a\n   - b\n     c\n"), "1. a\n  - b\n    c\n");
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
            "#raw(block: true, lang: \"rust\", \"fn main() {}\")\n\n#quote(block: true)[\n  quoted\n]\n\n#line(length: 100%)\n"
        );
        assert_eq!(
            body("| a | b |\n|:-:|--:|\n| 1 | 2 |\n"),
            "#table(\n  columns: 2,\n  align: (center, right,),\n  table.header([a], [b], ),\n  [1], [2], \n)\n"
        );
    }

    #[test]
    fn the_markup_carries_no_look_of_its_own() {
        // Every colour, stroke and size is the preamble's, so a preamble of
        // the user's own restyles the whole document.
        let markup =
            body("# T\n\n> q\n\n---\n\n| a |\n|---|\n| 1 |\n\n```\nx\n```\n\n[l](https://x.y)\n");
        for word in ["rgb(", "stroke", "fill", "size:", "#set", "#show"] {
            assert!(!markup.contains(word), "{word:?} in {markup}");
        }
    }

    #[test]
    fn the_preamble_is_the_palettes_design() {
        let text = paper_preamble();
        assert!(text.starts_with("// smep: the Paper look."), "{text}");
        for line in [
            "#let paper = rgb(\"#f8f7f3\")",
            "#let ink = rgb(\"#1a1a18\")",
            "#let accent = rgb(\"#c3423f\")",
            "#set page(paper: \"a4\", margin: 2cm, numbering: \"1\", fill: paper)",
            "size: 11pt, fill: ink)",
            "#show link: set text(fill: accent)",
            "size: 0.9em)",
            "#show heading.where(level: 1): it => block(width: 100%, above: 1.4em, below: 0.9em, inset: (bottom: 0.25em), stroke: (bottom: 1.5pt + rgb(\"#1a1a18\")), text(size: 1.62em, weight: \"bold\", it.body))",
            "#show heading.where(level: 2): it => block(width: 100%, above: 1.4em, below: 0.9em, inset: (bottom: 0.25em), stroke: (bottom: 0.75pt + rgb(\"#e4e1da\")), text(size: 1.46em, weight: \"semibold\", it.body))",
            "#show heading.where(level: 3): set text(size: 1.2em, weight: \"semibold\")",
            "#show heading.where(level: 6): set text(size: 0.92em, weight: \"semibold\", fill: soft)",
            "#show table.cell.where(y: 0): set text(fill: paper, weight: \"semibold\")",
            "#set line(stroke: 0.5pt + rule)",
            "\"Noto Serif\", \"Georgia\"",
        ] {
            assert!(text.contains(line), "{line:?} missing from {text}");
        }
        // The dark paper prints as itself only when asked for by name; the
        // export takes the family's light theme (`print_palette`).
        let dark = preamble(&PreviewTheme::PaperDark.palette().unwrap());
        assert!(dark.contains("#let paper = rgb(\"#121212\")"));
        assert!(dark.starts_with("// smep: the Paper Dark look."));
    }

    #[test]
    fn the_stylesheet_is_the_palettes_design() {
        let css = stylesheet(&paper());
        assert!(css.starts_with("/* smep: the Paper look."), "{css}");
        for line in [
            ":root{color-scheme:light;--paper:#f8f7f3;--ink:#1a1a18;--soft:#5d5b55;--accent:#c3423f;--surface:#f4f2ee;--line:#e4e1da}",
            "h1{font-size:1.62em;font-weight:700;color:#1a1a18;padding-bottom:0.25em;border-bottom:2px solid #1a1a18}",
            "h2{font-size:1.46em;font-weight:600;color:#1a1a18;padding-bottom:0.25em;border-bottom:1px solid #e4e1da}",
            "h3{font-size:1.2em;font-weight:600;color:#1a1a18;}",
            "h6{font-size:0.92em;font-weight:600;color:#5d5b55;}",
            "th{background:var(--ink);color:var(--paper);font-weight:600}",
            "tr:nth-child(even) td{background:var(--surface)}",
            "blockquote{padding:.5em 1em;color:var(--soft);background:var(--surface);border-left:3px solid var(--accent)",
            "pre{background:var(--surface);border:1px solid var(--line);border-radius:8px",
            "font-size:0.9em}",
            "img{max-width:100%;height:auto;display:block;margin:1em auto;border-radius:6px}",
            ".footnotes{",
            "@media print{",
        ] {
            assert!(css.contains(line), "{line:?} missing from {css}");
        }
        assert!(css.contains("sans-serif"), "Paper is sans");
        let sepia = stylesheet(&PreviewTheme::Sepia.palette().unwrap());
        assert!(sepia.contains("Georgia,'Noto Serif'"), "Sepia is serif");
        let dark = stylesheet(&PreviewTheme::PaperDark.palette().unwrap());
        assert!(dark.contains("color-scheme:dark;--paper:#121212;--ink:#c1bfba"));
    }

    #[test]
    fn the_users_own_files_replace_the_built_in_look() {
        let dir = std::env::temp_dir().join(format!("smep-export-look-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(user_stylesheet(Some(&dir)), None);
        assert_eq!(user_preamble(Some(&dir)), None);
        assert_eq!(user_stylesheet(None), None, "no config dir, nothing read");

        std::fs::write(dir.join(STYLESHEET_FILE), "body{color:red}\n").unwrap();
        std::fs::write(dir.join(PREAMBLE_FILE), "#set text(red)\n").unwrap();
        assert_eq!(
            user_stylesheet(Some(&dir)).as_deref(),
            Some("body{color:red}\n")
        );
        assert_eq!(
            user_preamble(Some(&dir)).as_deref(),
            Some("#set text(red)\n")
        );

        let html = to_html("hi\n", "T", "body{color:red}\n");
        assert!(
            html.contains("<style>\nbody{color:red}\n</style>"),
            "{html}"
        );
        assert!(to_typst("hi\n", "#set text(red)\n").starts_with("#set text(red)\n\nhi\n"));
        std::fs::remove_dir_all(&dir).unwrap();
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
        let source = to_typst("", &paper_preamble());
        assert!(source.starts_with("// smep: the Paper look."));
        assert!(source.contains("#set page("));
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
        let html = to_html(
            "# Hi <b>\n\n中文 & more\n",
            "T & <x>",
            &stylesheet(&paper()),
        );
        assert!(html.contains("<style>\n/* smep: the Paper look."));
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
        let markdown = "# 标题 Title\n\n## Second\n\n### Third\n\nSome *text* with 中文, `code` and a list:\n\n- one\n  wrapped\n- two\n\n> quoted\n\n---\n\nTotal\n= 5\n\n2\\. counted\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\n```rust\nfn main() {}\n```\n\nnote[^1]\n\n[^1]: The footnote.\n";
        let pdf = pdf(markdown, None).unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "{:?}", &pdf[..8]);

        // The same reader the import uses gets the text back out: the fonts
        // are embedded in the PDF and the words are where a reader finds
        // them. The escaped line starts typeset as text, not as a heading
        // or a list.
        let text = crate::convert::from_bytes(&pdf, crate::convert::Format::Pdf).unwrap();
        for expected in [
            "Title",
            "Second",
            "Third",
            "Some",
            "text",
            "code",
            "one",
            "wrapped",
            "two",
            "quoted",
            "= 5",
            "2. counted",
            "fn main",
            "footnote",
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
    fn the_same_document_exports_to_the_same_bytes() {
        // No timestamp or random id in the PDF, so an export can be
        // compared with the last one.
        let markdown = "# Same\n\nText, a [link](https://x.y) and a list:\n\n- one\n- two\n";
        let first = pdf(markdown, None).unwrap();
        let second = pdf(markdown, None).unwrap();
        assert!(first == second, "{} vs {} bytes", first.len(), second.len());
        // For comparing across processes (`--nocapture`): the font list is
        // read from the system on each run.
        let hash = first.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        eprintln!("pdf fnv1a {hash:016x}, {} bytes", first.len());
        let css = stylesheet(&paper());
        assert_eq!(
            to_html(markdown, "Same", &css),
            to_html(markdown, "Same", &css),
            "HTML too"
        );
    }

    #[cfg(feature = "pdf-export")]
    #[test]
    fn degenerate_inputs_typeset_without_error() {
        for markdown in [
            "",
            "\n",
            "=",
            "\\-",
            "\\+",
            "1\\.",
            "9999999999\\. ten digits",
            "  = indented",
            "a\r\n= crlf\r\n",
            "- \n",
            "-\n\n  orphan",
            "1. \n   = x",
            "> = in a quote\n> - not a list",
            "- a  \n  \\- b  \n  \\= c",
            "| = | - |\n|---|---|\n| + | 1. |",
            "[^n]\n\n[^n]: = foot\n  - note",
            "\\#\\$\\@\\/\\/\\~\\'",
            "<b>= html</b>",
            "#### four\n\n##### five\n\n###### six\n\n> # heading in a quote\n\n| only header |\n|---|\n",
        ] {
            let result = pdf(markdown, None);
            assert!(result.is_ok(), "{markdown:?}: {:?}", result.err());
        }
    }

    #[cfg(feature = "pdf-export")]
    #[test]
    fn every_palette_typesets_a_document() {
        let markdown = "# T\n\n## S\n\n> q\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\n```\nx\n```\n\n---\n\n[l](https://x.y) and `c`.\n";
        for theme in crate::theme::ALL {
            let Some(palette) = theme.palette() else {
                continue;
            };
            let result = to_pdf(markdown, None, &preamble(&palette));
            assert!(result.is_ok(), "{theme:?}: {:?}", result.err());
        }
    }

    #[test]
    fn escaping_is_linear_in_a_long_line() {
        let line = "1".repeat(1 << 20) + ". =";
        let started = std::time::Instant::now();
        let escaped = escape(&line);
        assert!(
            escaped.ends_with("\\. ="),
            "{}",
            &escaped[escaped.len() - 8..]
        );
        assert!(started.elapsed().as_secs() < 2, "{:?}", started.elapsed());
    }

    #[cfg(feature = "pdf-export")]
    #[test]
    fn a_broken_construct_is_an_error_not_a_panic() {
        // An image that does not exist next to a base directory.
        let dir = std::env::temp_dir().join(format!("smep-export-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let err = pdf("![x](missing.png)\n", Some(&dir)).unwrap_err();
        assert!(err.to_string().contains("could not typeset"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
