//! Reading and writing documents on disk.

use std::io;
use std::path::{Path, PathBuf};

use crate::app::PreviewFormat;

/// A document as loaded from (or destined for) a file.
#[derive(Debug)]
pub struct Document {
    pub path: Option<PathBuf>,
    pub text: String,
    pub format: PreviewFormat,
}

impl Document {
    pub fn empty() -> Self {
        Self {
            path: None,
            text: String::new(),
            format: PreviewFormat::Markdown,
        }
    }

    /// Read `path` as UTF-8. Line endings are kept exactly as they are.
    pub fn read(path: PathBuf) -> io::Result<Self> {
        let text = std::fs::read_to_string(&path)?;
        let format = PreviewFormat::for_path(&path);
        Ok(Self {
            path: Some(path),
            text,
            format,
        })
    }

    /// The file name, or "Untitled" for a document that has no path yet.
    pub fn display_name(path: Option<&Path>) -> String {
        path.and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string())
    }
}

/// Write `text` to `path`, replacing whatever was there.
pub fn write(path: &Path, text: &str) -> io::Result<()> {
    std::fs::write(path, text)
}

/// The local path in a `file://` URL, as macOS hands them to an app that a
/// document was opened with. Other schemes give `None`.
pub fn path_from_file_url(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    // `file://localhost/x` and `file:///x` both mean `/x`.
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let decoded = percent_decode(rest)?;
    // `file:///C:/x` is a Windows path; drop the leading slash there.
    let decoded = match decoded.as_bytes() {
        [b'/', drive, b':', b'/', ..] if drive.is_ascii_alphabetic() && cfg!(windows) => {
            decoded[1..].to_string()
        }
        _ => decoded,
    };
    Some(PathBuf::from(decoded))
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_falls_back_to_untitled() {
        assert_eq!(Document::display_name(None), "Untitled");
        assert_eq!(
            Document::display_name(Some(Path::new("/tmp/notes/todo.md"))),
            "todo.md"
        );
    }

    #[test]
    fn read_keeps_crlf_and_write_round_trips() {
        let dir = std::env::temp_dir().join(format!("smep-io-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("crlf.md");
        std::fs::write(&path, "a\r\nb\r\n").unwrap();

        let doc = Document::read(path.clone()).unwrap();
        assert_eq!(doc.text, "a\r\nb\r\n");
        assert_eq!(doc.format, PreviewFormat::Markdown);

        write(&path, &doc.text).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"a\r\nb\r\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn file_urls_become_paths() {
        assert_eq!(
            path_from_file_url("file:///Users/k/notes/a%20b.md"),
            Some(PathBuf::from("/Users/k/notes/a b.md"))
        );
        assert_eq!(
            path_from_file_url("file://localhost/tmp/%E4%B8%AD%E6%96%87.md"),
            Some(PathBuf::from("/tmp/中文.md"))
        );
        assert_eq!(path_from_file_url("https://example.com/a.md"), None);
        assert_eq!(path_from_file_url("file://host/a.md"), None);
        assert_eq!(path_from_file_url("file:///bad%zz"), None);
        if cfg!(windows) {
            assert_eq!(
                path_from_file_url("file:///C:/docs/a.md"),
                Some(PathBuf::from("C:/docs/a.md"))
            );
        }
    }

    #[test]
    fn read_rejects_invalid_utf8() {
        let dir = std::env::temp_dir().join(format!("smep-io-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.md");
        std::fs::write(&path, [0x23, 0x20, 0xC3, 0x28]).unwrap();

        let err = match Document::read(path) {
            Ok(_) => panic!("invalid UTF-8 must be rejected"),
            Err(err) => err,
        };
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
