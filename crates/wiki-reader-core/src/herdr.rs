//! Reading herdr's own settings (read-only).
//!
//! herdr does not tell child programs which theme it uses (no environment variable or socket
//! call), but the choice is in its `config.toml`, so wiki-reader reads the theme name from there
//! when its own `theme = "herdr"` is in effect.

use std::path::{Path, PathBuf};

/// herdr's config file: `$XDG_CONFIG_HOME/herdr/config.toml`, else `~/.config/herdr/config.toml`.
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let xdg = xdg.trim();
        if !xdg.is_empty() && Path::new(xdg).is_absolute() {
            return Some(PathBuf::from(xdg).join("herdr/config.toml"));
        }
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/herdr/config.toml"))
}

/// The theme name herdr is configured with, from the file at [`config_path`].
///
/// `None` when herdr's config is missing, unreadable, or names no theme.
#[must_use]
pub fn theme_name() -> Option<String> {
    let text = std::fs::read_to_string(config_path()?).ok()?;
    parse_theme_name(&text)
}

/// The theme name in herdr `config.toml` text: `[theme] name`.
///
/// With `auto_switch = true` herdr picks `light_name` or `dark_name` from the terminal's
/// appearance, which a child process cannot see; `name` is used when present, else `dark_name`.
#[must_use]
pub fn parse_theme_name(text: &str) -> Option<String> {
    let table: toml::Table = toml::from_str(text).ok()?;
    let theme = table.get("theme")?.as_table()?;
    ["name", "dark_name"]
        .iter()
        .filter_map(|k| theme.get(*k)?.as_str())
        .map(|s| s.trim().to_owned())
        .find(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_theme_name() {
        let text = "onboarding = false\n[ui]\nagent_panel_sort = \"spaces\"\n[theme]\nname = \"vesper\"\nauto_switch = false\n";
        assert_eq!(parse_theme_name(text).as_deref(), Some("vesper"));
    }

    #[test]
    fn falls_back_to_dark_name_then_none() {
        assert_eq!(
            parse_theme_name("[theme]\nauto_switch = true\ndark_name = \"gruvbox\"\nlight_name = \"gruvbox-light\"\n")
                .as_deref(),
            Some("gruvbox")
        );
        assert_eq!(parse_theme_name("[theme]\nauto_switch = true\n"), None);
        assert_eq!(parse_theme_name("[ui]\nx = 1\n"), None);
        assert_eq!(parse_theme_name("not = [valid"), None);
        assert_eq!(parse_theme_name("[theme]\nname = \"  \"\n"), None);
    }
}
