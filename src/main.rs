//! smep — a Simple Markdown Editor & Previewer, written in Rust.
//!
//! Usage: `smep [FILE]`. With no argument the editor starts empty.

mod app;
mod highlight;
mod insert;
mod io;
mod keymap;
mod settings;
mod theme;

use std::path::PathBuf;

use futures::StreamExt as _;
use gpui_kit::assets::Assets;
use gpui_kit::component::Root;
use gpui_kit::*;

use io::Document;
use settings::Settings;

fn main() {
    let settings = Settings::load();
    let document = match std::env::args_os().nth(1).map(PathBuf::from) {
        Some(path) => match Document::read(path.clone()) {
            Ok(document) => document,
            Err(err) => {
                eprintln!("smep: cannot read {}: {err}", path.display());
                std::process::exit(1);
            }
        },
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
        keymap::init(cx);

        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(app::Smep::title_for(document.path.as_deref(), false).into()),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::centered(size(px(1200.), px(800.)), cx)),
            ..Default::default()
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
