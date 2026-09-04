//! smep — a Simple Markdown Editor & Previewer, written in Rust.
//!
//! Usage: `smep [FILE]`. With no argument the editor starts empty.

mod app;

use std::path::PathBuf;

use gpui_kit::assets::Assets;
use gpui_kit::component::Root;
use gpui_kit::*;

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let (title, text) = match &path {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => (window_title(path), text),
            Err(err) => {
                eprintln!("smep: cannot read {}: {err}", path.display());
                std::process::exit(1);
            }
        },
        None => ("smep".to_string(), String::new()),
    };

    gpui_kit::application().with_assets(Assets).run(move |cx| {
        gpui_kit::init(cx);

        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::centered(size(px(1200.), px(800.)), cx)),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| app::Smep::new(text, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("failed to open the smep window");
        })
        .detach();
    });
}

fn window_title(path: &std::path::Path) -> String {
    match path.file_name() {
        Some(name) => format!("{} — smep", name.to_string_lossy()),
        None => "smep".to_string(),
    }
}
