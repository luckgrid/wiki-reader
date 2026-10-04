//! Reading herdr's settings and plugin launch context (read-only).
//!
//! herdr does not tell child programs which theme it uses (no environment variable or socket
//! call), but the choice is in its `config.toml`, so wiki-reader reads the theme name from there
//! when its own `theme = "herdr"` is in effect.

use std::path::{Path, PathBuf};

/// What the launcher needs from herdr's plugin context (`HERDR_PLUGIN_CONTEXT_JSON`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginContext {
    /// The pane that had focus when the plugin ran, if known.
    pub focused_pane_id: Option<String>,
    /// Collection cwd: the focused pane's cwd first, then the workspace's.
    pub cwd: Option<PathBuf>,
}

/// Parse the plugin context. `None` for text that is not a JSON object; absent or empty
/// fields are left unset. This only parses: callers check that the cwd is a directory.
#[must_use]
pub fn parse_context(text: &str) -> Option<PluginContext> {
    let context: serde_json::Value = serde_json::from_str(text).ok()?;
    let object = context.as_object()?;
    let string = |key: &str| {
        object
            .get(key)?
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    Some(PluginContext {
        focused_pane_id: string("focused_pane_id").map(str::to_owned),
        cwd: string("focused_pane_cwd")
            .or_else(|| string("workspace_cwd"))
            .map(PathBuf::from),
    })
}

/// Collection cwd from Herdr's plugin context: focused pane first, then workspace.
///
/// Missing, malformed or empty fields return `None`.
#[must_use]
pub fn parse_context_cwd(text: &str) -> Option<PathBuf> {
    parse_context(text)?.cwd
}

/// The new pane's id from a `herdr pane split` JSON response (`.result.pane.pane_id`).
#[must_use]
pub fn parse_split_pane_id(text: &str) -> Option<String> {
    let response: serde_json::Value = serde_json::from_str(text).ok()?;
    let id = response
        .get("result")?
        .get("pane")?
        .get("pane_id")?
        .as_str()?
        .trim();
    (!id.is_empty()).then(|| id.to_owned())
}

/// Whether this process was started by a herdr plugin pane entrypoint of any placement:
/// inside herdr (`HERDR_ENV=1`) with `HERDR_PLUGIN_ENTRYPOINT_ID` set.
///
/// herdr 0.9.x starts those panes without pixel metrics and never answers the terminal's
/// cell-size query, so graphics cannot be sized there. Ordinary shell panes answer.
#[must_use]
pub fn is_plugin_pane(herdr_env: Option<&str>, entrypoint_id: Option<&str>) -> bool {
    herdr_env == Some("1") && entrypoint_id.is_some_and(|id| !id.trim().is_empty())
}

/// [`is_plugin_pane`] for this process's environment.
#[must_use]
pub fn running_in_plugin_pane() -> bool {
    let env = std::env::var("HERDR_ENV").ok();
    let entrypoint = std::env::var("HERDR_PLUGIN_ENTRYPOINT_ID").ok();
    is_plugin_pane(env.as_deref(), entrypoint.as_deref())
}

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
    fn plugin_pane_is_herdr_with_an_entrypoint_id() {
        assert!(is_plugin_pane(Some("1"), Some("reader-popup")));
        assert!(is_plugin_pane(Some("1"), Some("overlay")));
        assert!(!is_plugin_pane(Some("1"), None), "an ordinary herdr pane");
        assert!(!is_plugin_pane(Some("1"), Some("  ")));
        assert!(!is_plugin_pane(None, Some("overlay")), "outside herdr");
        assert!(!is_plugin_pane(Some("0"), Some("overlay")));
    }

    #[test]
    fn context_reads_the_focused_pane_and_cwd() {
        let context = parse_context(
            r#"{"focused_pane_id":" w1:p2 ","focused_pane_cwd":"/a b","workspace_cwd":"/w","x":1}"#,
        )
        .unwrap();
        assert_eq!(context.focused_pane_id.as_deref(), Some("w1:p2"));
        assert_eq!(context.cwd, Some(PathBuf::from("/a b")));
        let partial = parse_context(r#"{"workspace_cwd":"/w"}"#).unwrap();
        assert_eq!(partial.focused_pane_id, None);
        assert_eq!(partial.cwd, Some(PathBuf::from("/w")));
        assert_eq!(parse_context("{}"), Some(PluginContext::default()));
        for text in ["", "not json", "null", "[]", "42"] {
            assert_eq!(parse_context(text), None, "{text}");
        }
    }

    #[test]
    fn split_response_yields_the_new_pane_id() {
        assert_eq!(
            parse_split_pane_id(r#"{"id":"cli:pane:split","result":{"pane":{"pane_id":"w30:pZ"},"type":"pane_info"}}"#)
                .as_deref(),
            Some("w30:pZ")
        );
        for text in [
            "",
            "{}",
            r#"{"result":{}}"#,
            r#"{"result":{"pane":{"pane_id":""}}}"#,
            r#"{"result":{"pane":{"pane_id":7}}}"#,
            r#"{"error":{"code":"x"}}"#,
        ] {
            assert_eq!(parse_split_pane_id(text), None, "{text}");
        }
    }

    #[test]
    fn context_prefers_focused_pane_and_preserves_path() {
        assert_eq!(
            parse_context_cwd(
                r#"{"focused_pane_cwd":"/a collection/with spaces", "workspace_cwd":"/workspace", "extra":true}"#
            ),
            Some(PathBuf::from("/a collection/with spaces"))
        );
    }

    #[test]
    fn context_falls_back_to_workspace() {
        for focused in ["null", "42", "\"\"", "\"  \""] {
            let text = format!(r#"{{"focused_pane_cwd":{focused},"workspace_cwd":"/workspace"}}"#);
            assert_eq!(parse_context_cwd(&text), Some(PathBuf::from("/workspace")));
        }
        assert_eq!(
            parse_context_cwd(r#"{"workspace_cwd":"/workspace"}"#),
            Some(PathBuf::from("/workspace"))
        );
    }

    #[test]
    fn context_rejects_malformed_or_missing_paths() {
        for text in [
            "",
            "not json",
            "{",
            "null",
            "[]",
            "{}",
            r#"{"workspace_cwd":false}"#,
            r#"{"workspace_cwd":" "}"#,
        ] {
            assert_eq!(parse_context_cwd(text), None, "{text}");
        }
    }

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
