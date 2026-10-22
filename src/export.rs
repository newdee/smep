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
use crate::theme::{self, CODE_LEADING, CODE_SCALE, Palette};

/// The user's own stylesheet for HTML pages, in the config directory.
pub const STYLESHEET_FILE: &str = "export.css";

/// The user's own Typst preamble for PDFs, in the config directory.
pub const PREAMBLE_FILE: &str = "export.typ";

/// Fonts to try, in order, for body text: the system faces of each
/// platform, Latin first and CJK after. Typst takes the first installed one
/// and falls back glyph by glyph through the rest, then through every font
/// on the system, so a missing name costs nothing. Sans, as the blog's
/// body; [`SERIF_FONTS`] for a serif theme.
const SANS_FONTS: &[&str] = &[
    "Segoe UI",
    "Helvetica Neue",
    "Arial",
    "Noto Sans",
    "DejaVu Sans",
    "Liberation Sans",
    "PingFang SC",
    "Microsoft YaHei",
    "Noto Sans CJK SC",
    "Source Han Sans SC",
    "Hiragino Sans GB",
];

/// Serif faces: the blog's headings (its "Song bones"), and the body of a
/// serif theme. Latin first, then a Song face for CJK, then any CJK face.
const SERIF_FONTS: &[&str] = &[
    "Noto Serif",
    "Georgia",
    "Times New Roman",
    "Times",
    "DejaVu Serif",
    "Liberation Serif",
    "Noto Serif CJK SC",
    "Source Han Serif SC",
    "Songti SC",
    "Noto Sans CJK SC",
    "PingFang SC",
    "Microsoft YaHei",
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
    let body = code_boards(&markdown::to_html_with_options(markdown, &options).unwrap_or_default());
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

/// The CSS font stacks: the blog's sans body, its serif headings, mono.
const CSS_SANS: &str = "-apple-system,'PingFang SC','Microsoft YaHei','Noto Sans SC','Segoe UI',Roboto,'Helvetica Neue',Arial,sans-serif";
const CSS_SERIF: &str =
    "'Noto Serif SC','Source Han Serif SC','Songti SC','STSong',Georgia,'Noto Serif',serif";
const CSS_MONO: &str =
    "ui-monospace,'Cascadia Code','SF Mono',Menlo,Consolas,'Liberation Mono',monospace";

/// The stylesheet of an HTML page in `palette`: the blog's article page in
/// the palette's colours (the window draws as much of it as its renderer
/// has hooks for).
pub fn stylesheet(palette: &Palette) -> String {
    let hex = theme::hex;
    let mut soft_accent = hex(palette.accent_soft());
    let _ = write!(
        soft_accent,
        "{:02x}",
        (palette.accent_soft().a * 255.).round() as u8
    );
    // The code board: a whiteboard in a thick frame on light paper, a
    // hairline frame with the accent along its top on dark.
    let board = if palette.dark {
        ".code{margin:1.6rem 0;border:1px solid var(--code-frame);border-top:2px solid var(--accent);border-radius:10px;overflow:hidden}\n\
.code pre{margin:0;border:0;border-radius:0}\n\
.code-head{border-bottom:1px solid var(--line)}\n"
    } else {
        ".code{margin:1.6rem 0;border:5px solid var(--code-frame);border-radius:12px;overflow:hidden;box-shadow:0 0 0 1px rgb(0 0 0/.08),0 2px 8px rgb(26 26 24/.06)}\n\
.code pre{margin:0;border:0;border-radius:0;box-shadow:inset 0 2px 6px rgb(0 0 0/.07)}\n\
.code-head{border-bottom:1px solid rgb(0 0 0/.16)}\n"
    };
    let body_font = if palette.serif { CSS_SERIF } else { CSS_SANS };
    format!(
        "/* smep: the {name} look. Save a copy as export.css in smep's config directory to make it your own. */\n\
:root{{color-scheme:{scheme};--paper:{paper};--paper-dim:{wash};--ink:{ink};--ink-soft:{soft};--ink-faint:{faint};--line:{line};--accent:{accent};--accent-soft:{soft_accent};--code-bg:{code_bg};--code-surface:{surface};--code-head-bg:{rail};--code-frame:{frame};--font-body:{body_font};--font-serif:{CSS_SERIF};--font-mono:{CSS_MONO}}}\n\
body{{margin:0;background:var(--paper);color:var(--ink);font-family:var(--font-body);font-size:1.02rem;line-height:1.85;overflow-wrap:break-word;-webkit-font-smoothing:antialiased}}\n\
main{{max-width:44rem;margin:0 auto;padding:3rem 1.25rem 4rem}}\n\
main>:first-child{{margin-top:0}}\n\
h1,h2,h3,h4,h5,h6{{line-height:1.45;overflow-wrap:anywhere}}\n\
h1{{font-family:var(--font-serif);font-size:{h1}rem;letter-spacing:.02em;margin:3rem 0 1.1rem}}\n\
h2{{font-family:var(--font-serif);font-size:{h2}rem;letter-spacing:.02em;margin:3rem 0 1.05rem;padding-bottom:{gap}rem;background:linear-gradient(to right,var(--line) 0%,var(--line) 30%,transparent 85%) bottom/100% 1px no-repeat}}\n\
h3{{font-family:var(--font-serif);font-size:{h3}rem;margin:2.3rem 0 .85rem}}\n\
h3::before{{content:'';display:inline-block;width:.46em;height:.46em;background:var(--accent);border-radius:1px;margin-right:.55em;vertical-align:.12em}}\n\
h4{{font-size:{h4}rem;font-weight:600;margin:1.7rem 0 .6rem}}\n\
h4::before{{content:'';display:inline-block;width:.42em;height:.42em;border:1.5px solid var(--accent);border-radius:1px;margin-right:.5em;vertical-align:.1em;opacity:.75}}\n\
h5,h6{{font-size:{h5}rem;font-weight:600;color:var(--ink-soft);margin:1.5rem 0 .5rem;letter-spacing:.04em}}\n\
p{{margin:.95rem 0}}\n\
a{{color:var(--accent);text-decoration:underline;text-decoration-color:var(--accent-soft);text-underline-offset:.22em;text-decoration-thickness:1px}}\n\
a:hover{{text-decoration-color:var(--accent)}}\n\
strong{{font-weight:600;background:linear-gradient(transparent 62%,var(--accent-soft) 62%)}}\n\
del{{color:var(--ink-faint)}}\n\
blockquote{{position:relative;margin:1.5rem 0;padding:.8rem 1.2rem .8rem 1.3rem;background:var(--paper-dim);border-left:2px solid var(--line);border-radius:0 8px 8px 0;color:var(--ink-soft);font-size:.97em}}\n\
blockquote::before{{content:'「';position:absolute;top:-.55rem;left:-.4rem;font-family:var(--font-serif);font-size:1.7rem;line-height:1;color:var(--accent);opacity:.5;pointer-events:none}}\n\
blockquote p{{margin:.4rem 0}}\n\
blockquote blockquote{{background:transparent;border-radius:0}}\n\
blockquote blockquote::before{{content:none}}\n\
hr{{border:none;height:1px;margin:2.6rem auto;width:38%;background:linear-gradient(to right,transparent,var(--ink-faint),transparent)}}\n\
table{{border-collapse:collapse;display:block;overflow-x:auto;max-width:100%;font-size:.92rem;margin:1.3rem 0}}\n\
th{{border:none;border-bottom:2px solid var(--ink-faint);padding:.42rem .9rem;text-align:left;font-weight:600}}\n\
td{{border:none;border-bottom:1px solid var(--line);padding:.42rem .9rem}}\n\
code,kbd,pre{{font-family:var(--font-mono)}}\n\
code{{font-size:.86em;background:var(--code-bg);border:1px solid var(--line);border-radius:5px;padding:.08em .4em}}\n\
kbd{{font-size:.8em;border:1px solid var(--line);border-bottom-width:2px;border-radius:5px;padding:.05em .42em;background:var(--code-surface)}}\n\
pre{{margin:1.6rem 0;padding:1rem 1.2rem;overflow-x:auto;line-height:{leading};background:var(--code-surface);border:1px solid var(--code-frame);border-radius:10px}}\n\
pre code{{background:none;border:none;padding:0;font-size:{code}rem}}\n\
{board}\
.code-head{{display:flex;align-items:center;gap:.7rem;padding:.45rem 1.1rem;background:var(--code-head-bg);font-family:var(--font-mono);font-size:.72rem;line-height:1.6}}\n\
.code-lang{{text-transform:uppercase;letter-spacing:.08em;color:var(--accent);background:var(--accent-soft);border-radius:4px;padding:.02rem .45rem}}\n\
img{{max-width:100%;height:auto;border-radius:8px;border:1px solid var(--line)}}\n\
ul,ol{{padding-left:1.45rem;margin:.9rem 0}}\n\
li{{margin:.32rem 0}}\n\
li>p{{margin:0}}\n\
li::marker{{color:var(--ink-faint)}}\n\
input[type=checkbox]{{accent-color:var(--accent);margin-right:.4em}}\n\
li:has(>input[type=checkbox]:first-child){{list-style:none;margin-left:-1.4em}}\n\
.footnotes{{margin-top:3rem;border-top:1px solid var(--line);padding-top:1.1rem;font-size:.88rem;color:var(--ink-soft)}}\n\
.footnotes ol{{padding-left:1.4em}}\n\
.data-footnote-backref{{color:var(--ink-faint);text-decoration:none}}\n\
sup a{{text-decoration:none}}\n\
.sr-only{{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0)}}\n\
::selection{{background:var(--accent-soft)}}\n\
@media print{{main{{max-width:none;padding:0}}.code,pre,blockquote,tr,img{{break-inside:avoid}}h1,h2,h3,h4{{break-after:avoid}}}}\n",
        name = palette.name,
        scheme = if palette.dark { "dark" } else { "light" },
        paper = hex(palette.background),
        wash = hex(palette.wash),
        ink = hex(palette.foreground),
        soft = hex(palette.muted),
        faint = hex(palette.faint),
        line = hex(palette.border),
        accent = hex(palette.link),
        code_bg = hex(palette.code_background),
        surface = hex(palette.code_surface),
        rail = hex(palette.code_rail),
        frame = hex(palette.code_frame),
        h1 = theme::heading_scale(1),
        h2 = theme::heading_scale(2),
        h3 = theme::heading_scale(3),
        h4 = theme::heading_scale(4),
        h5 = theme::heading_scale(5),
        gap = theme::HEADING_RULE_GAP,
        leading = CODE_LEADING,
        code = CODE_SCALE,
    )
}

/// Code blocks as the blog sets them: each `<pre><code>` the Markdown
/// rendering wrote goes into a `figure.code`, under a band naming its
/// language (`code-plain` without one). A `<pre>` from raw HTML that does
/// not close the same way is left alone.
fn code_boards(html: &str) -> String {
    const OPEN: &str = "<pre><code";
    const CLOSE: &str = "</code></pre>";
    let mut out = String::with_capacity(html.len() + 256);
    let mut rest = html;
    while let Some(start) = rest.find(OPEN) {
        let from = &rest[start..];
        let Some(end) = from.find(CLOSE) else {
            break;
        };
        let block = &from[..end + CLOSE.len()];
        if block[OPEN.len()..].contains("<pre") {
            // Another <pre> opens before this one closes: not a block the
            // Markdown rendering wrote. Move past its opening only.
            out.push_str(&rest[..start + OPEN.len()]);
            rest = &rest[start + OPEN.len()..];
            continue;
        }
        out.push_str(&rest[..start]);
        let lang = block
            .strip_prefix("<pre><code class=\"language-")
            .and_then(|tail| tail.split_once('"'))
            .map(|(lang, _)| lang);
        match lang {
            Some(lang) => {
                let _ = write!(
                    out,
                    "<figure class=\"code\"><div class=\"code-head\"><span class=\"code-lang\">{lang}</span></div>{block}</figure>"
                );
            }
            None => {
                let _ = write!(out, "<figure class=\"code code-plain\">{block}</figure>");
            }
        }
        rest = &from[block.len()..];
    }
    out.push_str(rest);
    out
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
    // Lengths written in CSS pixels at a 16 px body, in points at 11.
    let pt = |pixels: f32| (pixels * 11. / 16. * 100.).round() / 100.;
    // The code board's frame, and the band's rule (see `stylesheet`).
    let (frame, band) = if palette.dark {
        (
            format!(
                "(top: {}pt + accent, rest: {}pt + code-frame)",
                pt(theme::CODE_TOP_EDGE),
                pt(theme::CODE_FRAME_DARK)
            ),
            "rule",
        )
    } else {
        (
            format!("{}pt + code-frame", pt(theme::CODE_FRAME_LIGHT)),
            "rgb(0, 0, 0, 16%)",
        )
    };
    let radius = if palette.dark { pt(10.) } else { pt(12.) };
    let body_fonts = if palette.serif {
        SERIF_FONTS
    } else {
        SANS_FONTS
    };
    format!(
        "// smep: the {name} look. Save a copy as export.typ in smep's config directory to make it your own.\n\
#let paper = rgb({paper:?})\n\
#let ink = rgb({ink:?})\n\
#let soft = rgb({soft:?})\n\
#let faint = rgb({faint:?})\n\
#let accent = rgb({accent:?})\n\
#let accent-soft = accent.transparentize({transparent}%)\n\
#let wash = rgb({wash:?})\n\
#let rule = rgb({line:?})\n\
#let code-bg = rgb({code_bg:?})\n\
#let code-surface = rgb({surface:?})\n\
#let code-rail = rgb({rail:?})\n\
#let code-frame = rgb({code_frame:?})\n\
#let serif = ({serif_fonts})\n\
#let mono = ({code_fonts})\n\
#set page(paper: \"a4\", margin: 2cm, numbering: \"1\", fill: paper)\n\
#set text(font: ({body_fonts}), size: 11pt, fill: ink)\n\
#set par(justify: true, leading: 0.85em, spacing: 1.3em)\n\
#show link: it => underline(stroke: 0.5pt + accent-soft, offset: 2.5pt, text(fill: accent, it))\n\
#show strong: it => highlight(fill: accent-soft, top-edge: 0.32em, bottom-edge: -0.18em, it)\n\
#set strong(delta: 200)\n\
#show raw: set text(font: mono, size: {code_size}pt)\n\
#show raw.where(block: false): it => box(fill: code-bg, stroke: 0.5pt + rule, inset: (x: 3pt), outset: (y: 2.5pt), radius: 3pt, it)\n\
#show raw.where(block: true): it => block(width: 100%, above: 1.4em, below: 1.4em, stroke: {frame}, radius: {radius}pt, clip: true, {{\n\
  set block(spacing: 0pt)\n\
  if it.lang != none {{\n\
    block(width: 100%, fill: code-rail, inset: (x: 0.9em, y: 0.4em), stroke: (bottom: 0.5pt + {band}), box(fill: accent-soft, inset: (x: 0.4em, y: 0.15em), radius: 2.5pt, text(size: 0.75em, fill: accent, tracking: 0.08em, upper(it.lang))))\n\
  }}\n\
  block(width: 100%, fill: code-surface, inset: (x: 1.1em, y: 0.9em), {{ set par(justify: false, leading: 0.7em); it }})\n\
}})\n\
#show heading: set text(font: serif)\n\
#show heading: set block(above: 1.8em, below: 0.9em)\n\
#show heading.where(level: 1): set text(size: {h1}em, weight: \"bold\")\n\
#show heading.where(level: 2): it => block(width: 100%, above: 1.8em, below: 0.9em, inset: (bottom: {gap}em), stroke: (bottom: 0.75pt + gradient.linear((rule, 0%), (rule, 30%), (paper, 85%), (paper, 100%))), text(size: {h2}em, weight: \"bold\", it.body))\n\
#show heading.where(level: 3): it => block(above: 1.6em, below: 0.8em, text(size: {h3}em, weight: \"bold\")[#box(width: 0.46em, height: 0.46em, fill: accent, radius: 1pt, baseline: -0.12em)#h(0.55em)#it.body])\n\
#show heading.where(level: 4): it => block(above: 1.4em, below: 0.7em, text(size: {h4}em, weight: \"semibold\", font: ({body_fonts}))[#box(width: 0.42em, height: 0.42em, stroke: 1pt + accent.transparentize(25%), radius: 1pt, baseline: -0.1em)#h(0.5em)#it.body])\n\
#show heading.where(level: 5): set text(size: {h5}em, weight: \"semibold\", fill: soft, tracking: 0.04em, font: ({body_fonts}))\n\
#show heading.where(level: 6): set text(size: {h5}em, weight: \"semibold\", fill: soft, tracking: 0.04em, font: ({body_fonts}))\n\
#show quote.where(block: true): it => block(width: 100%, above: 1.4em, below: 1.4em, fill: wash, stroke: (left: 1.5pt + rule), inset: (left: 1em, right: 0.9em, y: 0.7em), radius: (right: 5.5pt), {{\n\
  place(top + left, dx: -1.25em, dy: -1.05em, text(font: serif, size: 1.7em, fill: accent.transparentize(50%))[「])\n\
  text(size: 0.97em, fill: soft, it.body)\n\
}})\n\
#set list(marker: text(fill: faint)[•])\n\
#set table(stroke: (x, y) => if y == 0 {{ (bottom: 1.5pt + faint) }} else {{ (bottom: 0.5pt + rule) }}, inset: (x: 0.62em, y: 0.45em))\n\
#show table: set text(size: 0.92em)\n\
#show table: set par(justify: false)\n\
#show table.cell.where(y: 0): set text(weight: \"semibold\")\n\
#show image: it => box(stroke: 0.5pt + rule, radius: 5.5pt, clip: true, it)\n\
#set line(stroke: 0.5pt + faint)\n\
#show line: it => if it.length == 100% {{ align(center, line(length: 38%, stroke: 0.6pt + gradient.linear(paper, faint, paper))) }} else {{ it }}\n\
#set footnote.entry(separator: line(length: 30%, stroke: 0.5pt + rule))\n",
        name = palette.name,
        paper = theme::hex(palette.background),
        ink = theme::hex(palette.foreground),
        soft = theme::hex(palette.muted),
        faint = theme::hex(palette.faint),
        accent = theme::hex(palette.link),
        transparent = ((1. - palette.accent_soft().a) * 100.).round(),
        wash = theme::hex(palette.wash),
        line = theme::hex(palette.border),
        code_bg = theme::hex(palette.code_background),
        surface = theme::hex(palette.code_surface),
        rail = theme::hex(palette.code_rail),
        code_frame = theme::hex(palette.code_frame),
        serif_fonts = fonts(SERIF_FONTS),
        code_fonts = fonts(CODE_FONTS),
        body_fonts = fonts(body_fonts),
        // Typst sets raw text at 0.8 of its surroundings on its own, so the
        // size is given outright: the body's 11 pt scaled.
        code_size = (CODE_SCALE * 11. * 100.).round() / 100.,
        h1 = theme::heading_scale(1),
        h2 = theme::heading_scale(2),
        h3 = theme::heading_scale(3),
        h4 = theme::heading_scale(4),
        h5 = theme::heading_scale(5),
        gap = theme::HEADING_RULE_GAP,
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
            "#let faint = rgb(\"#97948c\")",
            "#let accent = rgb(\"#c3423f\")",
            "#let accent-soft = accent.transparentize(87%)",
            "#let wash = rgb(\"#f1efeb\")",
            "#let code-surface = rgb(\"#fdfdfb\")",
            "#let code-rail = rgb(\"#eceae4\")",
            "#let code-frame = rgb(\"#cfccc4\")",
            "#set page(paper: \"a4\", margin: 2cm, numbering: \"1\", fill: paper)",
            "#set text(font: (\"Segoe UI\",",
            "size: 11pt, fill: ink)",
            "text(fill: accent, it)",
            "#show raw: set text(font: mono, size: 9.35pt)",
            // The whiteboard: a thick frame, the band naming the language.
            "stroke: 3.44pt + code-frame, radius: 8.25pt, clip: true",
            "if it.lang != none {",
            "upper(it.lang)",
            "#show heading: set text(font: serif)",
            "#show heading.where(level: 1): set text(size: 1.62em, weight: \"bold\")",
            "inset: (bottom: 0.5em), stroke: (bottom: 0.75pt + gradient.linear((rule, 0%), (rule, 30%), (paper, 85%), (paper, 100%))), text(size: 1.46em, weight: \"bold\", it.body))",
            "text(size: 1.2em, weight: \"bold\")[#box(width: 0.46em, height: 0.46em, fill: accent",
            "stroke: 1pt + accent.transparentize(25%)",
            "#show heading.where(level: 6): set text(size: 0.92em, weight: \"semibold\", fill: soft",
            "fill: wash, stroke: (left: 1.5pt + rule)",
            "[「]",
            "if y == 0 { (bottom: 1.5pt + faint) } else { (bottom: 0.5pt + rule) }",
            "#show table.cell.where(y: 0): set text(weight: \"semibold\")",
            "align(center, line(length: 38%",
        ] {
            assert!(text.contains(line), "{line:?} missing from {text}");
        }
        // The dark paper prints as itself only when asked for by name; the
        // export takes the family's light theme (`print_palette`). Its code
        // board is a hairline with the accent along the top.
        let dark = preamble(&PreviewTheme::PaperDark.palette().unwrap());
        assert!(dark.contains("#let paper = rgb(\"#121212\")"));
        assert!(dark.contains("#let accent-soft = accent.transparentize(85%)"));
        assert!(
            dark.contains(
                "stroke: (top: 1.38pt + accent, rest: 0.69pt + code-frame), radius: 6.88pt"
            )
        );
        assert!(dark.starts_with("// smep: the Paper Dark look."));
        let sepia = preamble(&PreviewTheme::Sepia.palette().unwrap());
        assert!(
            sepia.contains("#set text(font: (\"Noto Serif\","),
            "Sepia is serif"
        );
    }

    #[test]
    fn the_stylesheet_is_the_palettes_design() {
        let css = stylesheet(&paper());
        assert!(css.starts_with("/* smep: the Paper look."), "{css}");
        for line in [
            ":root{color-scheme:light;--paper:#f8f7f3;--paper-dim:#f1efeb;--ink:#1a1a18;--ink-soft:#5d5b55;--ink-faint:#97948c;--line:#e4e1da;--accent:#c3423f;--accent-soft:#c3423f21;--code-bg:#f4f2ee;--code-surface:#fdfdfb;--code-head-bg:#eceae4;--code-frame:#cfccc4;--font-body:-apple-system,",
            "h1{font-family:var(--font-serif);font-size:1.62rem;",
            "h2{font-family:var(--font-serif);font-size:1.46rem;letter-spacing:.02em;margin:3rem 0 1.05rem;padding-bottom:0.5rem;background:linear-gradient(to right,var(--line) 0%,var(--line) 30%,transparent 85%) bottom/100% 1px no-repeat}",
            "h3::before{content:'';display:inline-block;width:.46em;height:.46em;background:var(--accent)",
            "h4::before{content:'';display:inline-block;width:.42em;height:.42em;border:1.5px solid var(--accent)",
            "h5,h6{font-size:0.92rem;font-weight:600;color:var(--ink-soft)",
            "strong{font-weight:600;background:linear-gradient(transparent 62%,var(--accent-soft) 62%)}",
            "blockquote{position:relative;margin:1.5rem 0;padding:.8rem 1.2rem .8rem 1.3rem;background:var(--paper-dim);border-left:2px solid var(--line)",
            "blockquote::before{content:'「'",
            "hr{border:none;height:1px;margin:2.6rem auto;width:38%",
            "th{border:none;border-bottom:2px solid var(--ink-faint)",
            "td{border:none;border-bottom:1px solid var(--line)",
            "code{font-size:.86em;background:var(--code-bg);border:1px solid var(--line);border-radius:5px",
            "pre code{background:none;border:none;padding:0;font-size:0.85rem}",
            "line-height:1.7;",
            // The whiteboard.
            ".code{margin:1.6rem 0;border:5px solid var(--code-frame);border-radius:12px;",
            ".code-lang{text-transform:uppercase;letter-spacing:.08em;color:var(--accent);background:var(--accent-soft)",
            "img{max-width:100%;height:auto;border-radius:8px;border:1px solid var(--line)}",
            "li::marker{color:var(--ink-faint)}",
            ".footnotes{margin-top:3rem;border-top:1px solid var(--line)",
            "@media print{",
        ] {
            assert!(css.contains(line), "{line:?} missing from {css}");
        }
        assert!(css.contains("--font-body:-apple-system,"), "Paper is sans");
        let sepia = stylesheet(&PreviewTheme::Sepia.palette().unwrap());
        assert!(
            sepia.contains("--font-body:'Noto Serif SC'"),
            "Sepia is serif"
        );
        let dark = stylesheet(&PreviewTheme::PaperDark.palette().unwrap());
        assert!(
            dark.contains("color-scheme:dark;--paper:#121212;--paper-dim:#171716;--ink:#c1bfba")
        );
        assert!(dark.contains("--accent-soft:#dd625e26"));
        // On dark paper the board is a hairline with the accent along its top.
        assert!(dark.contains(
            ".code{margin:1.6rem 0;border:1px solid var(--code-frame);border-top:2px solid var(--accent);border-radius:10px"
        ));
    }

    #[test]
    fn code_blocks_become_boards_and_raw_pre_is_left_alone() {
        assert_eq!(
            code_boards("<p>a</p>\n<pre><code class=\"language-rust\">fn x() {}\n</code></pre>\n"),
            "<p>a</p>\n<figure class=\"code\"><div class=\"code-head\"><span class=\"code-lang\">rust</span></div><pre><code class=\"language-rust\">fn x() {}\n</code></pre></figure>\n"
        );
        assert_eq!(
            code_boards("<pre><code>x\n</code></pre>"),
            "<figure class=\"code code-plain\"><pre><code>x\n</code></pre></figure>"
        );
        // A raw <pre><code> that closes another way is not wrapped, and the
        // block after it still is.
        assert_eq!(
            code_boards("<pre><code>raw</code>\n</pre>\n<pre><code>y</code></pre>"),
            "<pre><code>raw</code>\n</pre>\n<figure class=\"code code-plain\"><pre><code>y</code></pre></figure>"
        );
        assert_eq!(code_boards("<pre><code>open"), "<pre><code>open");
        assert_eq!(code_boards(""), "");
        // Through the whole page, and the escaped language stays escaped.
        let html = to_html("```c&d\nint x;\n```\n", "T", "");
        assert!(
            html.contains("<span class=\"code-lang\">c&amp;d</span></div><pre><code class=\"language-c&amp;d\">int x;\n</code></pre></figure>"),
            "{html}"
        );
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
            "```\n```\n",
            "```x\n```\n",
            "```\n\n```\n",
            "> ```rust\n> fn x() {}\n> ```\n",
            "- ###### deep\n  ```\n  y\n  ```\n",
        ] {
            let result = pdf(markdown, None);
            assert!(result.is_ok(), "{markdown:?}: {:?}", result.err());
        }
    }

    #[cfg(feature = "pdf-export")]
    #[test]
    fn every_palette_typesets_a_document() {
        // Every rule of the preamble meets its construct: the headings with
        // their marks, both code boards, a nested quote, the list markers,
        // bold, a footnote, an image in its frame, the section break.
        let markdown = "# T\n\n## S\n\n### Three\n\n#### Four\n\n##### Five\n\n###### Six\n\n> q\n>\n> > nested\n\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\n```\nx\n```\n\n```rust\nfn main() {}\n```\n\n---\n\n- one\n  - two\n1. first\n\n[l](https://x.y), **bold**, `c`, note[^1] and ![icon](smep-256.png)\n\n[^1]: The note.\n";
        let icons = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon");
        for theme in crate::theme::ALL {
            let Some(palette) = theme.palette() else {
                continue;
            };
            let result = to_pdf(markdown, Some(&icons), &preamble(&palette));
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
