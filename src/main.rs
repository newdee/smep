//! smep — a Simple Markdown Editor & Previewer, written in Rust.
//!
//! Usage: `smep [FILE]...`. With no argument the editor starts empty; with
//! several, each file opens in a tab. A smep already running takes the
//! files instead (see `instance`).
//! `smep --version` and `smep --help` print and exit without a window, so
//! package tests and shell completion can ask them on a headless machine.

// A GUI program on Windows: no console window appears when smep is started
// from Explorer, "Open with" or a shortcut. (The default, the console
// subsystem, makes Windows open a terminal for any launch without one.)
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod convert;
mod defaults;
mod export;
mod highlight;
mod insert;
mod instance;
mod io;
mod keymap;
mod rendered;
mod settings;
mod theme;

use std::io::Write as _;
use std::path::PathBuf;

use futures::StreamExt as _;
use gpui_kit::assets::Assets;
use gpui_kit::component::{Root, TitleBar};
use gpui_kit::*;

use io::Document;
use settings::Settings;

const USAGE: &str = "\
Usage: smep [FILE]...

A Simple Markdown Editor & Previewer. Opens each FILE (Markdown or HTML) in
a tab, or an empty document; a FILE that does not exist yet is a new
document that saves there. A PDF, Word (.docx) or text FILE is converted to
a new, unsaved Markdown document. While smep is running, `smep FILE` opens
FILE in a tab of that window.

Options:
  -h, --help       Print this help and exit
  -V, --version    Print the version and exit
  --css [THEME]    Print the stylesheet an exported HTML page carries
  --typst [THEME]  Print the Typst preamble an exported PDF is typeset with

THEME is a preview theme (paper, paper-dark, github, newsprint, night,
sepia, solarized-light, solarized-dark); paper when left out. Saved as
export.css or export.typ in smep's config directory, the printed text
replaces the built-in one for every export.";

/// A GUI-subsystem process gets no console of its own. When a terminal
/// started smep, attach to the terminal's, so `--version`, `--help` and the
/// error for an unreadable file print there. Started from Explorer there is
/// no parent console, the call fails and nothing changes.
///
/// A standard handle the parent redirected (`smep --version > v.txt`, a
/// pipe) arrives set, and AttachConsole would replace it with the console;
/// those are put back, so redirection keeps working.
#[cfg(windows)]
fn attach_parent_console() {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE,
        STD_OUTPUT_HANDLE, SetStdHandle,
    };
    let kinds = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];
    // SAFETY: plain Win32 calls on this process's own console state and
    // standard handles, made once before any I/O.
    unsafe {
        let inherited = kinds.map(|kind| GetStdHandle(kind));
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }
        for (kind, handle) in kinds.into_iter().zip(inherited) {
            if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
                SetStdHandle(kind, handle);
            }
        }
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}

fn main() {
    attach_parent_console();
    let argument = std::env::args_os().nth(1);
    match argument.as_deref().and_then(|arg| arg.to_str()) {
        // A closed pipe (`smep --help | head -1`) is not worth a panic.
        Some("-h" | "--help") => {
            let _ = writeln!(std::io::stdout(), "{USAGE}");
            return;
        }
        Some("-V" | "--version") => {
            let _ = writeln!(std::io::stdout(), "smep {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some(flag @ ("--css" | "--typst")) => {
            let name = std::env::args()
                .nth(2)
                .unwrap_or_else(|| "paper".to_string());
            let Some(palette) =
                theme::PreviewTheme::parse(&name).and_then(theme::PreviewTheme::palette)
            else {
                eprintln!(
                    "smep: no preview theme named {name:?}; one of {}",
                    theme::ALL
                        .iter()
                        .filter_map(|theme| theme.palette().map(|_| theme.name()))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                std::process::exit(2);
            };
            let text = if flag == "--css" {
                export::stylesheet(&palette)
            } else {
                export::preamble(&palette)
            };
            let _ = write!(std::io::stdout(), "{text}");
            return;
        }
        _ => {}
    }

    let settings = Settings::load();
    // Relative to where smep was started, so a running smep (another
    // directory, maybe) finds them.
    let paths: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .map(|path| std::path::absolute(&path).unwrap_or(path))
        .collect();

    // Files opened through the OS (a double-click in Finder, "Open with")
    // arrive as file:// URLs, possibly before the window exists, and files
    // handed over by a later launch arrive from the instance listener; they
    // queue here and the window drains the queue once it is up.
    let (opens, mut requested) = futures::channel::mpsc::unbounded::<Vec<PathBuf>>();

    // One smep per user: when one is running, it opens the files in tabs
    // and this launch is done; otherwise this one listens for later ones.
    if let Some(name) = settings.dir().and_then(|dir| instance::name(dir).ok()) {
        let open = {
            let opens = opens.clone();
            move |paths| {
                let _ = opens.unbounded_send(paths);
            }
        };
        if instance::claim(name, &paths, open) == instance::Claim::HandedOver {
            return;
        }
    }

    let mut documents = Vec::with_capacity(paths.len());
    for path in paths {
        let read = match convert::Format::for_import(&path) {
            Some(format) => Document::import(path.clone(), format),
            None => Document::open(path.clone()).map_err(anyhow::Error::from),
        };
        match read {
            Ok(document) => documents.push(document),
            Err(err) => {
                eprintln!("smep: cannot read {}: {err}", path.display());
                std::process::exit(1);
            }
        }
    }
    let mut documents = documents.into_iter();
    let first = documents.next().unwrap_or_else(Document::empty);
    let rest: Vec<Document> = documents.collect();

    let app = gpui_kit::application().with_assets(Assets);
    app.on_open_urls(move |urls| {
        let paths = urls
            .iter()
            .filter_map(|url| io::path_from_file_url(url))
            .collect();
        let _ = opens.unbounded_send(paths);
    });

    app.run(move |cx| {
        gpui_kit::init(cx);
        theme::install(cx);
        keymap::init(cx);
        cx.set_menus(keymap::native_menus());
        // One window is the whole application: with it gone there is no way
        // to open another, so the process ends with it on every platform
        // (gpui's default keeps a macOS app alive without windows).
        cx.set_quit_mode(QuitMode::LastWindowClosed);

        // The window draws its own title bar (menus, document name, window
        // controls); the platform frame is hidden.
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1200.), px(800.)), cx)),
            #[cfg(target_os = "linux")]
            window_decorations: Some(WindowDecorations::Client),
            // The desktop matches the window to `smep.desktop` (and its
            // icon) by this id on Linux.
            app_id: Some("smep".to_string()),
            ..TitleBar::window_options()
        };

        cx.spawn(async move |cx| {
            let mut smep = None;
            let window = cx
                .open_window(options, |window, cx| {
                    let view = cx.new(|cx| {
                        let mut smep = app::Smep::new(first, settings, window, cx);
                        smep.add_documents(rest, window, cx);
                        smep
                    });
                    smep = Some(view.clone());
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("failed to open the smep window");
            let smep = smep.expect("the window builder ran");

            while let Some(paths) = requested.next().await {
                let opened = window.update(cx, |_, window, cx| {
                    smep.update(cx, |this, cx| this.open_paths(paths, window, cx));
                });
                if opened.is_err() {
                    break; // the window is gone
                }
            }
        })
        .detach();
    });
}
