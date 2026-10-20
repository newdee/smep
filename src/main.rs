//! smep — a Simple Markdown Editor & Previewer, written in Rust.
//!
//! Usage: `smep [FILE]`. With no argument the editor starts empty.
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
Usage: smep [FILE]

A Simple Markdown Editor & Previewer. Opens FILE (Markdown or HTML), or an
empty document. A PDF, Word (.docx) or text FILE is converted to a new,
unsaved Markdown document.

Options:
  -h, --help     Print this help and exit
  -V, --version  Print the version and exit";

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
        _ => {}
    }

    let settings = Settings::load();
    let document = match argument.map(PathBuf::from) {
        Some(path) => {
            let read = match convert::Format::for_import(&path) {
                Some(format) => Document::import(path.clone(), format),
                None => Document::read(path.clone()).map_err(anyhow::Error::from),
            };
            match read {
                Ok(document) => document,
                Err(err) => {
                    eprintln!("smep: cannot read {}: {err}", path.display());
                    std::process::exit(1);
                }
            }
        }
        None => Document::empty(),
    };

    // Files opened through the OS (a double-click in Finder, "Open with")
    // arrive as file:// URLs, possibly before the window exists; they queue
    // here and the window drains the queue once it is up.
    let (opens, mut requested) = futures::channel::mpsc::unbounded::<PathBuf>();
    let app = gpui_kit::application().with_assets(Assets);
    app.on_open_urls(move |urls| {
        for path in urls.iter().filter_map(|url| io::path_from_file_url(url)) {
            let _ = opens.unbounded_send(path);
        }
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
            ..TitleBar::window_options()
        };

        cx.spawn(async move |cx| {
            let mut smep = None;
            let window = cx
                .open_window(options, |window, cx| {
                    let view = cx.new(|cx| app::Smep::new(document, settings, window, cx));
                    smep = Some(view.clone());
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("failed to open the smep window");
            let smep = smep.expect("the window builder ran");

            while let Some(path) = requested.next().await {
                let opened = window.update(cx, |_, window, cx| {
                    smep.update(cx, |this, cx| this.open_document_at(path, window, cx));
                });
                if opened.is_err() {
                    break; // the window is gone
                }
            }
        })
        .detach();
    });
}
