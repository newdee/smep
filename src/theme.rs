//! How smep looks.
//!
//! The application itself (editor, title bar, menus) is drawn in Paper:
//! ink on paper, light or dark with the system, the look of dfine-blog's
//! "墨色编辑部" (warm white or near-black paper, grey inks, one accent,
//! cinnabar). They are gpui-kit theme configs in `assets/themes/paper.json`,
//! installed by [`install`].
//!
//! The rendered Markdown can wear a look of its own, a [`PreviewTheme`];
//! by default it follows the application. Whatever the colours, the type is
//! one design, the blog's article page: its heading scale (a fading rule
//! under h2, a cinnabar square before h3 and h4), its paragraph gap, code
//! on a board (a thick frame on light paper, a thin one with a cinnabar top
//! edge on dark, the language in a cinnabar tab), tables ruled across with
//! a heavier line under the header, quotes on a wash. The same design
//! dresses the window ([`Palette::text_view_style`]), the exported HTML page
//! and the PDF (`export::stylesheet`, `export::preamble`); the window draws
//! what its renderer has a hook for.

use gpui_kit::App;
use gpui_kit::base::text::TextViewStyle;
use gpui_kit::component::{Theme, ThemeRegistry};
use gpui_kit::{
    BoxShadow, FontWeight, HighlightStyle, Hsla, Pixels, Rgba, StyleRefinement, Styled as _, hsla,
    px, rems, rgb,
};
use serde::{Deserialize, Serialize};

/// The two application themes, in gpui-kit's JSON theme format.
pub const PAPER_THEMES: &str = include_str!("../assets/themes/paper.json");

/// The theme names in [`PAPER_THEMES`].
pub const PAPER_LIGHT: &str = "Paper";
pub const PAPER_DARK: &str = "Paper Dark";

/// Make Paper the application's light theme and Paper Dark its dark one.
/// Call once after `gpui_kit::init`; `Theme::sync_system_appearance` then
/// picks the one the system asks for.
pub fn install(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    registry
        .load_themes_from_str(PAPER_THEMES)
        .expect("the bundled theme file is valid");
    let light = registry.themes()[PAPER_LIGHT].clone();
    let dark = registry.themes()[PAPER_DARK].clone();
    let mode = {
        let theme = Theme::global_mut(cx);
        theme.light_theme = light;
        theme.dark_theme = dark;
        theme.mode
    };
    Theme::change(mode, None, cx);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewTheme {
    /// Follow the application: Paper, light or dark with the system.
    #[default]
    System,
    /// Paper's light look, whatever the system says.
    Paper,
    /// Paper's dark look, whatever the system says.
    PaperDark,
    Github,
    Newsprint,
    Night,
    Sepia,
    SolarizedLight,
    SolarizedDark,
}

/// Every theme, in menu order.
pub const ALL: [PreviewTheme; 9] = [
    PreviewTheme::System,
    PreviewTheme::Paper,
    PreviewTheme::PaperDark,
    PreviewTheme::Github,
    PreviewTheme::Newsprint,
    PreviewTheme::Night,
    PreviewTheme::Sepia,
    PreviewTheme::SolarizedLight,
    PreviewTheme::SolarizedDark,
];

/// The colours and type of one theme, named after the blog's tokens where
/// they differ from the obvious.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// The theme's name, for the comment at the top of a stylesheet.
    pub name: &'static str,
    /// The paper.
    pub background: Hsla,
    /// The ink.
    pub foreground: Hsla,
    /// Soft ink: quotes, the smallest headings.
    pub muted: Hsla,
    /// Faint ink: list markers, the rule under a table's header, the
    /// section break.
    pub faint: Hsla,
    /// The accent: links, the marks before h3 and h4, the code tab.
    pub link: Hsla,
    /// A shade under the paper: the quote's wash.
    pub wash: Hsla,
    /// Inline code's background.
    pub code_background: Hsla,
    /// The code board's surface.
    pub code_surface: Hsla,
    /// The code board's frame.
    pub code_frame: Hsla,
    /// The band along the top of a code board (the surface itself on dark
    /// paper, where the band melts into the board).
    pub code_rail: Hsla,
    /// Hairlines: table rows, the quote's edge, the rule under h2.
    pub border: Hsla,
    pub serif: bool,
    pub dark: bool,
}

/// The blog's heading scale, as a factor of the body size: h1 to h6.
const HEADING_SCALE: [f32; 6] = [1.62, 1.46, 1.2, 1.02, 0.92, 0.92];

/// The blog's gap between paragraphs, in rems of the body size.
const PARAGRAPH_GAP: f32 = 0.95;

/// Code in blocks, as a factor of the body size.
pub const CODE_SCALE: f32 = 0.85;

/// Code's line height in blocks.
pub const CODE_LEADING: f32 = 1.7;

/// The room between h2's text and the rule under it, in rems.
pub const HEADING_RULE_GAP: f32 = 0.5;

/// The code board's frame, in pixels at 16 px of body text: thick on light
/// paper (a whiteboard's aluminium edge), a hairline on dark.
pub const CODE_FRAME_LIGHT: f32 = 5.;
pub const CODE_FRAME_DARK: f32 = 1.;

/// The cinnabar top edge of a code board on dark paper, in pixels.
pub const CODE_TOP_EDGE: f32 = 2.;

/// The leading of running text: the blog's 1.85, a reader's loose page
/// rather than a dense screen.
pub const READING_LEADING: f32 = 1.85;

/// The page being read: its width, and the paper either side of the text
/// in it, which leaves the blog's 44 rem column.
pub const PAGE_WIDTH: Pixels = px(800.);
pub const PAGE_MARGIN: Pixels = px(48.);

/// The column of text that leaves: 44 rem at 16 px.
pub const MEASURE: Pixels = px(704.);

impl PreviewTheme {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System (Paper)",
            Self::Paper => "Paper",
            Self::PaperDark => "Paper Dark",
            Self::Github => "GitHub",
            Self::Newsprint => "Newsprint",
            Self::Night => "Night",
            Self::Sepia => "Sepia",
            Self::SolarizedLight => "Solarized Light",
            Self::SolarizedDark => "Solarized Dark",
        }
    }

    /// The name on the command line and in `settings.toml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Paper => "paper",
            Self::PaperDark => "paper-dark",
            Self::Github => "github",
            Self::Newsprint => "newsprint",
            Self::Night => "night",
            Self::Sepia => "sepia",
            Self::SolarizedLight => "solarized-light",
            Self::SolarizedDark => "solarized-dark",
        }
    }

    /// The theme called `name` (see [`Self::name`]).
    pub fn parse(name: &str) -> Option<Self> {
        ALL.into_iter().find(|theme| theme.name() == name)
    }

    /// The light theme of the same family: a dark theme's light counterpart,
    /// a light theme itself. A PDF is printed, and print is ink on paper.
    pub fn light(self) -> Self {
        match self {
            Self::System | Self::PaperDark => Self::Paper,
            Self::Night => Self::Github,
            Self::SolarizedDark => Self::SolarizedLight,
            other => other,
        }
    }

    /// The fixed palette, or `None` for the theme that follows the app.
    pub fn palette(self) -> Option<Palette> {
        let hex = |value: u32| -> Hsla { rgb(value).into() };
        let name = self.label();
        Some(match self {
            Self::System => return None,
            // The blog's light paper (src/styles/tokens.css): ink #1a1a18
            // on #f8f7f3, cinnabar, code on a whiteboard in an aluminium
            // frame under a grey band.
            Self::Paper => Palette {
                name,
                background: hex(0xf8f7f3),
                foreground: hex(0x1a1a18),
                muted: hex(0x5d5b55),
                faint: hex(0x97948c),
                link: hex(0xc3423f),
                wash: hex(0xf1efeb),
                code_background: hex(0xf4f2ee),
                code_surface: hex(0xfdfdfb),
                code_frame: hex(0xcfccc4),
                code_rail: hex(0xeceae4),
                border: hex(0xe4e1da),
                serif: false,
                dark: false,
            },
            // The blog's dark paper: ink #c1bfba on #121212, a brighter
            // cinnabar so links clear AA, the code board's band melted into
            // the board.
            Self::PaperDark => Palette {
                name,
                background: hex(0x121212),
                foreground: hex(0xc1bfba),
                muted: hex(0xa5a29a),
                faint: hex(0x87837b),
                link: hex(0xdd625e),
                wash: hex(0x171716),
                code_background: hex(0x191918),
                code_surface: hex(0x171716),
                code_frame: hex(0x2b2a28),
                code_rail: hex(0x171716),
                border: hex(0x2b2a28),
                serif: false,
                dark: true,
            },
            Self::Github => Palette {
                name,
                background: hex(0xffffff),
                foreground: hex(0x1f2328),
                muted: hex(0x59636e),
                faint: hex(0x818b98),
                link: hex(0x0969da),
                wash: hex(0xf6f8fa),
                code_background: hex(0xf6f8fa),
                code_surface: hex(0xf6f8fa),
                code_frame: hex(0xd1d9e0),
                code_rail: hex(0xeaeef2),
                border: hex(0xd1d9e0),
                serif: false,
                dark: false,
            },
            Self::Newsprint => Palette {
                name,
                background: hex(0xf3f2ee),
                foreground: hex(0x1f0909),
                muted: hex(0x6a6560),
                faint: hex(0x9a958f),
                link: hex(0x8b3a10),
                wash: hex(0xebeae5),
                code_background: hex(0xe7e6e1),
                code_surface: hex(0xf9f8f5),
                code_frame: hex(0xc9c8c3),
                code_rail: hex(0xe7e6e1),
                border: hex(0xc9c8c3),
                serif: true,
                dark: false,
            },
            Self::Night => Palette {
                name,
                background: hex(0x363b40),
                foreground: hex(0xd9d9d9),
                muted: hex(0x9aa0a6),
                faint: hex(0x7d848b),
                link: hex(0x7fb2f0),
                wash: hex(0x30353a),
                code_background: hex(0x2b2f33),
                code_surface: hex(0x2e3236),
                code_frame: hex(0x4b5259),
                code_rail: hex(0x2e3236),
                border: hex(0x4b5259),
                serif: false,
                dark: true,
            },
            Self::Sepia => Palette {
                name,
                background: hex(0xf4ecd8),
                foreground: hex(0x5b4636),
                muted: hex(0x8b7355),
                faint: hex(0xa8916f),
                link: hex(0x9c5a1e),
                wash: hex(0xede3cb),
                code_background: hex(0xeadfc3),
                code_surface: hex(0xfaf4e6),
                code_frame: hex(0xd0bf98),
                code_rail: hex(0xeadfc3),
                border: hex(0xd9c9a3),
                serif: true,
                dark: false,
            },
            Self::SolarizedLight => Palette {
                name,
                background: hex(0xfdf6e3),
                foreground: hex(0x586e75),
                muted: hex(0x93a1a1),
                faint: hex(0xa3aea9),
                link: hex(0x268bd2),
                wash: hex(0xf5eedb),
                code_background: hex(0xeee8d5),
                code_surface: hex(0xfffbf0),
                code_frame: hex(0xd6cfb8),
                code_rail: hex(0xeee8d5),
                border: hex(0xd6cfb8),
                serif: false,
                dark: false,
            },
            Self::SolarizedDark => Palette {
                name,
                background: hex(0x002b36),
                foreground: hex(0x93a1a1),
                muted: hex(0x657b83),
                faint: hex(0x586e75),
                link: hex(0x268bd2),
                wash: hex(0x03313d),
                code_background: hex(0x073642),
                code_surface: hex(0x01303b),
                code_frame: hex(0x0f4a5a),
                code_rail: hex(0x01303b),
                border: hex(0x0f4a5a),
                serif: false,
                dark: true,
            },
        })
    }

    /// The palette to draw with: the fixed one, or the application's own
    /// colours for the theme that follows it.
    pub fn resolve(self, theme: &Theme) -> Palette {
        self.palette().unwrap_or_else(|| Palette::from_app(theme))
    }

    /// The palette a PDF is printed with: the light theme of this family.
    pub fn print_palette(self) -> Palette {
        self.light()
            .palette()
            .expect("the light theme of a family is a fixed one")
    }
}

impl Palette {
    /// The palette of the preview that follows the application: the
    /// application wears Paper, so it is Paper or Paper Dark with the mode.
    pub fn from_app(theme: &Theme) -> Self {
        let paper = if theme.mode.is_dark() {
            PreviewTheme::PaperDark
        } else {
            PreviewTheme::Paper
        };
        paper.palette().expect("Paper has a fixed palette")
    }

    /// What the page being read stands on: the paper a shade darker, as a
    /// PDF reader's window around its page.
    pub fn canvas(&self) -> Hsla {
        let mut canvas = self.background;
        canvas.l = if self.dark {
            canvas.l * 0.55
        } else {
            (canvas.l - 0.06).max(0.)
        };
        canvas
    }

    /// The page's shadow on the canvas.
    pub fn page_shadow(&self) -> Vec<BoxShadow> {
        let alpha = if self.dark { 0.5 } else { 0.08 };
        vec![
            BoxShadow::new(px(0.), px(1.), hsla(0., 0., 0., alpha)).blur_radius(px(3.)),
            BoxShadow::new(px(0.), px(8.), hsla(0., 0., 0., alpha * 0.75)).blur_radius(px(24.)),
        ]
    }

    /// The accent, washed out: the code tab's ground, the mark under bold
    /// text on a page.
    pub fn accent_soft(&self) -> Hsla {
        let mut soft = self.link;
        soft.a = if self.dark { 0.15 } else { 0.13 };
        soft
    }

    /// The rich-text style for this palette, for text of `body` size: the
    /// colours, and the design on top of them.
    pub fn text_view_style(&self, body: Pixels) -> TextViewStyle {
        let mut selection = self.link;
        selection.a = 0.3;
        let palette = *self;
        let scale = f32::from(body) / 16.;
        TextViewStyle::default()
            .with_foreground(self.foreground)
            .with_muted_foreground(self.muted)
            .with_link(self.link)
            .with_selection(selection)
            // The code board's surface: the renderer also paints the code
            // tab's corner (`rendered::code_block_actions`) with it.
            .with_code_background(self.code_surface)
            .with_border(self.border)
            .with_inline_code(HighlightStyle {
                background_color: Some(self.code_background),
                color: Some(self.foreground),
                ..Default::default()
            })
            .with_paragraph_gap(rems(PARAGRAPH_GAP))
            .with_heading(move |level| palette.heading_style(body, level))
            .with_code_block(self.code_board(body))
            // A table is ruled across, never framed or divided into
            // columns, on the paper (the renderer's default surface is its
            // own theme's, which does not follow a fixed preview theme).
            .with_table(StyleRefinement::default().bg(self.background).border_0())
            // No column lines: the renderer draws them in the cell's border
            // colour (on the slack beside each cell), so that colour is none.
            .with_table_cell(
                StyleRefinement::default()
                    .border_r_0()
                    .border_color(gpui_kit::transparent_black())
                    .px(body * 0.9)
                    .py(body * 0.42),
            )
            // The header row is set heavier over a heavier rule, unfilled.
            .with_table_head(
                StyleRefinement::default()
                    .bg(gpui_kit::transparent_black())
                    .border_b(px(2. * scale))
                    .border_color(self.faint)
                    .text_color(self.foreground)
                    .font_weight(FontWeight::SEMIBOLD),
            )
            .with_dark(self.dark)
    }

    /// A fenced code block: the board, with room along its top for the tab
    /// that names the language. On light paper a whiteboard, a thick frame
    /// with a faint shadow and a slightly sunken surface; on dark paper a
    /// hairline frame whose top edge is a line of the accent.
    pub fn code_board(&self, body: Pixels) -> StyleRefinement {
        let scale = f32::from(body) / 16.;
        let board = StyleRefinement::default()
            .bg(self.code_surface)
            .border_color(self.code_frame)
            .pt(body * 2.4)
            .pb(body)
            .px(body * 1.2)
            .text_size(body * CODE_SCALE)
            .line_height(gpui_kit::relative(CODE_LEADING));
        if self.dark {
            let frame = px(CODE_FRAME_DARK * scale);
            board
                .border_t(px(0.))
                .border_r(frame)
                .border_b(frame)
                .border_l(frame)
                .rounded(px(10. * scale))
                .shadow(vec![
                    BoxShadow::new(px(0.), px(CODE_TOP_EDGE * scale), self.link).inset(),
                ])
        } else {
            board
                .border(px(CODE_FRAME_LIGHT * scale))
                .rounded(px(12. * scale))
                .shadow(vec![
                    BoxShadow::new(px(0.), px(0.), hsla(0., 0., 0., 0.08)).spread_radius(px(1.)),
                    BoxShadow::new(px(0.), px(2. * scale), hsla(60. / 360., 0.04, 0.1, 0.06))
                        .blur_radius(px(8. * scale)),
                    BoxShadow::new(px(0.), px(2. * scale), hsla(0., 0., 0., 0.07))
                        .blur_radius(px(6. * scale))
                        .inset(),
                ])
        }
    }

    /// A heading of `level` for text of `body` size: its size on the scale,
    /// its weight (bold to h3, semibold below), its colour (h5 and h6 in
    /// the soft ink), and under h2 a hairline.
    pub fn heading_style(&self, body: Pixels, level: u8) -> StyleRefinement {
        let size = body * heading_scale(level);
        let style = StyleRefinement::default()
            .text_size(size)
            .font_weight(heading_weight(level))
            .text_color(if level >= 5 {
                self.muted
            } else {
                self.foreground
            });
        if level != 2 {
            return style;
        }
        // The rule sits under the text, and the gap the renderer gives a
        // heading (0.35 of its size) moves below the rule.
        style
            .pb(body * HEADING_RULE_GAP)
            .border_b(px(f32::from(body) / 16.))
            .border_color(self.border)
            .mb(size * 0.35)
    }

    /// The body font family, or `None` to keep the application's.
    pub fn font_family(&self) -> Option<&'static str> {
        if !self.serif {
            return None;
        }
        Some(if cfg!(target_os = "linux") {
            "DejaVu Serif"
        } else {
            "Georgia"
        })
    }
}

/// The size of a heading of `level` as a factor of the body size.
pub fn heading_scale(level: u8) -> f32 {
    HEADING_SCALE[usize::from(level.clamp(1, 6)) - 1]
}

/// The weight of a heading of `level`.
pub fn heading_weight(level: u8) -> FontWeight {
    if level <= 3 {
        FontWeight::BOLD
    } else {
        FontWeight::SEMIBOLD
    }
}

/// A colour as CSS and Typst write it: `#rrggbb`.
pub fn hex(colour: Hsla) -> String {
    let rgba = Rgba::from(colour);
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_system_lacks_a_palette() {
        for theme in ALL {
            assert_eq!(
                theme.palette().is_none(),
                theme == PreviewTheme::System,
                "{theme:?}"
            );
        }
    }

    #[test]
    fn dark_palettes_have_dark_backgrounds() {
        for theme in ALL {
            let Some(palette) = theme.palette() else {
                continue;
            };
            assert_eq!(palette.background.l < 0.5, palette.dark, "{theme:?}");
            assert!(
                (palette.foreground.l - palette.background.l).abs() > 0.4,
                "{theme:?} text must contrast with its background"
            );
        }
    }

    #[test]
    fn names_round_trip_through_kebab_case() {
        for theme in ALL {
            let text = toml::to_string(&Holder { theme }).unwrap();
            let back: Holder = toml::from_str(&text).unwrap();
            assert_eq!(back.theme, theme, "{text}");
        }
        let text = toml::to_string(&Holder {
            theme: PreviewTheme::SolarizedDark,
        })
        .unwrap();
        assert_eq!(text.trim(), r#"theme = "solarized-dark""#);
    }

    #[derive(Serialize, Deserialize)]
    struct Holder {
        theme: PreviewTheme,
    }

    #[test]
    fn the_type_is_one_design_whatever_the_colours() {
        for theme in ALL {
            let Some(palette) = theme.palette() else {
                continue;
            };
            let style = palette.text_view_style(px(16.));
            assert_eq!(style.paragraph_gap(), rems(0.95), "{theme:?}");
            let heading = |level| style.heading(level);
            assert_eq!(
                heading(1).text.font_size,
                Some((px(16.) * 1.62).into()),
                "{theme:?}"
            );
            assert_eq!(heading(2).text.font_size, Some((px(16.) * 1.46).into()));
            assert_eq!(heading(3).text.font_size, Some((px(16.) * 1.2).into()));
            assert_eq!(heading(6).text.font_size, Some((px(16.) * 0.92).into()));
            assert_eq!(
                heading(9).text.font_size,
                Some((px(16.) * 0.92).into()),
                "out of range"
            );
            assert_eq!(heading(1).text.font_weight, Some(FontWeight::BOLD));
            assert_eq!(heading(3).text.font_weight, Some(FontWeight::BOLD));
            assert_eq!(heading(4).text.font_weight, Some(FontWeight::SEMIBOLD));
            // A hairline under h2 only, the gap below moved under it.
            assert_eq!(heading(1).border_widths.bottom, None);
            assert_eq!(heading(2).border_widths.bottom, Some(px(1.).into()));
            assert_eq!(heading(2).border_color, Some(palette.border));
            assert_eq!(heading(2).padding.bottom, Some((px(16.) * 0.5).into()));
            assert_eq!(
                heading(2).margin.bottom,
                Some((px(16.) * 1.46 * 0.35).into())
            );
            assert_eq!(heading(3).border_widths.bottom, None);
            assert_eq!(heading(3).margin.bottom, None);
            assert_eq!(heading(4).text.color, Some(palette.foreground));
            assert_eq!(heading(5).text.color, Some(palette.muted));
            assert_eq!(heading(6).text.color, Some(palette.muted));

            // Code on a board: the surface, the frame's colour, a smaller
            // size, the room for the tab along the top.
            let board = style.code_block();
            assert_eq!(
                board.background,
                Some(gpui_kit::Fill::Color(palette.code_surface.into()))
            );
            assert_eq!(style.code_background(), palette.code_surface);
            assert_eq!(board.border_color, Some(palette.code_frame));
            assert_eq!(board.text.font_size, Some((px(16.) * 0.85).into()));
            assert_eq!(board.padding.top, Some((px(16.) * 2.4).into()));
            if palette.dark {
                // A hairline frame, its top edge a line of the accent.
                assert_eq!(board.border_widths.left, Some(px(1.).into()));
                assert_eq!(board.border_widths.top, Some(px(0.).into()));
                let edge = board.box_shadow.as_ref().unwrap();
                assert_eq!(edge.len(), 1, "{theme:?}");
                assert!(edge[0].inset);
                assert_eq!(edge[0].offset.y, px(2.));
                assert_eq!(edge[0].color, palette.link, "{theme:?}");
            } else {
                // A whiteboard's thick frame.
                assert_eq!(board.border_widths.top, Some(px(5.).into()));
                assert_eq!(board.corner_radii.top_left, Some(px(12.).into()));
                assert_eq!(board.box_shadow.as_ref().map(Vec::len), Some(3));
            }
            assert_eq!(
                style.inline_code().background_color,
                Some(palette.code_background)
            );

            assert_eq!(style.table_cell().border_widths.right, Some(px(0.).into()));
            assert_eq!(
                style.table_cell().border_color,
                Some(gpui_kit::transparent_black()),
                "{theme:?}: no column lines"
            );
            assert_eq!(
                style.table().background,
                Some(gpui_kit::Fill::Color(palette.background.into())),
                "{theme:?}: the table surface is the paper"
            );
            assert_eq!(style.table().border_widths.top, Some(px(0.).into()));
            assert_eq!(
                style.table_head().background,
                Some(gpui_kit::Fill::Color(gpui_kit::transparent_black().into())),
                "{theme:?}: the header row is not filled"
            );
            assert_eq!(style.table_head().border_widths.bottom, Some(px(2.).into()));
            assert_eq!(style.table_head().border_color, Some(palette.faint));
            assert_eq!(style.table_head().text.color, Some(palette.foreground));
            assert_eq!(style.foreground(), palette.foreground);
            assert_eq!(style.link(), palette.link);
            assert_eq!(style.is_dark(), palette.dark);
        }
        // A bigger body scales the headings, the rule and the frame with it.
        let style = PreviewTheme::Paper
            .palette()
            .unwrap()
            .text_view_style(px(32.));
        assert_eq!(
            style.heading(1).text.font_size,
            Some((px(32.) * 1.62).into())
        );
        assert_eq!(style.heading(2).border_widths.bottom, Some(px(2.).into()));
        assert_eq!(style.code_block().border_widths.top, Some(px(10.).into()));
    }

    #[test]
    fn the_page_stands_on_a_darker_canvas() {
        assert_eq!(PAGE_WIDTH - PAGE_MARGIN * 2., MEASURE, "the blog's column");
        for theme in ALL {
            let Some(palette) = theme.palette() else {
                continue;
            };
            assert!(
                palette.canvas().l < palette.background.l,
                "{theme:?}: the canvas is darker than the paper"
            );
            assert_eq!(palette.page_shadow().len(), 2);
        }
    }
    #[test]
    fn names_parse_and_every_family_prints_light() {
        for theme in ALL {
            assert_eq!(PreviewTheme::parse(theme.name()), Some(theme));
            assert!(!theme.print_palette().dark, "{theme:?} prints on paper");
            if theme.palette().is_some_and(|palette| !palette.dark) {
                assert_eq!(theme.light(), theme, "a light theme is its own print");
            }
        }
        assert_eq!(PreviewTheme::parse("Paper"), None, "names are kebab-case");
        assert_eq!(PreviewTheme::System.light(), PreviewTheme::Paper);
        assert_eq!(PreviewTheme::PaperDark.light(), PreviewTheme::Paper);
        assert_eq!(PreviewTheme::Night.light(), PreviewTheme::Github);
        assert_eq!(
            PreviewTheme::SolarizedDark.light(),
            PreviewTheme::SolarizedLight
        );
    }

    #[test]
    fn colours_write_as_the_hex_they_were_read_from() {
        for theme in ALL {
            let Some(palette) = theme.palette() else {
                continue;
            };
            // Faint ink is fainter than soft ink, and soft ink than the ink.
            let distance = |colour: Hsla| (colour.l - palette.background.l).abs();
            assert!(
                distance(palette.faint) < distance(palette.muted)
                    && distance(palette.muted) < distance(palette.foreground),
                "{theme:?}"
            );
            for colour in [
                palette.background,
                palette.foreground,
                palette.muted,
                palette.faint,
                palette.link,
                palette.wash,
                palette.code_background,
                palette.code_surface,
                palette.code_frame,
                palette.code_rail,
                palette.border,
            ] {
                let text = super::hex(colour);
                assert_eq!(text.len(), 7, "{theme:?}: {text}");
                let value = u32::from_str_radix(&text[1..], 16).unwrap();
                assert_eq!(super::hex(hex(value)), text, "{theme:?}: round trip");
            }
        }
        assert_eq!(super::hex(hex(0xf8f7f3)), "#f8f7f3");
        assert_eq!(super::hex(hex(0x121212)), "#121212");
        assert_eq!(super::hex(hex(0xc3423f)), "#c3423f");
    }

    #[test]
    fn the_bundled_theme_file_parses_with_both_looks() {
        let set: gpui_kit::component::ThemeSet = serde_json::from_str(PAPER_THEMES).unwrap();
        let names: Vec<&str> = set.themes.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(names, [PAPER_LIGHT, PAPER_DARK]);
        assert!(!set.themes[0].mode.is_dark());
        assert!(set.themes[1].mode.is_dark());
        for theme in &set.themes {
            assert!(theme.highlight.is_some(), "{}: syntax colours", theme.name);
            assert_eq!(theme.colors.background.as_deref().map(str::len), Some(7));
        }

        // A misspelt key is silently ignored by the config's deserializer
        // (gpui-kit's own sample theme has a few). Every key written in the
        // file must come back out of the parsed colours, or it did nothing.
        let raw: serde_json::Value = serde_json::from_str(PAPER_THEMES).unwrap();
        for (raw_theme, theme) in raw["themes"].as_array().unwrap().iter().zip(&set.themes) {
            let parsed = serde_json::to_value(&theme.colors).unwrap();
            let parsed = parsed.as_object().unwrap();
            for key in raw_theme["colors"].as_object().unwrap().keys() {
                assert!(
                    parsed.get(key).is_some_and(|v| !v.is_null()),
                    "{}: colour key {key:?} is not one the theme schema knows",
                    theme.name
                );
            }
        }
    }

    #[gpui_kit::test]
    fn paper_is_the_application_theme_in_both_modes(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::component::{ActiveTheme as _, ThemeMode};
        cx.update(|cx| {
            gpui_kit::init(cx);
            install(cx);
            Theme::change(ThemeMode::Light, None, cx);
            assert_eq!(cx.theme().background, hex(0xf8f7f3));
            assert_eq!(cx.theme().foreground, hex(0x1a1a18));
            assert_eq!(cx.theme().title_bar, hex(0xf1efeb));
            assert_eq!(cx.theme().caret, hex(0xc3423f), "the cursor is cinnabar");
            let heading = cx.theme().highlight_theme.style("title").unwrap();
            assert_eq!(heading.color, Some(hex(0xc3423f)));
            // The preview that follows the application is the Paper palette.
            assert_eq!(
                Palette::from_app(cx.theme()),
                PreviewTheme::Paper.palette().unwrap()
            );
            assert_eq!(
                PreviewTheme::System.resolve(cx.theme()),
                PreviewTheme::Paper.palette().unwrap()
            );

            Theme::change(ThemeMode::Dark, None, cx);
            assert_eq!(cx.theme().background, hex(0x121212));
            assert_eq!(cx.theme().foreground, hex(0xc1bfba));
            assert_eq!(cx.theme().title_bar, hex(0x171716));
            let heading = cx.theme().highlight_theme.style("title").unwrap();
            assert_eq!(heading.color, Some(hex(0xdd625e)));
            assert_eq!(
                PreviewTheme::System.resolve(cx.theme()),
                PreviewTheme::PaperDark.palette().unwrap()
            );
        });
    }

    fn hex(value: u32) -> Hsla {
        rgb(value).into()
    }
}
