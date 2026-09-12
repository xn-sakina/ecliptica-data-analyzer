use std::{str::FromStr, sync::OnceLock};

use eframe::egui;
use egui_extras::syntax_highlighting::{CodeTheme, SyntectSettings, highlight_with};
use syntect::{
    highlighting::{
        Color, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSet, ThemeSettings,
    },
    parsing::{SyntaxDefinition, SyntaxSetBuilder},
};

const LANGUAGE: &str = "Ecliptica Handlebars";
const SYNTAX: &str = include_str!("../assets/ecliptica-handlebars.sublime-syntax");
const THEME: &str = "base16-mocha.dark";
const TEXT: Color = rgb(246, 243, 255);
const COMMENT: Color = rgb(166, 156, 184);
const DELIMITER: Color = rgb(203, 184, 255);
const KEYWORD: Color = rgb(255, 143, 203);
const HELPER: Color = rgb(118, 215, 255);
const VARIABLE: Color = rgb(255, 211, 102);
const STRING: Color = rgb(155, 226, 143);
const LITERAL: Color = rgb(255, 184, 108);

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

pub fn highlight(ui: &egui::Ui, text: &str) -> egui::text::LayoutJob {
    highlight_with(
        ui.ctx(),
        ui.style(),
        &CodeTheme::dark(14.0),
        text,
        LANGUAGE,
        settings(),
    )
}

fn settings() -> &'static SyntectSettings {
    static SETTINGS: OnceLock<SyntectSettings> = OnceLock::new();

    SETTINGS.get_or_init(|| {
        let syntax = SyntaxDefinition::load_from_str(SYNTAX, true, None)
            .expect("bundled Ecliptica Handlebars syntax must be valid");
        let mut builder = SyntaxSetBuilder::new();
        builder.add(syntax);
        let mut themes = ThemeSet::new();
        themes.themes.insert(THEME.to_owned(), template_theme());

        SyntectSettings {
            ps: builder.build(),
            ts: themes,
        }
    })
}

fn template_theme() -> Theme {
    Theme {
        name: Some("Ecliptica high contrast".to_owned()),
        settings: ThemeSettings {
            foreground: Some(TEXT),
            ..Default::default()
        },
        scopes: vec![
            theme_item("comment", COMMENT, Some(FontStyle::ITALIC)),
            theme_item(
                "punctuation.section.embedded, punctuation.section.group, keyword.operator",
                DELIMITER,
                None,
            ),
            theme_item("keyword.control", KEYWORD, Some(FontStyle::BOLD)),
            theme_item("support.function", HELPER, None),
            theme_item("variable.other", VARIABLE, None),
            theme_item("string.quoted", STRING, None),
            theme_item("constant.numeric, constant.language", LITERAL, None),
            theme_item("constant.character.escape", HELPER, None),
        ],
        ..Default::default()
    }
}

fn theme_item(scope: &str, foreground: Color, font_style: Option<FontStyle>) -> ThemeItem {
    ThemeItem {
        scope: ScopeSelectors::from_str(scope).expect("template theme scope must be valid"),
        style: StyleModifier {
            foreground: Some(foreground),
            font_style,
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syntect::{easy::HighlightLines, util::LinesWithEndings};

    #[test]
    fn settings_load_only_the_template_language_and_theme() {
        let settings = settings();

        assert_eq!(settings.ps.syntaxes().len(), 1);
        assert_eq!(settings.ts.themes.len(), 1);
    }

    #[test]
    fn bundled_syntax_highlights_template_token_categories() {
        let settings = settings();
        let syntax = settings.ps.find_syntax_by_name(LANGUAGE).unwrap();
        let theme = &settings.ts.themes[THEME];
        let mut highlighter = HighlightLines::new(syntax, theme);
        let template = "{{#if has_latest_dps}}{{random \"DPS\" latest_dps 20 true}}{{else}}{{! hidden }}{{/if}}\n";
        let colors = LinesWithEndings::from(template)
            .flat_map(|line| highlighter.highlight_line(line, &settings.ps).unwrap())
            .map(|(style, _)| style.foreground)
            .collect::<std::collections::HashSet<_>>();

        assert!(
            colors.len() >= 5,
            "expected syntax categories to use distinct colors"
        );
    }

    #[test]
    fn template_variables_are_distinct_from_message_text() {
        let settings = settings();
        let syntax = settings.ps.find_syntax_by_name(LANGUAGE).unwrap();
        let theme = &settings.ts.themes[THEME];
        let mut highlighter = HighlightLines::new(syntax, theme);
        let highlighted = highlighter
            .highlight_line("DPS: {{latest_dps}}\n", &settings.ps)
            .unwrap();
        let plain = highlighted
            .iter()
            .find(|(_, text)| text.contains("DPS"))
            .unwrap()
            .0
            .foreground;
        let variable = highlighted
            .iter()
            .find(|(_, text)| text.contains("latest_dps"))
            .unwrap()
            .0
            .foreground;

        assert_eq!(plain, TEXT);
        assert_eq!(variable, VARIABLE);
    }
}
