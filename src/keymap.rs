//! Actions, their default key bindings, and the native (macOS) menu bar.

use gpui_kit::component::input::{
    Copy, Cut, MoveToEnd, MoveToStart, Paste, Redo, Replace, Search, SelectAll, SelectToEnd,
    SelectToStart, Undo,
};
use gpui_kit::{App, KeyBinding, Menu, MenuItem};

gpui_kit::actions!(
    smep,
    [
        Open,
        Save,
        SaveAs,
        Quit,
        ToggleMenuBar,
        ToggleFullscreen,
        ViewSource,
        ViewSplit,
        ViewRendered
    ]
);

/// The key context of the root view; bindings below apply inside it.
pub const CONTEXT: &str = "Smep";

const MAC: bool = cfg!(target_os = "macos");

/// `cmd` on macOS, `ctrl` elsewhere.
fn primary(keys: &str) -> String {
    format!("{}-{keys}", if MAC { "cmd" } else { "ctrl" })
}

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(&primary("o"), Open, Some(CONTEXT)),
        KeyBinding::new(&primary("s"), Save, Some(CONTEXT)),
        KeyBinding::new(&primary("shift-s"), SaveAs, Some(CONTEXT)),
        KeyBinding::new(&primary("shift-m"), ToggleMenuBar, Some(CONTEXT)),
        KeyBinding::new(&primary("1"), ViewSource, Some(CONTEXT)),
        KeyBinding::new(&primary("2"), ViewSplit, Some(CONTEXT)),
        KeyBinding::new(&primary("3"), ViewRendered, Some(CONTEXT)),
    ]);

    if MAC {
        cx.bind_keys([
            KeyBinding::new("ctrl-cmd-f", ToggleFullscreen, Some(CONTEXT)),
            KeyBinding::new("cmd-q", Quit, Some(CONTEXT)),
        ]);
    } else {
        cx.bind_keys([
            KeyBinding::new("f11", ToggleFullscreen, Some(CONTEXT)),
            // The editor binds Home/End to the line only. Document start/end
            // are Ctrl+Home/End here (macOS has Cmd+Up/Down built in).
            // These fire from the focused editor, which handles the actions.
            KeyBinding::new("ctrl-home", MoveToStart, Some(CONTEXT)),
            KeyBinding::new("ctrl-end", MoveToEnd, Some(CONTEXT)),
            KeyBinding::new("ctrl-shift-home", SelectToStart, Some(CONTEXT)),
            KeyBinding::new("ctrl-shift-end", SelectToEnd, Some(CONTEXT)),
        ]);
    }
}

/// The application menu bar. macOS shows it at the top of the screen; the
/// other platforms ignore it and use the menu bar drawn in the window.
pub fn native_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "smep".into(),
            items: vec![MenuItem::action("Quit smep", Quit)],
            disabled: false,
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open…", Open),
                MenuItem::action("Save", Save),
                MenuItem::action("Save As…", SaveAs),
            ],
            disabled: false,
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
                MenuItem::action("Cut", Cut),
                MenuItem::action("Copy", Copy),
                MenuItem::action("Paste", Paste),
                MenuItem::action("Select All", SelectAll),
                MenuItem::separator(),
                MenuItem::action("Find", Search),
                MenuItem::action("Replace", Replace),
            ],
            disabled: false,
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Source", ViewSource),
                MenuItem::action("Split", ViewSplit),
                MenuItem::action("Rendered", ViewRendered),
                MenuItem::separator(),
                MenuItem::action("Toggle Menu Bar", ToggleMenuBar),
                MenuItem::action("Toggle Full Screen", ToggleFullscreen),
            ],
            disabled: false,
        },
    ]
}
