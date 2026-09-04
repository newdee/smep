//! Actions and their default key bindings.

use gpui_kit::component::input::{MoveToEnd, MoveToStart, SelectToEnd, SelectToStart};
use gpui_kit::{App, KeyBinding};

gpui_kit::actions!(smep, [Open, Save, SaveAs]);

/// The key context of the root view; bindings below apply inside it.
pub const CONTEXT: &str = "Smep";

pub fn init(cx: &mut App) {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.bind_keys([
        KeyBinding::new(&format!("{modifier}-o"), Open, Some(CONTEXT)),
        KeyBinding::new(&format!("{modifier}-s"), Save, Some(CONTEXT)),
        KeyBinding::new(&format!("{modifier}-shift-s"), SaveAs, Some(CONTEXT)),
    ]);

    // The editor binds Home/End to the line only. Document start/end are
    // Ctrl+Home/End on Windows and Linux (macOS has Cmd+Up/Down built in).
    // These fire from the focused editor, which handles the actions.
    if !cfg!(target_os = "macos") {
        cx.bind_keys([
            KeyBinding::new("ctrl-home", MoveToStart, Some(CONTEXT)),
            KeyBinding::new("ctrl-end", MoveToEnd, Some(CONTEXT)),
            KeyBinding::new("ctrl-shift-home", SelectToStart, Some(CONTEXT)),
            KeyBinding::new("ctrl-shift-end", SelectToEnd, Some(CONTEXT)),
        ]);
    }
}
