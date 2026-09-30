//! Reader configuration (excludes, nav labels, themes, etc.).
//!
//! See [product spec](../../../wiki/product/spec.md) (C1) and
//! [architecture overview](../../../wiki/architecture/overview.md).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// How side-nav (and footer) page labels are built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LabelMode {
    /// `nav_title` → `title` → H1 → humanized filename.
    #[default]
    Title,
    /// Always the humanized filename / stem.
    Filename,
    /// Title with dim filename — for now: `"Title (filename)"`.
    #[serde(rename = "title+filename")]
    TitleFilename,
}

/// Diagram render preference (D1; stored for later wiring).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagramMode {
    #[default]
    Auto,
    Image,
    Text,
    Source,
}

/// `[nav]` table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct NavConfig {
    #[serde(default)]
    pub labels: LabelMode,
}

/// Loaded reader config plus per-key diagnostics (never a hard startup failure).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    pub exclude: Vec<String>,
    pub nav: NavConfig,
    pub opener: Option<String>,
    pub editor: Option<String>,
    pub diagrams: DiagramMode,
    /// Theme name or path (token table wiring is later).
    pub theme: Option<String>,
    /// Action-name → key chord overrides (e.g. `"quit" = "Q"`).
    pub keys: BTreeMap<String, String>,
    /// Human-readable diagnostics for bad / unknown keys.
    pub diagnostics: Vec<String>,
}

impl Config {
    /// Merge XDG → `<root>/.wiki-reader.toml` → optional `--config` override.
    #[must_use]
    pub fn load(root: &Path, explicit: Option<&Path>) -> Self {
        Self::load_with_xdg(root, explicit, xdg_config_path().as_deref())
    }

    #[must_use]
    pub fn load_with_xdg(root: &Path, explicit: Option<&Path>, xdg: Option<&Path>) -> Self {
        let mut cfg = Self::default();
        if let Some(xdg) = xdg
            && xdg.is_file()
        {
            // User-level config may set opener/editor/keys.
            cfg.merge_file(xdg, true);
        }
        let root_cfg = root.join(".wiki-reader.toml");
        if root_cfg.is_file() {
            // Collection-local config is untrusted for launchers and keybinds.
            cfg.merge_file(&root_cfg, false);
        }
        if let Some(path) = explicit
            && path.is_file()
        {
            // Explicit --config is operator-chosen and trusted.
            cfg.merge_file(path, true);
        } else if let Some(path) = explicit {
            cfg.diagnostics
                .push(format!("config file not found: {}", path.display()));
        }
        cfg
    }

    fn merge_file(&mut self, path: &Path, trusted: bool) {
        let Ok(text) = fs::read_to_string(path) else {
            self.diagnostics
                .push(format!("could not read config: {}", path.display()));
            return;
        };
        let table: toml::Table = match toml::from_str(&text) {
            Ok(t) => t,
            Err(err) => {
                self.diagnostics
                    .push(format!("invalid TOML ({}): {err}", path.display()));
                return;
            }
        };
        for key in table.keys() {
            if !matches!(
                key.as_str(),
                "exclude" | "nav" | "opener" | "editor" | "diagrams" | "theme" | "keys"
            ) {
                self.diagnostics.push(format!("unknown config key: {key}"));
            }
        }
        if let Some(v) = table.get("exclude") {
            match parse_string_list(v) {
                Ok(list) => {
                    for pat in list {
                        if !self.exclude.contains(&pat) {
                            self.exclude.push(pat);
                        }
                    }
                }
                Err(msg) => self.diagnostics.push(format!("exclude: {msg}")),
            }
        }
        if let Some(v) = table.get("nav") {
            match v.clone().try_into::<NavConfig>() {
                Ok(nav) => self.nav = nav,
                Err(err) => self.diagnostics.push(format!("nav: {err}")),
            }
        }
        if let Some(v) = table.get("opener") {
            if trusted {
                match expect_string(v, "opener") {
                    Ok(s) => self.opener = Some(s),
                    Err(msg) => self.diagnostics.push(msg),
                }
            } else {
                self.diagnostics.push(
                    "opener ignored in repo config; set it in ~/.config/wiki-reader/config.toml"
                        .into(),
                );
            }
        }
        if let Some(v) = table.get("editor") {
            if trusted {
                match expect_string(v, "editor") {
                    Ok(s) => self.editor = Some(s),
                    Err(msg) => self.diagnostics.push(msg),
                }
            } else {
                self.diagnostics.push(
                    "editor ignored in repo config; set it in ~/.config/wiki-reader/config.toml"
                        .into(),
                );
            }
        }
        if let Some(v) = table.get("diagrams") {
            match v.clone().try_into::<DiagramMode>() {
                Ok(d) => self.diagrams = d,
                Err(err) => self.diagnostics.push(format!("diagrams: {err}")),
            }
        }
        if let Some(v) = table.get("theme") {
            match expect_string(v, "theme") {
                Ok(s) => self.theme = Some(s),
                Err(msg) => self.diagnostics.push(msg),
            }
        }
        if let Some(v) = table.get("keys") {
            if trusted {
                match parse_string_map(v) {
                    Ok(map) => {
                        for (k, val) in map {
                            self.keys.insert(k, val);
                        }
                    }
                    Err(msg) => self.diagnostics.push(format!("keys: {msg}")),
                }
            } else {
                self.diagnostics.push(
                    "keys ignored in repo config; set them in ~/.config/wiki-reader/config.toml"
                        .into(),
                );
            }
        }
    }

    /// First diagnostic line for the status bar (if any).
    #[must_use]
    pub fn status_message(&self) -> Option<String> {
        self.diagnostics.first().cloned()
    }
}

fn expect_string(v: &toml::Value, key: &str) -> Result<String, String> {
    v.as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{key}: expected string"))
}

fn parse_string_list(v: &toml::Value) -> Result<Vec<String>, String> {
    match v {
        toml::Value::Array(items) => items
            .iter()
            .map(|i| {
                i.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "expected string array".to_owned())
            })
            .collect(),
        toml::Value::String(s) => Ok(vec![s.clone()]),
        _ => Err("expected string or array of strings".into()),
    }
}

fn parse_string_map(v: &toml::Value) -> Result<BTreeMap<String, String>, String> {
    let table = v.as_table().ok_or_else(|| "expected table".to_owned())?;
    let mut out = BTreeMap::new();
    for (k, val) in table {
        let s = val
            .as_str()
            .ok_or_else(|| format!("{k}: expected string"))?;
        out.insert(k.clone(), s.to_owned());
    }
    Ok(out)
}

/// `$XDG_CONFIG_HOME/wiki-reader/config.toml` or `~/.config/wiki-reader/config.toml`.
#[must_use]
pub fn xdg_config_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let xdg = xdg.trim();
        // XDG Base Directory Spec: empty means unset.
        if !xdg.is_empty() {
            let p = PathBuf::from(xdg);
            if p.is_absolute() {
                return Some(p.join("wiki-reader/config.toml"));
            }
        }
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/wiki-reader/config.toml"))
}

/// Build a [`globset::GlobSet`] from exclude patterns; bad patterns become diagnostics.
#[must_use]
pub fn build_exclude_set(patterns: &[String]) -> (Option<globset::GlobSet>, Vec<String>) {
    if patterns.is_empty() {
        return (None, Vec::new());
    }
    let mut builder = globset::GlobSetBuilder::new();
    let mut diags = Vec::new();
    for pat in patterns {
        match globset::Glob::new(pat) {
            Ok(g) => {
                builder.add(g);
            }
            Err(err) => diags.push(format!("exclude pattern {pat:?}: {err}")),
        }
    }
    match builder.build() {
        Ok(set) => (Some(set), diags),
        Err(err) => {
            diags.push(format!("exclude globset: {err}"));
            (None, diags)
        }
    }
}

/// True when `rel` (path relative to collection root) matches an exclude glob.
#[must_use]
pub fn path_excluded(set: Option<&globset::GlobSet>, rel: &Path) -> bool {
    let Some(set) = set else {
        return false;
    };
    set.is_match(rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn missing_file_yields_defaults() {
        let tmp = tempdir().unwrap();
        let cfg = load_isolated(tmp.path(), None);
        assert!(cfg.exclude.is_empty());
        assert_eq!(cfg.nav.labels, LabelMode::Title);
        assert!(cfg.diagnostics.is_empty());
    }

    #[test]
    fn bad_key_records_diagnostic_and_applies_others() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join(".wiki-reader.toml");
        fs::write(
            &path,
            r#"
exclude = ["drafts/**"]
unknown_thing = 1
nav = { labels = "filename" }
editor = 42
"#,
        )
        .unwrap();
        let cfg = load_isolated(tmp.path(), None);
        assert!(
            !cfg.diagnostics
                .iter()
                .any(|d| d.contains("must be a table")),
            "diags={:?}",
            cfg.diagnostics
        );
        assert_eq!(
            cfg.exclude,
            vec!["drafts/**".to_owned()],
            "diags={:?}",
            cfg.diagnostics
        );
        assert_eq!(cfg.nav.labels, LabelMode::Filename);
        assert!(cfg.diagnostics.iter().any(|d| d.contains("unknown_thing")));
        assert!(
            cfg.diagnostics
                .iter()
                .any(|d| d.contains("editor ignored in repo config")),
            "diags={:?}",
            cfg.diagnostics
        );
        assert!(cfg.editor.is_none());
    }

    #[test]
    fn explicit_config_overrides_root() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".wiki-reader.toml"), r#"editor = "vim""#).unwrap();
        let override_path = tmp.path().join("override.toml");
        fs::write(&override_path, r#"editor = "nvim""#).unwrap();
        let cfg = load_isolated(tmp.path(), Some(&override_path));
        assert_eq!(cfg.editor.as_deref(), Some("nvim"));
    }

    #[test]
    fn hostile_root_editor_never_reaches_launcher() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join(".wiki-reader.toml"),
            r#"exclude = ["secrets/**"]
editor = "touch /tmp/pwned"
opener = "sh -c 'evil'"
[keys]
quit = "X"
"#,
        )
        .unwrap();
        let cfg = load_isolated(tmp.path(), None);
        assert!(cfg.editor.is_none(), "editor={:?}", cfg.editor);
        assert!(cfg.opener.is_none(), "opener={:?}", cfg.opener);
        assert!(cfg.keys.is_empty(), "keys={:?}", cfg.keys);
        assert_eq!(cfg.exclude, vec!["secrets/**".to_owned()]);
        assert!(
            cfg.diagnostics
                .iter()
                .any(|d| d.contains("editor ignored in repo config")),
            "diags={:?}",
            cfg.diagnostics
        );
        assert!(
            cfg.diagnostics
                .iter()
                .any(|d| d.contains("opener ignored in repo config")),
            "diags={:?}",
            cfg.diagnostics
        );
        assert!(
            cfg.diagnostics
                .iter()
                .any(|d| d.contains("keys ignored in repo config")),
            "diags={:?}",
            cfg.diagnostics
        );
    }

    #[test]
    fn exclude_unions_across_files() {
        let tmp = tempdir().unwrap();
        let xdg = tmp.path().join("xdg.toml");
        fs::write(&xdg, r#"exclude = ["drafts/**"]"#).unwrap();
        fs::write(
            tmp.path().join(".wiki-reader.toml"),
            r#"exclude = ["secrets/**", "drafts/**"]"#,
        )
        .unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), None, Some(&xdg));
        assert_eq!(
            cfg.exclude,
            vec!["drafts/**".to_owned(), "secrets/**".to_owned()]
        );
    }

    #[test]
    fn keys_merge_later_wins_per_action() {
        let tmp = tempdir().unwrap();
        let xdg = tmp.path().join("xdg.toml");
        fs::write(
            &xdg,
            r#"
[keys]
quit = "q"
toggle-nav = "b"
"#,
        )
        .unwrap();
        let over = tmp.path().join("over.toml");
        fs::write(
            &over,
            r#"
[keys]
quit = "Q"
"#,
        )
        .unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), Some(&over), Some(&xdg));
        assert_eq!(cfg.keys.get("quit").map(String::as_str), Some("Q"));
        assert_eq!(cfg.keys.get("toggle-nav").map(String::as_str), Some("b"));
    }

    fn load_isolated(root: &Path, explicit: Option<&Path>) -> Config {
        // Skip the developer's real XDG config.
        Config::load_with_xdg(root, explicit, None)
    }
}
