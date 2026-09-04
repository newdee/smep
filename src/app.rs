//! The root view: a Markdown editor on the left, its live preview on the right.

use std::path::{Path, PathBuf};

use gpui_kit::base::POPUP_PRIORITY;
use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, Theme, h_resizable, resizable_panel};
use gpui_kit::*;

use crate::insert;
use crate::io::{self, Document};
use crate::keymap::{self, Open, Save, SaveAs};

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
/// `source`, which is what the preview renders. `saved` is the text as it
/// last was on disk; the document is dirty while the two differ.
pub struct Smep {
    editor: Entity<EditorState>,
    path: Option<PathBuf>,
    format: PreviewFormat,
    source: SharedString,
    saved: SharedString,
    insert_menu: Option<InsertMenu>,
    _subscriptions: Vec<Subscription>,
}

impl Smep {
    pub fn new(document: Document, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Theme::sync_system_appearance(Some(window), cx);

        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value(document.text.clone())
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
                        let was_dirty = this.is_dirty();
                        this.source = editor.read(cx).value();
                        if this.is_dirty() != was_dirty {
                            this.refresh_title(window);
                        }
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

        // Closing with unsaved changes asks first. The platform close is
        // vetoed here and redone once the prompt resolves.
        window.on_window_should_close(cx, {
            let this = cx.weak_entity();
            move |window, cx| {
                let Some(smep) = this.upgrade() else {
                    return true;
                };
                if !smep.read(cx).is_dirty() {
                    return true;
                }
                let proceed = smep.update(cx, |this, cx| this.confirm_discard(window, cx));
                let handle = window.window_handle();
                cx.spawn(async move |cx| {
                    if proceed.await {
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                    }
                })
                .detach();
                false
            }
        });

        let saved: SharedString = document.text.into();
        let this = Self {
            editor,
            path: document.path,
            format: document.format,
            source: saved.clone(),
            saved,
            insert_menu: None,
            _subscriptions: subscriptions,
        };
        this.refresh_title(window);
        this
    }

    /// The window title for a document: `● name — smep` while dirty.
    pub fn title_for(path: Option<&Path>, dirty: bool) -> String {
        let marker = if dirty { "● " } else { "" };
        format!("{marker}{} — smep", Document::display_name(path))
    }

    pub fn is_dirty(&self) -> bool {
        self.source != self.saved
    }

    fn refresh_title(&self, window: &mut Window) {
        window.set_window_title(&Self::title_for(self.path.as_deref(), self.is_dirty()));
    }

    /// Replace the buffer with `document`, as after Open.
    fn load(&mut self, document: Document, window: &mut Window, cx: &mut Context<Self>) {
        self.path = document.path;
        self.format = document.format;
        self.saved = document.text.clone().into();
        self.source = self.saved.clone();
        // `set_value` emits no change event, so `source` is set by hand above.
        self.editor
            .update(cx, |state, cx| state.set_value(document.text, window, cx));
        self.refresh_title(window);
        cx.notify();
    }

    /// Write the buffer to `path` and adopt it as the document's path.
    fn write_to(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let text = self.editor.read(cx).value();
        match io::write(&path, &text) {
            Ok(()) => {
                self.saved = text.clone();
                self.source = text;
                self.format = PreviewFormat::for_path(&path);
                self.path = Some(path);
                self.refresh_title(window);
                cx.notify();
                true
            }
            Err(err) => {
                self.report(
                    &format!("Could not save {}", path.display()),
                    &err.to_string(),
                    window,
                    cx,
                );
                false
            }
        }
    }

    fn report(&self, what: &str, detail: &str, window: &mut Window, cx: &mut Context<Self>) {
        let answered = window.prompt(PromptLevel::Critical, what, Some(detail), &["OK"], cx);
        cx.spawn(async move |_, _| {
            let _ = answered.await;
        })
        .detach();
    }

    /// Save to the current path, or ask for one. Resolves to whether it saved.
    fn save_task(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        match self.path.clone() {
            Some(path) => Task::ready(self.write_to(path, window, cx)),
            None => self.save_as_task(window, cx),
        }
    }

    /// Ask for a path, then save there. Resolves to whether it saved.
    fn save_as_task(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        let directory = self
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let suggested = self
            .path
            .as_deref()
            .map(|path| Document::display_name(Some(path)))
            .unwrap_or_else(|| "untitled.md".to_string());
        let chosen = cx.prompt_for_new_path(&directory, Some(&suggested));

        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return false;
            };
            this.update_in(cx, |this, window, cx| this.write_to(path, window, cx))
                .unwrap_or(false)
        })
    }

    /// Ask before discarding unsaved changes. Resolves to whether to proceed.
    fn confirm_discard(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        if !self.is_dirty() {
            return Task::ready(true);
        }
        let detail = format!(
            "{} has unsaved changes.",
            Document::display_name(self.path.as_deref())
        );
        let answer = window.prompt(
            PromptLevel::Warning,
            "Save changes?",
            Some(&detail),
            &["Save", "Don't Save", "Cancel"],
            cx,
        );

        cx.spawn_in(window, async move |this, cx| match answer.await {
            Ok(0) => match this.update_in(cx, |this, window, cx| this.save_task(window, cx)) {
                Ok(saved) => saved.await,
                Err(_) => false,
            },
            Ok(1) => true,
            _ => false,
        })
    }

    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        let proceed = self.confirm_discard(window, cx);
        cx.spawn_in(window, async move |this, cx| {
            if !proceed.await {
                return;
            }
            let Ok(chosen) = this.update(cx, |_, cx| {
                cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                })
            }) else {
                return;
            };
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let document = Document::read(path.clone());
            let _ = this.update_in(cx, |this, window, cx| match document {
                Ok(document) => this.load(document, window, cx),
                Err(err) => this.report(
                    &format!("Could not open {}", path.display()),
                    &err.to_string(),
                    window,
                    cx,
                ),
            });
        })
        .detach();
    }

    fn save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        self.save_task(window, cx).detach();
    }

    fn save_as(&mut self, _: &SaveAs, window: &mut Window, cx: &mut Context<Self>) {
        self.save_as_task(window, cx).detach();
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
            .key_context(keymap::CONTEXT)
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::save_as))
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
    use std::path::{Path, PathBuf};

    use gpui_kit::component::input::{EditorState, Position};
    use gpui_kit::{Entity, TestAppContext, VisualTestContext};

    use super::{PreviewFormat, Smep};
    use crate::insert::{self, BlockKind};
    use crate::io::Document;
    use crate::keymap::{Open, Save};

    fn open<'a>(
        cx: &'a mut TestAppContext,
        text: &str,
    ) -> (Entity<Smep>, &'a mut VisualTestContext) {
        open_document(
            cx,
            Document {
                path: None,
                text: text.to_string(),
                format: PreviewFormat::Markdown,
            },
        )
    }

    fn open_document(
        cx: &mut TestAppContext,
        document: Document,
    ) -> (Entity<Smep>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::keymap::init(cx);
        });
        cx.add_window_view(|window, cx| Smep::new(document, window, cx))
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

    fn dirty(smep: &Entity<Smep>, cx: &VisualTestContext) -> bool {
        cx.read(|cx| smep.read(cx).is_dirty())
    }

    fn type_text(editor: &Entity<EditorState>, text: &str, cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| state.replace_all(text, window, cx));
        });
    }

    /// A fresh directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("smep-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[gpui_kit::test]
    fn an_edit_in_the_editor_reaches_the_preview(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        type_text(&editor, "# Hello", cx);

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
        assert!(!dirty(&smep, cx));
    }

    #[gpui_kit::test]
    fn editing_marks_the_document_dirty_and_saving_clears_it(cx: &mut TestAppContext) {
        let dir = TempDir::new("save");
        let path = dir.path("notes.md");
        std::fs::write(&path, "before").unwrap();
        let (smep, cx) = open_document(cx, Document::read(path.clone()).unwrap());
        let editor = editor(&smep, cx);
        assert!(!dirty(&smep, cx));

        type_text(&editor, "after\r\n", cx);
        assert!(dirty(&smep, cx));
        assert_eq!(
            cx.read(|cx| Smep::title_for(smep.read(cx).path.as_deref(), true)),
            "● notes.md — smep"
        );

        cx.update(|window, cx| smep.update(cx, |this, cx| this.save(&Save, window, cx)));
        cx.run_until_parked();

        assert!(!dirty(&smep, cx));
        assert_eq!(std::fs::read(&path).unwrap(), b"after\r\n");
        // Undoing back to the saved text is clean again; one more edit is not.
        type_text(&editor, "before", cx);
        assert!(dirty(&smep, cx));
    }

    #[gpui_kit::test]
    async fn saving_an_untitled_document_asks_for_a_path(cx: &mut TestAppContext) {
        let dir = TempDir::new("save-as");
        let target = dir.path("chosen.md");
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);
        type_text(&editor, "fresh", cx);

        // The dialog is pending only once the task has asked for it.
        let saved = cx.update(|window, cx| smep.update(cx, |this, cx| this.save_task(window, cx)));
        cx.simulate_new_path_selection({
            let target = target.clone();
            move |_| Some(target)
        });

        assert!(saved.await);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "fresh");
        assert!(!dirty(&smep, cx));
        assert_eq!(cx.read(|cx| smep.read(cx).path.clone()), Some(target));
    }

    #[gpui_kit::test]
    async fn cancelling_save_as_leaves_the_document_dirty(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);
        type_text(&editor, "fresh", cx);

        let saved = cx.update(|window, cx| smep.update(cx, |this, cx| this.save_task(window, cx)));
        cx.simulate_new_path_selection(|_| None);

        assert!(!saved.await);
        assert!(dirty(&smep, cx));
        assert_eq!(cx.read(|cx| smep.read(cx).path.clone()), None);
    }

    #[gpui_kit::test]
    async fn discarding_asks_only_while_dirty(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        let clean =
            cx.update(|window, cx| smep.update(cx, |this, cx| this.confirm_discard(window, cx)));
        assert!(clean.await, "a clean document needs no prompt");
        assert!(!cx.has_pending_prompt());

        type_text(&editor, "unsaved", cx);
        let proceed =
            cx.update(|window, cx| smep.update(cx, |this, cx| this.confirm_discard(window, cx)));
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        assert!(!proceed.await, "cancel keeps the document");

        let proceed =
            cx.update(|window, cx| smep.update(cx, |this, cx| this.confirm_discard(window, cx)));
        cx.simulate_prompt_answer("Don't Save");
        assert!(proceed.await, "don't save proceeds without writing");
        assert!(dirty(&smep, cx));
    }

    #[gpui_kit::test]
    fn open_replaces_the_buffer_and_resets_dirty(cx: &mut TestAppContext) {
        let dir = TempDir::new("open");
        let path = dir.path("other.html");
        std::fs::write(&path, "<b>bold</b>").unwrap();
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        cx.update(|window, cx| smep.update(cx, |this, cx| this.open(&Open, window, cx)));
        // The task asks for the file only after the (clean) discard check resolves.
        cx.run_until_parked();
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response({
            let path = path.clone();
            move |_| Some(vec![path])
        });
        cx.run_until_parked();

        assert_eq!(value(&editor, cx), "<b>bold</b>");
        assert_eq!(
            cx.read(|cx| smep.read(cx).source.to_string()),
            "<b>bold</b>"
        );
        assert_eq!(cx.read(|cx| smep.read(cx).format), PreviewFormat::Html);
        assert_eq!(cx.read(|cx| smep.read(cx).path.clone()), Some(path));
        assert!(!dirty(&smep, cx));
    }

    #[gpui_kit::test]
    fn ctrl_s_reaches_the_save_action(cx: &mut TestAppContext) {
        let dir = TempDir::new("keys");
        let path = dir.path("keys.md");
        std::fs::write(&path, "").unwrap();
        let (smep, cx) = open_document(cx, Document::read(path.clone()).unwrap());
        let editor = editor(&smep, cx);
        cx.update(|window, cx| editor.update(cx, |state, cx| state.focus(window, cx)));
        draw(cx);
        type_text(&editor, "typed", cx);
        assert!(dirty(&smep, cx));

        let save = if cfg!(target_os = "macos") {
            "cmd-s"
        } else {
            "ctrl-s"
        };
        cx.simulate_keystrokes(save);
        cx.run_until_parked();

        assert!(!dirty(&smep, cx));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "typed");
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

    #[test]
    fn titles_show_the_name_and_a_dirty_marker() {
        assert_eq!(Smep::title_for(None, false), "Untitled — smep");
        assert_eq!(
            Smep::title_for(Some(Path::new("/x/a.md")), true),
            "● a.md — smep"
        );
    }
}
