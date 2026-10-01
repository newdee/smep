//! Other document formats, turned into Markdown so they can be edited here:
//! PDF (text layer only), Word (.docx), HTML and plain text.
//!
//! Each converter is a pure function from the file's bytes to Markdown, so
//! the callers (a dropped file, `File > Import…`, a path on the command
//! line) can run it off the UI thread and the tests can feed it fixtures.

use std::fmt::Write as _;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};
use docx_rust::document::{
    BodyContent, ParagraphContent, RunContent, TableCellContent, TableRowContent,
};
use docx_rust::{DocxFile, document::Paragraph};

/// A format smep can turn into Markdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Docx,
    Html,
    Text,
}

impl Format {
    /// The format a file's extension announces, if it is one to convert.
    /// Markdown and HTML are not here: HTML opens in its own preview and
    /// Markdown needs no conversion, so both are opened, not imported.
    pub fn for_import(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "pdf" => Self::Pdf,
            "docx" => Self::Docx,
            "txt" | "text" => Self::Text,
            _ => return None,
        })
    }

    /// Like [`Self::for_import`], but HTML counts too: for an explicit
    /// "Import…" of a web page as Markdown.
    pub fn for_explicit_import(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "html" | "htm" | "xhtml" => Some(Self::Html),
            _ => Self::for_import(path),
        }
    }
}

/// Where a converted document suggests saving itself: next to the
/// original, as `<stem>.md`.
pub fn markdown_path_for(source: &Path) -> PathBuf {
    source.with_extension("md")
}

/// Convert the file at `path` to Markdown.
pub fn to_markdown(path: &Path, format: Format) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    from_bytes(&bytes, format)
}

/// Convert a document held in memory.
pub fn from_bytes(bytes: &[u8], format: Format) -> Result<String> {
    match format {
        Format::Pdf => pdf(bytes),
        Format::Docx => docx(bytes),
        Format::Html => html(bytes),
        Format::Text => text(bytes),
    }
}

fn text(bytes: &[u8]) -> Result<String> {
    let text = String::from_utf8_lossy(bytes);
    Ok(normalize(&text))
}

fn html(bytes: &[u8]) -> Result<String> {
    let source = String::from_utf8_lossy(bytes);
    let markdown = htmd::convert(&source).context("could not read the HTML")?;
    Ok(normalize(&markdown))
}

#[cfg(not(feature = "pdf-import"))]
fn pdf(_bytes: &[u8]) -> Result<String> {
    bail!("this build of smep was made without PDF import (the `pdf-import` feature)");
}

/// The PDF's text layer, page by page, in pdf-inspector's Markdown
/// (headings, lists and tables where it finds them). Pages without a
/// usable text layer (scans) are marked rather than silently dropped; a
/// document with none at all is an error, since there is nothing to edit.
#[cfg(feature = "pdf-import")]
fn pdf(bytes: &[u8]) -> Result<String> {
    let extraction = pdf_inspector::extract_pages_markdown_mem(bytes, None)
        .map_err(|err| anyhow!("not a readable PDF: {err}"))?;
    let mut parts = Vec::new();
    let mut read_any = false;
    for page in &extraction.pages {
        let number = page.page + 1;
        let text = page.markdown.trim();
        if !page.needs_ocr && !text.is_empty() {
            parts.push(text.to_string());
            read_any = true;
        } else if page.needs_ocr {
            parts.push(format!(
                "> Page {number} has no readable text layer (a scanned image, or a font that does not decode); smep does not do OCR."
            ));
        }
        // An empty page with nothing flagged is simply blank.
    }
    if !read_any {
        bail!("no text in this PDF: its pages are scanned images, and smep does not do OCR");
    }
    Ok(normalize(&parts.join("\n\n")))
}

/// Word's paragraphs, runs and tables. Headings come from the paragraph
/// style, lists from the numbering definition (bullet or numbered, with
/// their level), bold and italic from the run properties, links from the
/// document's relationships. Images are not carried over.
fn docx(bytes: &[u8]) -> Result<String> {
    let file = DocxFile::from_reader(Cursor::new(bytes))
        .map_err(|err| anyhow!("not a readable Word document: {err:?}"))?;
    let docx = file
        .parse()
        .map_err(|err| anyhow!("not a readable Word document: {err:?}"))?;

    let link_target = |id: &str| -> Option<String> {
        docx.document_rels
            .as_ref()?
            .relationships
            .iter()
            .find(|rel| rel.id == id)
            .map(|rel| rel.target.to_string())
    };
    // numId -> the abstract numbering's levels, to tell bullets from numbers.
    let list_marker = |num_id: isize, level: isize| -> String {
        let numbering = docx.numbering.as_ref();
        let format = numbering.and_then(|numbering| {
            let num = numbering
                .numberings
                .iter()
                .find(|n| n.num_id == Some(num_id))?;
            let abstract_id = num.abstract_num_id.as_ref()?.value?;
            let abstract_num = numbering
                .abstract_numberings
                .iter()
                .find(|a| a.abstract_num_id == Some(abstract_id))?;
            let lvl = abstract_num
                .levels
                .iter()
                .find(|l| l.i_level == Some(level))?;
            Some(lvl.number_format.as_ref()?.value.to_string())
        });
        match format.as_deref() {
            Some("bullet") | None => "- ".to_string(),
            _ => "1. ".to_string(),
        }
    };

    let mut out = String::new();
    let mut blocks: Vec<String> = Vec::new();
    for content in &docx.document.body.content {
        match content {
            BodyContent::Paragraph(paragraph) => {
                if let Some(block) = paragraph_markdown(paragraph, &link_target, &list_marker) {
                    blocks.push(block);
                }
            }
            BodyContent::Table(table) => {
                let rows: Vec<Vec<String>> = table
                    .rows
                    .iter()
                    .map(|row| {
                        row.cells
                            .iter()
                            .map(|cell| match cell {
                                TableRowContent::TableCell(cell) => cell
                                    .content
                                    .iter()
                                    .map(|content| match content {
                                        TableCellContent::Paragraph(p) => {
                                            inline_markdown(p, &link_target)
                                        }
                                    })
                                    .filter(|text| !text.is_empty())
                                    .collect::<Vec<_>>()
                                    .join("<br>"),
                                TableRowContent::SDT(sdt) => escape(&sdt.text()),
                            })
                            .collect()
                    })
                    .collect();
                if let Some(table) = table_markdown(&rows) {
                    blocks.push(table);
                }
            }
            BodyContent::Sdt(sdt) => {
                let text = sdt.text();
                if !text.trim().is_empty() {
                    blocks.push(escape(text.trim()));
                }
            }
            BodyContent::SectionProperty(_) | BodyContent::TableCell(_) | BodyContent::Run(_) => {}
        }
    }
    // Consecutive list items stay one list; everything else is its own block.
    let mut previous_was_list = false;
    for block in blocks {
        let is_list = block.starts_with("- ")
            || block.starts_with("1. ")
            || block.starts_with(' ') && block.trim_start().starts_with(['-', '1']);
        if !out.is_empty() {
            out.push_str(if is_list && previous_was_list {
                "\n"
            } else {
                "\n\n"
            });
        }
        out.push_str(&block);
        previous_was_list = is_list;
    }
    Ok(normalize(&out))
}

/// A paragraph as a Markdown block, or `None` for an empty one.
fn paragraph_markdown(
    paragraph: &Paragraph,
    link_target: &dyn Fn(&str) -> Option<String>,
    list_marker: &dyn Fn(isize, isize) -> String,
) -> Option<String> {
    let inline = inline_markdown(paragraph, link_target);
    if inline.trim().is_empty() {
        return None;
    }
    let property = paragraph.property.as_ref();
    let style = property
        .and_then(|p| p.style_id.as_ref())
        .map(|s| s.value.to_ascii_lowercase())
        .unwrap_or_default();
    let heading = match style.as_str() {
        "title" => Some(1),
        "heading1" | "heading 1" => Some(1),
        "heading2" | "heading 2" | "subtitle" => Some(2),
        "heading3" | "heading 3" => Some(3),
        "heading4" | "heading 4" => Some(4),
        "heading5" | "heading 5" => Some(5),
        "heading6" | "heading 6" => Some(6),
        _ => None,
    };
    if let Some(level) = heading {
        return Some(format!("{} {}", "#".repeat(level), inline.trim()));
    }
    // numId 0 is how Word switches a style's numbering off.
    if let Some(numbering) = property
        .and_then(|p| p.numbering.as_ref())
        .filter(|n| n.id.as_ref().map(|id| id.value) != Some(0))
    {
        let id = numbering.id.as_ref().map(|id| id.value).unwrap_or_default();
        let level = numbering
            .level
            .as_ref()
            .map(|l| l.value)
            .unwrap_or(0)
            .max(0);
        let indent = "  ".repeat(level as usize);
        return Some(format!(
            "{indent}{}{}",
            list_marker(id, level),
            inline.trim()
        ));
    }
    if style == "quote" || style == "intensequote" {
        return Some(format!("> {}", inline.trim()));
    }
    Some(inline.trim().to_string())
}

/// The runs of a paragraph as inline Markdown: bold, italic, links, and
/// line breaks as hard breaks.
fn inline_markdown(paragraph: &Paragraph, link_target: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut pending = Styled::default();
    for content in &paragraph.content {
        match content {
            ParagraphContent::Run(run) => pending.push(&mut out, run),
            ParagraphContent::Link(link) => {
                pending.flush(&mut out);
                let mut text = String::new();
                if let Some(run) = &link.content {
                    let mut styled = Styled::default();
                    styled.push(&mut text, run);
                    styled.flush(&mut text);
                }
                let target = link.id.as_deref().and_then(link_target);
                match target {
                    Some(target) if !text.is_empty() => {
                        let _ = write!(out, "[{text}]({target})");
                    }
                    _ => out.push_str(&text),
                }
            }
            ParagraphContent::SDT(sdt) => {
                pending.flush(&mut out);
                out.push_str(&escape(&sdt.text()));
            }
            _ => {}
        }
    }
    pending.flush(&mut out);
    out
}

/// The text of consecutive runs with the same emphasis. Word splits text
/// into runs freely (spell checking, revisions), and marking each run on
/// its own would put `*foo**bar*` side by side, which CommonMark renders
/// with literal asterisks; so runs are joined and marked once.
#[derive(Default)]
struct Styled {
    text: String,
    bold: bool,
    italic: bool,
}

impl Styled {
    fn push(&mut self, out: &mut String, run: &docx_rust::document::Run) {
        let property = run.property.as_ref();
        let on = |flag: Option<bool>| flag != Some(false);
        let bold = property
            .and_then(|p| p.bold.as_ref())
            .is_some_and(|b| on(b.value));
        let italic = property
            .and_then(|p| p.italics.as_ref())
            .is_some_and(|i| on(i.value));
        let mut text = String::new();
        for content in &run.content {
            match content {
                RunContent::Text(t) => text.push_str(&t.text),
                RunContent::Tab(_) => text.push(' '),
                RunContent::Break(_) | RunContent::CarriageReturn(_) => text.push_str("  \n"),
                _ => {}
            }
        }
        if text.is_empty() {
            return;
        }
        if (bold, italic) != (self.bold, self.italic) {
            self.flush(out);
            self.bold = bold;
            self.italic = italic;
        }
        self.text.push_str(&text);
    }

    fn flush(&mut self, out: &mut String) {
        emphasize(out, &self.text, self.bold, self.italic);
        self.text.clear();
    }
}

fn emphasize(out: &mut String, text: &str, bold: bool, italic: bool) {
    if text.is_empty() {
        return;
    }
    // Emphasis markers must hug the text, so surrounding spaces stay outside.
    let leading = text.len() - text.trim_start().len();
    let trailing = text.len() - text.trim_end().len();
    let core = escape(text.trim());
    out.push_str(&text[..leading]);
    if core.is_empty() {
        out.push_str(&text[text.len() - trailing..]);
        return;
    }
    match (bold, italic) {
        (true, true) => {
            let _ = write!(out, "***{core}***");
        }
        (true, false) => {
            let _ = write!(out, "**{core}**");
        }
        (false, true) => {
            let _ = write!(out, "*{core}*");
        }
        (false, false) => out.push_str(&core),
    }
    out.push_str(&text[text.len() - trailing..]);
}

/// A GFM table from rows of cell text; the first row is the header.
fn table_markdown(rows: &[Vec<String>]) -> Option<String> {
    let columns = rows.iter().map(Vec::len).max()?;
    if columns == 0 {
        return None;
    }
    let mut out = String::new();
    for (ix, row) in rows.iter().enumerate() {
        out.push('|');
        for column in 0..columns {
            let cell = row.get(column).map(String::as_str).unwrap_or("");
            let _ = write!(out, " {} |", cell.replace('|', "\\|").replace('\n', " "));
        }
        out.push('\n');
        if ix == 0 {
            out.push('|');
            for _ in 0..columns {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    Some(out.trim_end().to_string())
}

/// Characters that would otherwise be read as Markdown syntax: everywhere,
/// and at a line's start (after any spaces) the ones that open a heading,
/// a list, a table, a fence or a setext underline.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut line = LineStart::default();
    for ch in text.chars() {
        let escaped = matches!(ch, '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>')
            || (line.blank() && matches!(ch, '#' | '-' | '+' | '|' | '=' | '~'))
            || (matches!(ch, '.' | ')') && line.after_number());
        if escaped {
            out.push('\\');
        }
        out.push(ch);
        line.next(ch);
    }
    out
}

/// Where a scan stands relative to the start of its line, for escapers that
/// must neutralise markers there: only spaces so far, or spaces and then
/// only digits (an ordered-list number).
#[derive(Clone, Copy)]
pub struct LineStart {
    /// Digits seen after the leading spaces; `None` once anything else is.
    digits: Option<usize>,
}

impl Default for LineStart {
    fn default() -> Self {
        Self { digits: Some(0) }
    }
}

impl LineStart {
    pub fn blank(self) -> bool {
        self.digits == Some(0)
    }

    pub fn after_number(self) -> bool {
        self.digits.is_some_and(|n| n > 0)
    }

    pub fn next(&mut self, ch: char) {
        self.digits = match (ch, self.digits) {
            ('\n', _) => Some(0),
            (' ', Some(0)) => Some(0),
            (ch, Some(n)) if ch.is_ascii_digit() => Some(n + 1),
            _ => None,
        };
    }
}

/// LF line endings, no trailing whitespace on lines, no more than one
/// blank line in a row, one newline at the end.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0;
    for line in text.replace("\r\n", "\n").lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    let trimmed = out.trim_start_matches('\n').trim_end().to_string();
    if trimmed.is_empty() {
        trimmed
    } else {
        trimmed + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_come_from_the_extension() {
        assert_eq!(Format::for_import(Path::new("a.PDF")), Some(Format::Pdf));
        assert_eq!(Format::for_import(Path::new("a.docx")), Some(Format::Docx));
        assert_eq!(Format::for_import(Path::new("a.txt")), Some(Format::Text));
        assert_eq!(
            Format::for_import(Path::new("a.html")),
            None,
            "opened, not imported"
        );
        assert_eq!(Format::for_import(Path::new("a.md")), None);
        assert_eq!(Format::for_import(Path::new("a")), None);
        assert_eq!(
            Format::for_explicit_import(Path::new("a.htm")),
            Some(Format::Html)
        );
        assert_eq!(
            markdown_path_for(Path::new("/x/report.pdf")),
            PathBuf::from("/x/report.md")
        );
    }

    #[test]
    fn html_becomes_markdown() {
        let md = from_bytes(
            b"<h1>Title</h1><p>Some <b>bold</b> and <a href=\"https://x.y\">a link</a>.</p><ul><li>one</li><li>two</li></ul>",
            Format::Html,
        )
        .unwrap();
        assert_eq!(
            md,
            "# Title\n\nSome **bold** and [a link](https://x.y).\n\n*   one\n*   two\n"
        );
    }

    #[test]
    fn text_is_normalized_not_changed() {
        let md = from_bytes(b"line one\r\n\r\n\r\n\r\nline two  \r\n", Format::Text).unwrap();
        assert_eq!(md, "line one\n\nline two\n");
        assert_eq!(from_bytes(b"", Format::Text).unwrap(), "");
    }

    /// A minimal one-page PDF with a text layer, written by hand.
    #[cfg(feature = "pdf-import")]
    fn tiny_pdf(text: &str) -> Vec<u8> {
        let content = format!("BT /F1 24 Tf 72 720 Td ({text}) Tj ET");
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        ];
        let mut out = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (ix, object) in objects.iter().enumerate() {
            offsets.push(out.len());
            let _ = writeln!(out, "{} 0 obj\n{object}\nendobj", ix + 1);
        }
        let xref = out.len();
        let _ = writeln!(out, "xref\n0 {}\n0000000000 65535 f ", objects.len() + 1);
        for offset in offsets {
            let _ = writeln!(out, "{offset:010} 00000 n ");
        }
        let _ = writeln!(
            out,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
            objects.len() + 1
        );
        out.into_bytes()
    }

    #[cfg(feature = "pdf-import")]
    #[test]
    fn a_pdf_text_layer_comes_out_as_markdown() {
        let md = from_bytes(&tiny_pdf("Hello from smep"), Format::Pdf).unwrap();
        assert!(md.contains("Hello from smep"), "{md:?}");
        assert!(md.ends_with('\n'));
    }

    #[cfg(feature = "pdf-import")]
    #[test]
    fn a_pdf_without_text_is_an_error_and_garbage_is_not_a_pdf() {
        let err = from_bytes(&tiny_pdf(""), Format::Pdf).unwrap_err();
        assert!(err.to_string().contains("no text"), "{err}");
        let err = from_bytes(b"not a pdf at all", Format::Pdf).unwrap_err();
        assert!(err.to_string().contains("not a readable PDF"), "{err}");
    }

    /// A .docx written by docx-rust itself, then read back and converted.
    fn docx_bytes(build: impl FnOnce(&mut docx_rust::Docx)) -> Vec<u8> {
        let mut docx = docx_rust::Docx::default();
        build(&mut docx);
        let cursor = docx.write(Cursor::new(Vec::new())).unwrap();
        cursor.into_inner()
    }

    #[test]
    fn word_headings_runs_lists_and_tables_become_markdown() {
        use docx_rust::document::{Run, Table, TableCell, TableRow, Text};
        use docx_rust::formatting::{Bold, CharacterProperty, Italics, ParagraphProperty};

        let bytes = docx_bytes(|docx| {
            let heading = Paragraph::default()
                .property(ParagraphProperty::default().style_id("Heading1"))
                .push_text("Report");
            let body = Paragraph::default()
                .push(Run::default().push_text("Plain, "))
                .push(
                    Run::default()
                        .property(CharacterProperty::default().bold(Bold::from(true)))
                        .push_text("bold"),
                )
                .push(Run::default().push_text(" and "))
                .push(
                    Run::default()
                        .property(CharacterProperty::default().italics(Italics::from(true)))
                        .push_text("italic"),
                )
                .push(Run::default().push_text(" with a * star."));
            let quote = Paragraph::default()
                .property(ParagraphProperty::default().style_id("Quote"))
                .push_text("Said once.");
            let table = Table::default()
                .push_row(
                    TableRow::default()
                        .push_cell(TableCell::paragraph(Paragraph::default().push_text("A")))
                        .push_cell(TableCell::paragraph(Paragraph::default().push_text("B"))),
                )
                .push_row(
                    TableRow::default()
                        .push_cell(TableCell::paragraph(Paragraph::default().push_text("1")))
                        .push_cell(TableCell::paragraph(Paragraph::default().push_text("2"))),
                );
            docx.document.push(heading);
            docx.document.push(body);
            docx.document.push(quote);
            docx.document.push(table);
            let _ = Text::from("unused");
        });

        let md = from_bytes(&bytes, Format::Docx).unwrap();
        assert_eq!(
            md,
            "# Report\n\nPlain, **bold** and *italic* with a \\* star.\n\n> Said once.\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n"
        );
    }

    #[test]
    fn word_lists_use_the_numbering_definition() {
        use docx_rust::formatting::{
            IndentLevel, NumberingId, NumberingProperty, ParagraphProperty,
        };

        let bytes = docx_bytes(|docx| {
            let item = |text: &'static str, id: isize, level: isize| {
                Paragraph::default()
                    .property(ParagraphProperty::default().numbering(NumberingProperty {
                        id: Some(NumberingId::from(id)),
                        level: Some(IndentLevel::from(level)),
                        ..Default::default()
                    }))
                    .push_text(text)
            };
            docx.document.push(item("first", 1, 0));
            docx.document.push(item("nested", 1, 1));
            docx.document.push(item("second", 1, 0));
            docx.document.push(Paragraph::default().push_text("after"));
        });

        // No numbering part in this file, so every list is a bullet list.
        let md = from_bytes(&bytes, Format::Docx).unwrap();
        assert_eq!(md, "- first\n  - nested\n- second\n\nafter\n");
    }

    #[test]
    fn word_runs_split_mid_format_stay_one_emphasis() {
        use docx_rust::document::Run;
        use docx_rust::formatting::{CharacterProperty, Italics};

        // Word splits text into runs for spell checking and revisions; two
        // italic runs side by side must not come out as `*foo**bar*`,
        // which CommonMark renders with literal asterisks.
        let italic = |text: &'static str| {
            Run::default()
                .property(CharacterProperty::default().italics(Italics::from(true)))
                .push_text(text)
        };
        let bytes = docx_bytes(|docx| {
            docx.document.push(
                Paragraph::default()
                    .push(Run::default().push_text("A "))
                    .push(italic("foo"))
                    .push(italic("bar "))
                    .push(Run::default().push_text("end.")),
            );
        });
        let md = from_bytes(&bytes, Format::Docx).unwrap();
        assert_eq!(md, "A *foobar* end.\n");
        assert_eq!(markdown::to_html(&md), "<p>A <em>foobar</em> end.</p>\n");
        // What marking each run on its own gave.
        assert!(markdown::to_html("A *foo**bar* end.").contains("**"));
    }

    #[test]
    fn word_numbering_id_zero_is_not_a_list() {
        use docx_rust::formatting::{
            IndentLevel, NumberingId, NumberingProperty, ParagraphProperty,
        };

        // numId 0 is how Word switches a style's numbering off.
        let bytes = docx_bytes(|docx| {
            docx.document.push(
                Paragraph::default()
                    .property(ParagraphProperty::default().numbering(NumberingProperty {
                        id: Some(NumberingId::from(0isize)),
                        level: Some(IndentLevel::from(0isize)),
                        ..Default::default()
                    }))
                    .push_text("not a list"),
            );
        });
        assert_eq!(from_bytes(&bytes, Format::Docx).unwrap(), "not a list\n");
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

    #[test]
    fn empty_and_broken_word_files_are_errors_not_panics() {
        assert!(from_bytes(b"", Format::Docx).is_err());
        assert!(from_bytes(b"PK\x03\x04junk", Format::Docx).is_err());
        let md = from_bytes(&docx_bytes(|_| {}), Format::Docx).unwrap();
        assert_eq!(md, "");
    }

    #[test]
    fn escaping_and_normalizing() {
        assert_eq!(escape("a*b_c`d[e]"), "a\\*b\\_c\\`d\\[e\\]");
        assert_eq!(escape("# not a heading"), "\\# not a heading");
        assert_eq!(escape("5 - 3"), "5 - 3");
        // Line starts that would open a list, a setext underline or a fence.
        assert_eq!(escape("1. Intro"), "1\\. Intro");
        assert_eq!(escape("12) b"), "12\\) b");
        assert_eq!(escape("a\n==="), "a\n\\===");
        assert_eq!(escape("~~~"), "\\~~~");
        assert_eq!(escape("in 2024. Then"), "in 2024. Then");
        for text in ["1. Intro", "a  \n===", "~~~", "a  \n---"] {
            let html = markdown::to_html(&escape(text));
            assert!(
                !html.contains("<ol") && !html.contains("<h") && !html.contains("<pre"),
                "{text:?} -> {html}"
            );
        }
        assert_eq!(normalize("\n\na\n\n\n\nb   \n\n"), "a\n\nb\n");
    }
}
