//! smep — a Simple Markdown Editor & Previewer, written in Rust.
//!
//! Usage: `smep [FILE]`. With no argument the editor starts empty.

mod app;
mod insert;
mod io;
mod keymap;

use std::path::PathBuf;

use gpui_kit::assets::Assets;
use gpui_kit::component::Root;
use gpui_kit::*;

use io::Document;

fn main() {
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

    gpui_kit::application().with_assets(Assets).run(move |cx| {
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
            cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| app::Smep::new(document, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("failed to open the smep window");
        })
        .detach();
    });
}
