//! The root view: a Markdown editor on the left, its live preview on the right.

use std::path::Path;

use gpui_kit::base::POPUP_PRIORITY;
use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, Theme, h_resizable, resizable_panel};
use gpui_kit::*;

use crate::insert;

/// How the preview interprets the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewFormat {
    Markdown,
    Html,
}

impl PreviewFormat {
    pub fn for_path(path: &Path) -> Self {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm") => {
                Self::Html
            }
            _ => Self::Markdown,
        }
    }
}

/// The insert menu while it is open, anchored under the cursor's line.
struct InsertMenu {
    menu: Entity<PopupMenu>,
    position: Point<Pixels>,
    _dismiss: Subscription,
}

/// The editor pane and the preview pane, kept in sync.
///
/// The editor state owns the text. Every change it emits is copied into
/// `source`, which is what the preview renders.
pub struct Smep {
    editor: Entity<EditorState>,
    format: PreviewFormat,
    source: SharedString,
    insert_menu: Option<InsertMenu>,
    _subscriptions: Vec<Subscription>,
}

impl Smep {
    pub fn new(
        text: String,
        format: PreviewFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Theme::sync_system_appearance(Some(window), cx);

        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value(text.clone())
                .soft_wrap(true)
                .line_number(false)
                .searchable(true)
                .placeholder("Write here. Type / on an empty line to insert a block.")
        });
        editor.update(cx, |state, cx| state.focus(window, cx));

        let subscriptions = vec![
            cx.subscribe_in(
                &editor,
                window,
                |this, editor, event: &InputEvent, window, cx| {
                    if let InputEvent::Change = event {
                        this.source = editor.read(cx).value();
                        if Self::slash_typed(editor.read(cx)) {
                            this.open_insert_menu(window, cx);
                        }
                        cx.notify();
                    }
                },
            ),
            // Cursor moves do not emit events; observe so the "+" follows the cursor.
            cx.observe(&editor, |_, _, cx| cx.notify()),
            window.observe_window_appearance(|window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
            }),
        ];

        Self {
            editor,
            format,
            source: text.into(),
            insert_menu: None,
            _subscriptions: subscriptions,
        }
    }

    /// A lone `/` on the cursor's line, with the cursor after it.
    fn slash_typed(state: &EditorState) -> bool {
        insert::cursor_line_trimmed(state) == "/"
            && state.cursor() == insert::cursor_line_range(state).end
    }

    /// Window-space bounds of the cursor's line, once the editor has laid out.
    fn cursor_line_bounds(&self, cx: &App) -> Option<Bounds<Pixels>> {
        let state = self.editor.read(cx);
        let line = insert::cursor_line_range(state);
        state.range_to_bounds(&(line.start..line.start))
    }

    /// Open the block menu under the cursor's line.
    pub fn open_insert_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.insert_menu.is_some() {
            return;
        }
        let Some(bounds) = self.cursor_line_bounds(cx) else {
            return;
        };

        let editor = self.editor.clone();
        let editor_focus = editor.focus_handle(cx);
        let menu = PopupMenu::build(window, cx, |mut menu, _, _| {
            for entry in insert::MENU {
                menu = match entry {
                    Some(kind) => {
                        let kind = *kind;
                        let editor = editor.clone();
                        menu.item(PopupMenuItem::new(kind.label()).on_click(
                            move |_, window, cx| {
                                editor.update(cx, |state, cx| {
                                    insert::insert_block(state, kind, window, cx);
                                });
                            },
                        ))
                    }
                    None => menu.separator(),
                };
            }
            menu.action_context(editor_focus)
        });

        let dismiss = cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _, cx| {
            this.insert_menu = None;
            cx.notify();
        });

        self.insert_menu = Some(InsertMenu {
            menu,
            position: bounds.bottom_left(),
            _dismiss: dismiss,
        });
        cx.notify();
    }

    /// The "+" beside an empty line while the editor has focus.
    fn render_plus(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.insert_menu.is_some() {
            return None;
        }
        if !self.editor.focus_handle(cx).is_focused(window) {
            return None;
        }
        let state = self.editor.read(cx);
        if !insert::cursor_line_trimmed(state).is_empty() {
            return None;
        }
        let bounds = self.cursor_line_bounds(cx)?;

        let size = px(18.);
        let origin = point(
            bounds.left() - px(24.),
            bounds.top() + (bounds.size.height - size) / 2.,
        );
        let border = cx.theme().border;
        let color = cx.theme().muted_foreground;
        let hover = cx.theme().muted;

        let plus = div()
            .id("insert-plus")
            .size(size)
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(border)
            .text_color(color)
            .text_sm()
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .child("+")
            .on_click(cx.listener(|this, _, window, cx| this.open_insert_menu(window, cx)));

        Some(
            deferred(anchored().position(origin).child(plus))
                .with_priority(1)
                .into_any_element(),
        )
    }

    /// The open insert menu, focused so the keyboard drives it.
    fn render_insert_menu(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let open = self.insert_menu.as_ref()?;
        let focus = open.menu.focus_handle(cx);
        if !focus.contains_focused(window, cx) {
            focus.focus(window, cx);
        }
        Some(
            deferred(
                anchored()
                    .position(open.position)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(open.menu.clone()),
            )
            .with_priority(POPUP_PRIORITY)
            .into_any_element(),
        )
    }
}

impl Render for Smep {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text_view = match self.format {
            PreviewFormat::Markdown => TextView::markdown("preview", self.source.clone()),
            PreviewFormat::Html => TextView::html("preview", self.source.clone()),
        };
        let preview = div()
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().background)
            .px_6()
            .py_4()
            .child(text_view.scrollable(true));

        let plus = self.render_plus(window, cx);
        let menu = self.render_insert_menu(window, cx);

        div()
            .size_full()
            .child(
                h_resizable("smep-split")
                    .child(
                        resizable_panel().child(
                            Editor::new(&self.editor)
                                .bordered(false)
                                .size_full()
                                .pl(px(28.))
                                .into_any_element(),
                        ),
                    )
                    .child(resizable_panel().child(preview.into_any_element())),
            )
            .children(plus)
            .children(menu)
    }
}

#[cfg(test)]
mod tests {
    // Not `use gpui_kit::*`: with `test-support` on, the glob would also pull
    // in GPUI's `test` attribute and shadow the built-in `#[test]`.
    use gpui_kit::component::input::{EditorState, Position};
    use gpui_kit::{Entity, TestAppContext, VisualTestContext};

    use super::{PreviewFormat, Smep};
    use crate::insert::{self, BlockKind};

    fn open<'a>(
        cx: &'a mut TestAppContext,
        text: &str,
    ) -> (Entity<Smep>, &'a mut VisualTestContext) {
        cx.update(gpui_kit::init);
        cx.add_window_view(|window, cx| Smep::new(text.into(), PreviewFormat::Markdown, window, cx))
    }

    fn editor(smep: &Entity<Smep>, cx: &VisualTestContext) -> Entity<EditorState> {
        cx.read(|cx| smep.read(cx).editor.clone())
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }

    fn value(editor: &Entity<EditorState>, cx: &VisualTestContext) -> String {
        cx.read(|cx| editor.read(cx).value().to_string())
    }

    fn menu_open(smep: &Entity<Smep>, cx: &VisualTestContext) -> bool {
        cx.read(|cx| smep.read(cx).insert_menu.is_some())
    }

    #[gpui_kit::test]
    fn an_edit_in_the_editor_reaches_the_preview(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        cx.update(|window, cx| {
            editor.update(cx, |state, cx| state.replace_all("# Hello", window, cx));
        });

        assert_eq!(cx.read(|cx| smep.read(cx).source.to_string()), "# Hello");
    }

    #[gpui_kit::test]
    fn the_initial_text_is_the_preview_source(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "- one\n- two");
        let editor = editor(&smep, cx);

        assert_eq!(
            cx.read(|cx| smep.read(cx).source.to_string()),
            "- one\n- two"
        );
        assert_eq!(value(&editor, cx), "- one\n- two");
    }

    #[gpui_kit::test]
    fn a_slash_on_an_empty_line_opens_the_menu_and_escape_closes_it(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| editor.update(cx, |state, cx| state.focus(window, cx)));
        draw(cx);

        cx.simulate_input("/");
        assert!(
            menu_open(&smep, cx),
            "typing / on an empty line opens the menu"
        );

        draw(cx);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!menu_open(&smep, cx), "escape closes the menu");
        assert_eq!(value(&editor, cx), "/", "escape keeps what was typed");
    }

    #[gpui_kit::test]
    fn a_slash_inside_text_does_not_open_the_menu(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "a");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.focus(window, cx);
                state.set_cursor_position(Position::new(0, 1), window, cx);
            });
        });
        draw(cx);

        cx.simulate_input("/");
        assert_eq!(value(&editor, cx), "a/");
        assert!(!menu_open(&smep, cx));
    }

    #[gpui_kit::test]
    fn choosing_the_first_entry_replaces_the_line(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "intro\n\nend");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.focus(window, cx);
                state.set_cursor_position(Position::new(1, 0), window, cx);
            });
        });
        draw(cx);

        cx.update(|window, cx| smep.update(cx, |this, cx| this.open_insert_menu(window, cx)));
        assert!(menu_open(&smep, cx));
        draw(cx);

        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();

        assert_eq!(value(&editor, cx), "intro\n# \nend");
        assert_eq!(cx.read(|cx| editor.read(cx).cursor()), "intro\n# ".len());
        assert!(!menu_open(&smep, cx), "the menu closes after a choice");
        assert_eq!(
            cx.read(|cx| smep.read(cx).source.to_string()),
            "intro\n# \nend",
            "the preview sees the inserted block"
        );
    }

    #[gpui_kit::test]
    fn insert_block_keeps_crlf_line_endings(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "a\r\n\r\nb");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_cursor_position(Position::new(1, 0), window, cx);
                insert::insert_block(state, BlockKind::Heading2, window, cx);
            });
        });

        assert_eq!(value(&editor, cx), "a\r\n## \r\nb");
        assert_eq!(cx.read(|cx| editor.read(cx).cursor()), "a\r\n## ".len());
    }

    #[gpui_kit::test]
    fn multi_line_snippets_put_the_cursor_inside(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "/");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_cursor_position(Position::new(0, 1), window, cx);
                insert::insert_block(state, BlockKind::CodeBlock, window, cx);
            });
        });

        assert_eq!(value(&editor, cx), "```\n\n```");
        assert_eq!(cx.read(|cx| editor.read(cx).cursor()), "```\n".len());
    }

    #[test]
    fn html_files_get_the_html_preview() {
        use std::path::Path;
        assert_eq!(
            PreviewFormat::for_path(Path::new("a.html")),
            PreviewFormat::Html
        );
        assert_eq!(
            PreviewFormat::for_path(Path::new("a.HTM")),
            PreviewFormat::Html
        );
        assert_eq!(
            PreviewFormat::for_path(Path::new("a.md")),
            PreviewFormat::Markdown
        );
        assert_eq!(
            PreviewFormat::for_path(Path::new("README")),
            PreviewFormat::Markdown
        );
    }
}
