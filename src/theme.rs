//! How smep looks.
//!
//! The application itself (editor, title bar, menus) is drawn in Paper:
//! ink on paper, light or dark with the system, the same two looks as
//! keepane's `paper` and `paper-dark` themes (warm cream or warm near-black,
//! Vitesse hues, one accent, cinnabar). They are gpui-kit theme configs in
//! `assets/themes/paper.json`, installed by [`install`].
//!
//! The rendered Markdown can wear a look of its own, a [`PreviewTheme`];
//! by default it follows the application.

use gpui_kit::App;
use gpui_kit::base::text::TextViewStyle;
use gpui_kit::component::{Theme, ThemeRegistry};
use gpui_kit::{HighlightStyle, Hsla, rgb};
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

/// The colours and type of one fixed theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub background: Hsla,
    pub foreground: Hsla,
    pub muted: Hsla,
    pub link: Hsla,
    pub code_background: Hsla,
    pub border: Hsla,
    pub serif: bool,
    pub dark: bool,
}

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

    /// The fixed palette, or `None` for the theme that follows the app.
    pub fn palette(self) -> Option<Palette> {
        let hex = |value: u32| -> Hsla { rgb(value).into() };
        Some(match self {
            Self::System => return None,
            // keepane's paper: ink #33302a on cream #f0e7d2, the bar's
            // darker edge for code, warm grey inks, blue links.
            Self::Paper => Palette {
                background: hex(0xf0e7d2),
                foreground: hex(0x33302a),
                muted: hex(0x635b4e),
                link: hex(0x035fa0),
                code_background: hex(0xe3d8bf),
                border: hex(0xd3c8ad),
                serif: false,
                dark: false,
            },
            // keepane's paper-dark: ink #cecabe on warm near-black #171716.
            Self::PaperDark => Palette {
                background: hex(0x171716),
                foreground: hex(0xcecabe),
                muted: hex(0x87837b),
                link: hex(0x4f95cf),
                code_background: hex(0x242423),
                border: hex(0x2c2c2a),
                serif: false,
                dark: true,
            },
            Self::Github => Palette {
                background: hex(0xffffff),
                foreground: hex(0x1f2328),
                muted: hex(0x59636e),
                link: hex(0x0969da),
                code_background: hex(0xf6f8fa),
                border: hex(0xd1d9e0),
                serif: false,
                dark: false,
            },
            Self::Newsprint => Palette {
                background: hex(0xf3f2ee),
                foreground: hex(0x1f0909),
                muted: hex(0x6a6560),
                link: hex(0x8b3a10),
                code_background: hex(0xe7e6e1),
                border: hex(0xc9c8c3),
                serif: true,
                dark: false,
            },
            Self::Night => Palette {
                background: hex(0x363b40),
                foreground: hex(0xd9d9d9),
                muted: hex(0x9aa0a6),
                link: hex(0x7fb2f0),
                code_background: hex(0x2b2f33),
                border: hex(0x4b5259),
                serif: false,
                dark: true,
            },
            Self::Sepia => Palette {
                background: hex(0xf4ecd8),
                foreground: hex(0x5b4636),
                muted: hex(0x8b7355),
                link: hex(0x9c5a1e),
                code_background: hex(0xeadfc3),
                border: hex(0xd9c9a3),
                serif: true,
                dark: false,
            },
            Self::SolarizedLight => Palette {
                background: hex(0xfdf6e3),
                foreground: hex(0x586e75),
                muted: hex(0x93a1a1),
                link: hex(0x268bd2),
                code_background: hex(0xeee8d5),
                border: hex(0xd6cfb8),
                serif: false,
                dark: false,
            },
            Self::SolarizedDark => Palette {
                background: hex(0x002b36),
                foreground: hex(0x93a1a1),
                muted: hex(0x657b83),
                link: hex(0x268bd2),
                code_background: hex(0x073642),
                border: hex(0x0f4a5a),
                serif: false,
                dark: true,
            },
        })
    }
}

impl Palette {
    /// The rich-text style for this palette.
    pub fn text_view_style(&self) -> TextViewStyle {
        let mut selection = self.link;
        selection.a = 0.3;
        TextViewStyle::default()
            .with_foreground(self.foreground)
            .with_muted_foreground(self.muted)
            .with_link(self.link)
            .with_selection(selection)
            .with_code_background(self.code_background)
            .with_border(self.border)
            .with_inline_code(HighlightStyle {
                background_color: Some(self.code_background),
                color: Some(self.foreground),
                ..Default::default()
            })
            .with_dark(self.dark)
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
            assert_eq!(cx.theme().background, hex(0xf0e7d2));
            assert_eq!(cx.theme().foreground, hex(0x33302a));
            assert_eq!(cx.theme().title_bar, hex(0xe3d8bf));
            assert_eq!(cx.theme().caret, hex(0xa53736), "the cursor is cinnabar");
            let heading = cx.theme().highlight_theme.style("title").unwrap();
            assert_eq!(heading.color, Some(hex(0xa53736)));

            Theme::change(ThemeMode::Dark, None, cx);
            assert_eq!(cx.theme().background, hex(0x171716));
            assert_eq!(cx.theme().foreground, hex(0xcecabe));
            assert_eq!(cx.theme().title_bar, hex(0x121212));
            let heading = cx.theme().highlight_theme.style("title").unwrap();
            assert_eq!(heading.color, Some(hex(0xdd625e)));
        });
    }

    fn hex(value: u32) -> Hsla {
        rgb(value).into()
    }
}
