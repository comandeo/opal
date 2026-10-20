//! Text tags, colours and the body font.

use std::collections::HashMap;

use gtk::{gdk, pango};

use crate::config::{Config, Theme};
use crate::restyle::Style;

const HEADING_SCALE: [f64; 6] = [1.8, 1.5, 1.25, 1.1, 1.0, 1.0];

pub struct Palette {
    pub markup: gdk::RGBA,
    pub link: gdk::RGBA,
    pub code_bg: gdk::RGBA,
    /// Quote bars and rules.
    pub line: gdk::RGBA,
    /// Bullets and check boxes.
    pub glyph: gdk::RGBA,
}

impl Palette {
    pub fn for_theme(theme: Theme) -> Self {
        let c = |hex: &str| gdk::RGBA::parse(hex).expect("valid colour literal");
        match theme {
            Theme::Light => Palette {
                markup: c("#a3a3a3"),
                link: c("#2766b0"),
                code_bg: c("#f1f1f1"),
                line: c("#c9c9c9"),
                glyph: c("#7a7a7a"),
            },
            Theme::Dark => Palette {
                markup: c("#6e6e6e"),
                link: c("#79a9e6"),
                code_bg: c("#2b2b2b"),
                line: c("#4f4f4f"),
                glyph: c("#9a9a9a"),
            },
        }
    }
}

pub struct Tags(HashMap<Style, gtk::TextTag>);

impl Tags {
    /// One tag per style, added in `Style::ALL` order: later tags win, so
    /// `Markup` and `Hidden` override everything else.
    pub fn new(table: &gtk::TextTagTable, config: &Config, palette: &Palette) -> Self {
        let mono = pango::FontDescription::from_string(&config.mono_font);
        let mut map = HashMap::new();
        for style in Style::ALL {
            let b = gtk::TextTag::builder().name(tag_name(style));
            let tag = match style {
                Style::Strong => b.weight(700),
                Style::Emphasis => b.style(pango::Style::Italic),
                Style::Strike => b.strikethrough(true),
                Style::Code => b.font_desc(&mono).background_rgba(&palette.code_bg),
                Style::Link => b.foreground_rgba(&palette.link).underline(pango::Underline::Single),
                Style::Heading(n) => b.weight(700).scale(HEADING_SCALE[n as usize - 1]).pixels_above_lines(8),
                // Their left margin follows the column; see editor::margins.
                Style::Quote | Style::ListItem => b,
                Style::CodeBlock => b.font_desc(&mono),
                Style::Markup => b.foreground_rgba(&palette.markup),
                Style::Hidden => b.invisible(true),
            }
            .build();
            table.add(&tag);
            map.insert(style, tag);
        }
        Tags(map)
    }

    pub fn get(&self, style: Style) -> &gtk::TextTag {
        &self.0[&style]
    }
}

fn tag_name(style: Style) -> String {
    match style {
        Style::Strong => "strong".into(),
        Style::Emphasis => "em".into(),
        Style::Strike => "strike".into(),
        Style::Code => "code".into(),
        Style::Link => "link".into(),
        Style::Heading(n) => format!("h{n}"),
        Style::Quote => "quote".into(),
        Style::ListItem => "list-item".into(),
        Style::CodeBlock => "code-block".into(),
        Style::Markup => "markup".into(),
        Style::Hidden => "hidden".into(),
    }
}

/// Sets the body font of `textview.opal` widgets from a Pango description.
pub fn install_font_css(font: &pango::FontDescription) {
    let mut css = String::from("textview.opal {");
    if let Some(family) = font.family() {
        css.push_str(&format!(" font-family: \"{family}\";"));
    }
    if font.size() > 0 {
        let unit = if font.is_size_absolute() { "px" } else { "pt" };
        css.push_str(&format!(" font-size: {}{unit};", font.size() as f64 / pango::SCALE as f64));
    }
    css.push_str(" }");
    let provider = gtk::CssProvider::new();
    provider.load_from_data(&css);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}
