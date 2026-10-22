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
    /// The file this Markdown was converted from (a PDF, a Word file…).
    /// Such a document has no path of its own yet: it is unsaved, named
    /// after the original, and saves next to it by default.
    pub origin: Option<PathBuf>,
}

impl Document {
    pub fn empty() -> Self {
        Self {
            path: None,
            text: String::new(),
            format: PreviewFormat::Markdown,
            origin: None,
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
            origin: None,
        })
    }

    /// Open `path` from the command line: read it, or, when there is no such
    /// file yet in a directory that exists, start an empty document that
    /// saves there (`smep notes.md` before notes.md exists). A missing
    /// directory is still an error, since nothing could be saved.
    pub fn open(path: PathBuf) -> io::Result<Self> {
        match Self::read(path.clone()) {
            Err(err) if err.kind() == io::ErrorKind::NotFound && parent_exists(&path) => {
                let format = PreviewFormat::for_path(&path);
                Ok(Self {
                    path: Some(path),
                    text: String::new(),
                    format,
                    origin: None,
                })
            }
            read => read,
        }
    }

    /// Convert the file at `path` (a PDF, a Word file, text) into a new,
    /// unsaved Markdown document.
    pub fn import(path: PathBuf, format: crate::convert::Format) -> anyhow::Result<Self> {
        let text = crate::convert::to_markdown(&path, format)?;
        Ok(Self {
            path: None,
            text,
            format: PreviewFormat::Markdown,
            origin: Some(path),
        })
    }

    /// The file name, or "Untitled" for a document that has no path yet.
    pub fn display_name(path: Option<&Path>) -> String {
        path.and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string())
    }
}

/// Whether the directory `path` sits in exists (a bare file name is in the
/// current one).
fn parent_exists(path: &Path) -> bool {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.is_dir(),
        _ => true,
    }
}

/// Write `text` to `path`, replacing whatever was there.
/// Whether `a` and `b` name the same file: equal as written, or equal once
/// both are resolved (links, `..`, letter case on Windows). A path that
/// cannot be resolved (no such file yet) is only equal as written.
pub fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

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

/// Where an image a document refers to is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageLocation {
    /// A file on this machine.
    File(PathBuf),
    /// Anything else (a web address, a data URL, or a relative path of a
    /// document that has no directory yet), left to the renderer.
    Uri(String),
}

/// Where `url`, an image in a document kept in `base`, is: a relative path
/// is next to the document (percent-encoded as a Markdown URL may be), a
/// `file://` URL or an absolute path is that file, and a web or data URL
/// stays what it is. Without a directory a relative path stays a URI.
pub fn locate_image(base: Option<&Path>, url: &str) -> ImageLocation {
    if url.starts_with("data:") || (url.contains("://") && !url.starts_with("file://")) {
        return ImageLocation::Uri(url.to_string());
    }
    if let Some(path) = path_from_file_url(url) {
        return ImageLocation::File(path);
    }
    let decoded = percent_decode(url).unwrap_or_else(|| url.to_string());
    let path = Path::new(&decoded);
    if path.is_absolute() {
        return ImageLocation::File(path.to_path_buf());
    }
    match base {
        Some(base) => ImageLocation::File(base.join(path)),
        None => ImageLocation::Uri(url.to_string()),
    }
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
    fn images_are_found_next_to_the_document() {
        let base = Path::new("/docs/notes");
        let file = |path: &str| ImageLocation::File(PathBuf::from(path));
        assert_eq!(
            locate_image(Some(base), "pics/a.png"),
            file("/docs/notes/pics/a.png")
        );
        assert_eq!(
            locate_image(Some(base), "my%20pic.png"),
            file("/docs/notes/my pic.png"),
            "a Markdown URL may be percent-encoded"
        );
        assert_eq!(
            locate_image(Some(base), "../up.png"),
            file("/docs/notes/../up.png")
        );
        #[cfg(not(windows))]
        assert_eq!(locate_image(Some(base), "/abs/b.png"), file("/abs/b.png"));
        #[cfg(not(windows))]
        assert_eq!(
            locate_image(Some(base), "file:///abs/c%20d.png"),
            file("/abs/c d.png")
        );
        #[cfg(windows)]
        assert_eq!(
            locate_image(Some(base), "file:///C:/abs/c%20d.png"),
            file(r"C:/abs/c d.png")
        );
        for uri in [
            "https://x.y/z.png",
            "http://x.y/z.png",
            "data:image/png;base64,iVBORw0KGgo=",
        ] {
            assert_eq!(
                locate_image(Some(base), uri),
                ImageLocation::Uri(uri.to_string()),
                "{uri}"
            );
        }
        // A new document has no directory: a relative path cannot be found.
        assert_eq!(
            locate_image(None, "a.png"),
            ImageLocation::Uri("a.png".to_string())
        );
        // Absolute is the platform's word: a leading slash alone is not
        // absolute on Windows (no drive), where it is kept next to the
        // document like any relative path.
        #[cfg(not(windows))]
        assert_eq!(locate_image(None, "/abs/b.png"), file("/abs/b.png"));
        #[cfg(windows)]
        {
            assert_eq!(
                locate_image(Some(base), r"C:\pics\w.png"),
                file(r"C:\pics\w.png")
            );
            assert_eq!(locate_image(None, r"C:\pics\w.png"), file(r"C:\pics\w.png"));
            assert_eq!(
                locate_image(None, "/abs/b.png"),
                ImageLocation::Uri("/abs/b.png".to_string())
            );
        }
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
    fn open_starts_a_new_document_for_a_missing_file_in_an_existing_directory() {
        let dir = std::env::temp_dir().join(format!("smep-io-open-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let fresh = dir.join("new.html");
        let doc = Document::open(fresh.clone()).unwrap();
        assert_eq!(doc.path, Some(fresh.clone()));
        assert_eq!(doc.text, "");
        assert_eq!(
            doc.format,
            PreviewFormat::Html,
            "the name decides the format"
        );
        assert!(doc.origin.is_none());
        assert!(!fresh.exists(), "nothing is written until the first save");

        // An existing file reads as before.
        std::fs::write(dir.join("old.md"), "kept").unwrap();
        assert_eq!(Document::open(dir.join("old.md")).unwrap().text, "kept");

        // A missing directory is an error: there would be nowhere to save.
        let err = Document::open(dir.join("no-such-dir").join("x.md")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        std::fs::remove_dir_all(&dir).unwrap();
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
