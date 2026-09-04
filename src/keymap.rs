//! Actions and their default key bindings.

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
}
