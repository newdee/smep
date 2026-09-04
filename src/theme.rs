//! Preview themes: how the rendered Markdown looks, independent of the
//! application's light/dark chrome.

use gpui_kit::base::text::TextViewStyle;
use gpui_kit::{HighlightStyle, Hsla, rgb};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewTheme {
    /// Follow the application theme (light or dark with the system).
    #[default]
    System,
    Github,
    Newsprint,
    Night,
    Sepia,
    SolarizedLight,
    SolarizedDark,
}

/// Every theme, in menu order.
pub const ALL: [PreviewTheme; 7] = [
    PreviewTheme::System,
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
            Self::System => "System",
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
}
