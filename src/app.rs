//! The root view: a Markdown editor on the left, its live preview on the right.

use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, Theme, h_resizable, resizable_panel};
use gpui_kit::*;

/// The editor pane and the preview pane, kept in sync.
///
/// The editor state owns the text. Every change it emits is copied into
/// `source`, which is what the preview renders.
pub struct Smep {
    editor: Entity<EditorState>,
    source: SharedString,
    _subscriptions: Vec<Subscription>,
}

impl Smep {
    pub fn new(text: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Theme::sync_system_appearance(Some(window), cx);

        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value(text.clone())
                .soft_wrap(true)
                .line_number(true)
                .searchable(true)
                .placeholder("Write Markdown here")
        });

        let subscriptions = vec![
            cx.subscribe_in(
                &editor,
                window,
                |this, editor, event: &InputEvent, _window, cx| {
                    if let InputEvent::Change = event {
                        this.source = editor.read(cx).value();
                        cx.notify();
                    }
                },
            ),
            window.observe_window_appearance(|window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
            }),
        ];

        Self {
            editor,
            source: text.into(),
            _subscriptions: subscriptions,
        }
    }
}

impl Render for Smep {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = div()
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().background)
            .px_6()
            .py_4()
            .child(TextView::markdown("preview", self.source.clone()).scrollable(true));

        h_resizable("smep-split")
            .child(
                resizable_panel().child(
                    Editor::new(&self.editor)
                        .bordered(false)
                        .size_full()
                        .into_any_element(),
                ),
            )
            .child(resizable_panel().child(preview.into_any_element()))
    }
}

#[cfg(test)]
mod tests {
    // Not `use gpui_kit::*`: with `test-support` on, the glob would also pull
    // in GPUI's `test` attribute and shadow the built-in `#[test]`.
    use gpui_kit::TestAppContext;

    use super::Smep;

    #[gpui_kit::test]
    fn an_edit_in_the_editor_reaches_the_preview(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (smep, cx) = cx.add_window_view(|window, cx| Smep::new(String::new(), window, cx));
        let editor = cx.read(|cx| smep.read(cx).editor.clone());

        cx.update(|window, cx| {
            editor.update(cx, |state, cx| state.replace_all("# Hello", window, cx));
        });

        assert_eq!(cx.read(|cx| smep.read(cx).source.to_string()), "# Hello");
    }

    #[gpui_kit::test]
    fn the_initial_text_is_the_preview_source(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (smep, cx) =
            cx.add_window_view(|window, cx| Smep::new("- one\n- two".into(), window, cx));

        assert_eq!(
            cx.read(|cx| smep.read(cx).source.to_string()),
            "- one\n- two"
        );
        assert_eq!(
            cx.read(|cx| smep.read(cx).editor.read(cx).value().to_string()),
            "- one\n- two"
        );
    }
}
