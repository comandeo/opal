//! `~/.config/opal/config`: `key = value` lines, `#` starts a comment.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Column width in characters.
    pub column_width: u32,
    /// Pango font description, e.g. "Serif 14".
    pub font: String,
    pub mono_font: String,
    /// Typewriter mode at startup.
    pub typewriter: bool,
    pub theme: Theme,
    /// Typographic replacements while typing: `--` → `—`, `"` → «».
    pub autocorrect: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            column_width: 70,
            font: "Serif 14".to_string(),
            mono_font: "Monospace 12".to_string(),
            typewriter: true,
            theme: Theme::Light,
            autocorrect: true,
        }
    }
}

/// Parses config text; returns the config and a warning per ignored line.
pub fn parse(text: &str) -> (Config, Vec<String>) {
    let mut cfg = Config::default();
    let mut warnings = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            warnings.push(format!("строка {n}: ожидается `ключ = значение`"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let ok = match key {
            "column_width" => match value.parse::<u32>() {
                Ok(v) if (20..=400).contains(&v) => {
                    cfg.column_width = v;
                    true
                }
                _ => false,
            },
            "font" | "mono_font" if !value.is_empty() => {
                let slot = if key == "font" { &mut cfg.font } else { &mut cfg.mono_font };
                *slot = value.to_string();
                true
            }
            "typewriter" | "autocorrect" => match value {
                "true" | "false" => {
                    let slot = if key == "typewriter" { &mut cfg.typewriter } else { &mut cfg.autocorrect };
                    *slot = value == "true";
                    true
                }
                _ => false,
            },
            "theme" => match value {
                "light" => {
                    cfg.theme = Theme::Light;
                    true
                }
                "dark" => {
                    cfg.theme = Theme::Dark;
                    true
                }
                _ => false,
            },
            "font" | "mono_font" => false,
            _ => {
                warnings.push(format!("строка {n}: неизвестный ключ `{key}`"));
                continue;
            }
        };
        if !ok {
            warnings.push(format!("строка {n}: недопустимое значение `{value}` для `{key}`"));
        }
    }
    (cfg, warnings)
}

/// `$XDG_CONFIG_HOME/opal/config`, falling back to `$HOME/.config/opal/config`.
pub fn path_from(xdg_config_home: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let base = match xdg_config_home.filter(|v| !v.is_empty()) {
        Some(xdg) => PathBuf::from(xdg),
        None => PathBuf::from(home.filter(|v| !v.is_empty())?).join(".config"),
    };
    Some(base.join("opal").join("config"))
}

/// Reads the config file; problems are reported to stderr and never fatal.
pub fn load() -> Config {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let home = std::env::var("HOME").ok();
    let Some(path) = path_from(xdg.as_deref(), home.as_deref()) else {
        return Config::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let (cfg, warnings) = parse(&text);
            for w in warnings {
                eprintln!("opal: {}: {w}", path.display());
            }
            cfg
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(e) => {
            eprintln!("opal: не удалось прочитать {}: {e}", path.display());
            Config::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_gives_defaults() {
        assert_eq!(parse(""), (Config::default(), vec![]));
    }

    #[test]
    fn all_keys() {
        let text = "# comment\ncolumn_width = 80\nfont = Literata 15  # trailing comment\nmono_font=JetBrains Mono 11\ntypewriter = false\ntheme = dark\nautocorrect = false\n";
        let (cfg, warnings) = parse(text);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            cfg,
            Config {
                column_width: 80,
                font: "Literata 15".into(),
                mono_font: "JetBrains Mono 11".into(),
                typewriter: false,
                theme: Theme::Dark,
                autocorrect: false,
            }
        );
    }

    #[test]
    fn autocorrect_is_on_by_default() {
        assert!(Config::default().autocorrect);
        let (cfg, warnings) = parse("autocorrect = maybe\n");
        assert!(cfg.autocorrect);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn bad_lines_warn_and_keep_defaults() {
        let (cfg, warnings) = parse("column_width = wide\ncolumn_width = 5\ntheme = blue\nfont =\nnonsense\ncolour = red\n");
        assert_eq!(cfg, Config::default());
        assert_eq!(warnings.len(), 6, "{warnings:?}");
        assert!(warnings[0].starts_with("строка 1:"));
        assert!(warnings[5].contains("colour"));
    }

    #[test]
    fn path_prefers_xdg() {
        assert_eq!(path_from(Some("/x"), Some("/h")), Some(PathBuf::from("/x/opal/config")));
        assert_eq!(path_from(None, Some("/h")), Some(PathBuf::from("/h/.config/opal/config")));
        assert_eq!(path_from(Some(""), Some("/h")), Some(PathBuf::from("/h/.config/opal/config")));
        assert_eq!(path_from(None, None), None);
    }
}
