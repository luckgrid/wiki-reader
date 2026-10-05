//! Reader configuration (excludes, nav labels, themes, etc.).
//!
//! See [product spec](../../../wiki/product/spec.md) (C1) and
//! [architecture overview](../../../wiki/architecture/overview.md).

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// How side-nav page labels are built; breadcrumbs and view footer always use titles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LabelMode {
    /// `nav_title` → `title` → H1 → humanized filename.
    #[default]
    Title,
    /// On-disk filename, including its extension.
    Filename,
}

impl LabelMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Filename => "filename",
        }
    }
}

/// Diagram render preference (ADR-0004 / P3-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagramMode {
    #[default]
    Auto,
    Image,
    Text,
    Source,
}

impl DiagramMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Image => "image",
            Self::Text => "text",
            Self::Source => "source",
        }
    }
}

/// Built-in colour preset (`theme = "dark" | "light" | "herdr"`; P3-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    /// Tuned for dark terminals; the default.
    #[default]
    Dark,
    /// Tuned for light terminals.
    Light,
    /// Dark, matching herdr's palette.
    Herdr,
}

impl ThemeName {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Herdr => "herdr",
        }
    }
}

/// How `y` (copy page path) formats the path (`copy.path`; P2-55).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CopyPathMode {
    /// Relative to the collection root (default).
    #[default]
    Relative,
    /// Absolute filesystem path.
    Absolute,
}

impl CopyPathMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relative => "relative",
            Self::Absolute => "absolute",
        }
    }
}

/// `[copy]` table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct CopyConfig {
    #[serde(default)]
    pub path: CopyPathMode,
}

/// Where the side nav sits (`nav.position`; P3-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NavPosition {
    /// Nav column on the left (default).
    #[default]
    Left,
    /// Nav column on the right.
    Right,
}

impl NavPosition {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// `[images]` table (P3-13).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImagesConfig {
    /// When false, skip graphics probe and always use text placeholders.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Tallest image/diagram slot in display rows (default 30).
    #[serde(default = "default_max_slot_rows")]
    pub max_slot_rows: u16,
}

impl Default for ImagesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_slot_rows: default_max_slot_rows(),
        }
    }
}

/// `[herdr]` table (P3-10).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HerdrConfig {
    /// Publish the current page to herdr's sidebar while running in a herdr pane.
    #[serde(default = "default_true")]
    pub publish: bool,
}

impl Default for HerdrConfig {
    fn default() -> Self {
        Self { publish: true }
    }
}

fn default_true() -> bool {
    true
}

fn default_max_slot_rows() -> u16 {
    30
}

/// Typed fields the options window can persist ([ADR-0018](../../../wiki/decisions/0018-config-write-path.md)).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigPatch {
    pub theme: Option<ThemeName>,
    pub nav_labels: Option<LabelMode>,
    pub nav_position: Option<NavPosition>,
    pub diagrams: Option<DiagramMode>,
    pub copy_path: Option<CopyPathMode>,
    pub images_enabled: Option<bool>,
    pub images_max_slot_rows: Option<u16>,
}

/// `[nav]` table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct NavConfig {
    #[serde(default)]
    pub labels: LabelMode,
    #[serde(default)]
    pub position: NavPosition,
}

/// Loaded reader config plus per-key diagnostics (never a hard startup failure).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    pub exclude: Vec<String>,
    pub nav: NavConfig,
    pub opener: Option<String>,
    pub editor: Option<String>,
    pub diagrams: DiagramMode,
    /// Built-in colour preset. An unknown value in a later file is ignored (with a diagnostic),
    /// so the value from an earlier file, or `dark`, stays; same rule as `diagrams`.
    pub theme: ThemeName,
    /// True once a config file set a valid `theme`; without it, running inside herdr defaults the
    /// theme to `herdr`.
    pub theme_set: bool,
    pub copy: CopyConfig,
    pub images: ImagesConfig,
    pub herdr: HerdrConfig,
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

    #[allow(clippy::too_many_lines)] // one merge path per config key
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
                "exclude"
                    | "nav"
                    | "opener"
                    | "editor"
                    | "diagrams"
                    | "theme"
                    | "copy"
                    | "images"
                    | "herdr"
                    | "keys"
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
            if let Some(nav_table) = v.as_table() {
                if let Some(labels) = nav_table.get("labels") {
                    if labels.as_str() == Some("title+filename") {
                        self.nav.labels = LabelMode::Title;
                        let warning = "nav.labels: title+filename is deprecated; using title";
                        if !self.diagnostics.iter().any(|d| d == warning) {
                            self.diagnostics.push(warning.into());
                        }
                    } else {
                        match labels.clone().try_into::<LabelMode>() {
                            Ok(l) => self.nav.labels = l,
                            Err(err) => self.diagnostics.push(format!("nav.labels: {err}")),
                        }
                    }
                }
                if let Some(position) = nav_table.get("position") {
                    match position.clone().try_into::<NavPosition>() {
                        Ok(p) => self.nav.position = p,
                        Err(_) => self.diagnostics.push(format!(
                            "nav.position: expected \"left\" or \"right\", got {position}"
                        )),
                    }
                }
            } else {
                self.diagnostics
                    .push(format!("nav: expected table, got {v}"));
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
            match v.clone().try_into::<ThemeName>() {
                Ok(t) => {
                    self.theme = t;
                    self.theme_set = true;
                }
                Err(_) => self.diagnostics.push(format!(
                    "theme: expected \"dark\", \"light\" or \"herdr\", got {v}"
                )),
            }
        }
        if let Some(v) = table.get("copy") {
            if let Some(copy_table) = v.as_table() {
                // ponytail: only `path` today; unknown nested keys ignored until P3-13 grows the table
                if let Some(path_v) = copy_table.get("path") {
                    match path_v.clone().try_into::<CopyPathMode>() {
                        Ok(p) => self.copy.path = p,
                        Err(_) => self.diagnostics.push(format!(
                            "copy.path: expected \"relative\" or \"absolute\", got {path_v}"
                        )),
                    }
                }
            } else {
                self.diagnostics
                    .push(format!("copy: expected table, got {v}"));
            }
        }
        if let Some(v) = table.get("images") {
            if let Some(img) = v.as_table() {
                if let Some(en) = img.get("enabled") {
                    match en.as_bool() {
                        Some(b) => self.images.enabled = b,
                        None => self
                            .diagnostics
                            .push(format!("images.enabled: expected bool, got {en}")),
                    }
                }
                if let Some(rows) = img.get("max_slot_rows") {
                    match rows.as_integer().and_then(|n| u16::try_from(n).ok()) {
                        Some(n) if (1..=60).contains(&n) => self.images.max_slot_rows = n,
                        Some(_) => self
                            .diagnostics
                            .push(format!("images.max_slot_rows: expected 1..=60, got {rows}")),
                        None => self.diagnostics.push(format!(
                            "images.max_slot_rows: expected integer 1..=60, got {rows}"
                        )),
                    }
                }
            } else {
                self.diagnostics
                    .push(format!("images: expected table, got {v}"));
            }
        }
        if let Some(v) = table.get("herdr") {
            if let Some(herdr) = v.as_table() {
                if let Some(publish) = herdr.get("publish") {
                    match publish.as_bool() {
                        Some(b) => self.herdr.publish = b,
                        None => self
                            .diagnostics
                            .push(format!("herdr.publish: expected bool, got {publish}")),
                    }
                }
            } else {
                self.diagnostics
                    .push(format!("herdr: expected table, got {v}"));
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

/// File the options window should write ([ADR-0018](../../../wiki/decisions/0018-config-write-path.md)).
///
/// Explicit `--config` wins; otherwise the XDG user config path. Never the
/// collection `.wiki-reader.toml`.
#[must_use]
pub fn write_target(explicit_config: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit_config {
        return Some(path.to_path_buf());
    }
    xdg_config_path()
}

/// Apply `update` to `path`, preserving comments and unknown keys.
///
/// Creates the parent directory and the file when missing.
///
/// # Errors
///
/// Returns a human-readable message on I/O or parse failure.
pub fn write_patch(path: &Path, update: &ConfigPatch) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
    }
    let mut doc = if path.is_file() {
        let text = fs::read_to_string(path)
            .map_err(|err| format!("could not read {}: {err}", path.display()))?;
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|err| format!("invalid TOML ({}): {err}", path.display()))?
    } else {
        toml_edit::DocumentMut::new()
    };

    if let Some(theme) = update.theme {
        doc["theme"] = toml_edit::value(theme.as_str());
    }
    if update.nav_labels.is_some() || update.nav_position.is_some() {
        let nav = doc["nav"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        let table = nav
            .as_table_mut()
            .ok_or_else(|| "nav: expected table".to_owned())?;
        if let Some(labels) = update.nav_labels {
            table["labels"] = toml_edit::value(labels.as_str());
        }
        if let Some(position) = update.nav_position {
            table["position"] = toml_edit::value(position.as_str());
        }
    }
    if let Some(diagrams) = update.diagrams {
        doc["diagrams"] = toml_edit::value(diagrams.as_str());
    }
    if let Some(copy_path) = update.copy_path {
        let copy = doc["copy"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        let table = copy
            .as_table_mut()
            .ok_or_else(|| "copy: expected table".to_owned())?;
        table["path"] = toml_edit::value(copy_path.as_str());
    }
    if update.images_enabled.is_some() || update.images_max_slot_rows.is_some() {
        let images = doc["images"].or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        let table = images
            .as_table_mut()
            .ok_or_else(|| "images: expected table".to_owned())?;
        if let Some(enabled) = update.images_enabled {
            table["enabled"] = toml_edit::value(enabled);
        }
        if let Some(rows) = update.images_max_slot_rows {
            table["max_slot_rows"] = toml_edit::value(i64::from(rows));
        }
    }

    // ponytail: options patches scalar/table keys only; inline-table rewrite needs a full
    // document round-trip if we ever write nested inline tables here.
    let text = doc.to_string();
    // Resolve symlinks first so rename updates the target and the link stays a link (N16).
    let target = if path.exists() {
        fs::canonicalize(path)
            .map_err(|err| format!("could not resolve {}: {err}", path.display()))?
    } else {
        path.to_path_buf()
    };
    let parent = target.parent().filter(|p| !p.as_os_str().is_empty());
    let tmp = match parent {
        Some(dir) => dir.join(format!(
            ".{}.tmp-{}",
            target
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("config.toml"),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        )),
        None => target.with_extension(format!(
            "toml.tmp-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        )),
    };
    let write_tmp = || -> Result<(), String> {
        let mut file = fs::File::create(&tmp)
            .map_err(|err| format!("could not create {}: {err}", tmp.display()))?;
        if let Ok(meta) = fs::metadata(&target) {
            file.set_permissions(meta.permissions())
                .map_err(|err| format!("could not set permissions on {}: {err}", tmp.display()))?;
        }
        file.write_all(text.as_bytes())
            .map_err(|err| format!("could not write {}: {err}", tmp.display()))?;
        file.sync_all()
            .map_err(|err| format!("could not sync {}: {err}", tmp.display()))?;
        fs::rename(&tmp, &target)
            .map_err(|err| format!("could not write {}: {err}", target.display()))?;
        Ok(())
    };
    if let Err(err) = write_tmp() {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(())
}

/// Keys in `update` that the collection file at `path` also sets.
///
/// The collection `.wiki-reader.toml` merges after the user file, so a key set there wins on the
/// next launch over a value the options window just saved to the user file. An unreadable or
/// invalid file shadows nothing (the merge reports it separately).
#[must_use]
pub fn shadowed_keys(path: &Path, update: &ConfigPatch) -> Vec<&'static str> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(table) = toml::from_str::<toml::Table>(&text) else {
        return Vec::new();
    };
    let has = |section: Option<&str>, key: &str| match section {
        None => table.contains_key(key),
        Some(s) => table
            .get(s)
            .and_then(toml::Value::as_table)
            .is_some_and(|t| t.contains_key(key)),
    };
    let candidates = [
        (update.theme.is_some(), None, "theme", "theme"),
        (
            update.nav_labels.is_some(),
            Some("nav"),
            "labels",
            "nav.labels",
        ),
        (
            update.nav_position.is_some(),
            Some("nav"),
            "position",
            "nav.position",
        ),
        (update.diagrams.is_some(), None, "diagrams", "diagrams"),
        (
            update.copy_path.is_some(),
            Some("copy"),
            "path",
            "copy.path",
        ),
        (
            update.images_enabled.is_some(),
            Some("images"),
            "enabled",
            "images.enabled",
        ),
        (
            update.images_max_slot_rows.is_some(),
            Some("images"),
            "max_slot_rows",
            "images.max_slot_rows",
        ),
    ];
    candidates
        .into_iter()
        .filter(|(patched, section, key, _)| *patched && has(*section, key))
        .map(|(_, _, _, label)| label)
        .collect()
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
    fn herdr_publish_defaults_on_and_parses() {
        let tmp = tempdir().unwrap();
        assert!(load_isolated(tmp.path(), None).herdr.publish);
        let file = tmp.path().join("c.toml");
        fs::write(&file, "[herdr]\npublish = false\n").unwrap();
        let cfg = load_isolated(tmp.path(), Some(&file));
        assert!(!cfg.herdr.publish);
        assert!(cfg.diagnostics.is_empty(), "{:?}", cfg.diagnostics);
        // A later layer can turn it back on; bad values keep the earlier one.
        fs::write(&file, "[herdr]\npublish = \"no\"\n").unwrap();
        let cfg = load_isolated(tmp.path(), Some(&file));
        assert!(cfg.herdr.publish);
        assert!(cfg.diagnostics[0].contains("herdr.publish: expected bool"));
        fs::write(&file, "herdr = 3\n").unwrap();
        let cfg = load_isolated(tmp.path(), Some(&file));
        assert!(cfg.diagnostics[0].contains("herdr: expected table"));
    }

    #[test]
    fn legacy_labels_migrate_once_across_config_layers() {
        let tmp = tempdir().unwrap();
        let user = tmp.path().join("user.toml");
        let explicit = tmp.path().join("explicit.toml");
        for path in [&user, &explicit, &tmp.path().join(".wiki-reader.toml")] {
            fs::write(path, "[nav]\nlabels = \"title+filename\"\n").unwrap();
        }
        let cfg = Config::load_with_xdg(tmp.path(), Some(&explicit), Some(&user));
        assert_eq!(cfg.nav.labels, LabelMode::Title);
        assert_eq!(cfg.diagnostics.len(), 1);
        assert!(cfg.diagnostics[0].contains("deprecated; using title"));
        fs::write(&explicit, "[nav]\nlabels = \"filename\"\n").unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), Some(&explicit), Some(&user));
        assert_eq!(cfg.nav.labels, LabelMode::Filename);
        assert_eq!(cfg.diagnostics.len(), 1);
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
    fn theme_key_selects_a_preset() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("user.toml");
        for (value, want) in [
            ("dark", ThemeName::Dark),
            ("light", ThemeName::Light),
            ("herdr", ThemeName::Herdr),
        ] {
            fs::write(&path, format!("theme = \"{value}\"")).unwrap();
            let cfg = Config::load_with_xdg(tmp.path(), None, Some(&path));
            assert_eq!(cfg.theme, want);
            assert!(cfg.diagnostics.is_empty(), "diags={:?}", cfg.diagnostics);
        }
    }

    #[test]
    fn unknown_theme_alone_stays_dark_with_a_diagnostic() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("user.toml");
        fs::write(&path, r#"theme = "bogus""#).unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), None, Some(&path));
        assert_eq!(cfg.theme, ThemeName::Dark);
        assert!(
            cfg.diagnostics.iter().any(|d| d.starts_with("theme:")),
            "diags={:?}",
            cfg.diagnostics
        );
    }

    #[test]
    fn invalid_theme_in_a_later_file_keeps_the_earlier_value() {
        let tmp = tempdir().unwrap();
        let xdg = tmp.path().join("user.toml");
        fs::write(&xdg, r#"theme = "light""#).unwrap();
        fs::write(tmp.path().join(".wiki-reader.toml"), r#"theme = "neon""#).unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), None, Some(&xdg));
        assert_eq!(
            cfg.theme,
            ThemeName::Light,
            "a typo must not flip the user's theme"
        );
        assert!(
            cfg.diagnostics.iter().any(|d| d.starts_with("theme:")),
            "diags={:?}",
            cfg.diagnostics
        );
        // A valid later value still overrides.
        fs::write(tmp.path().join(".wiki-reader.toml"), r#"theme = "herdr""#).unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), None, Some(&xdg));
        assert_eq!(cfg.theme, ThemeName::Herdr);
    }

    #[test]
    fn missing_theme_is_dark_without_a_diagnostic() {
        let tmp = tempdir().unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), None, None);
        assert_eq!(cfg.theme, ThemeName::Dark);
        assert!(cfg.diagnostics.is_empty(), "diags={:?}", cfg.diagnostics);
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
    fn nav_position_key_selects_left_or_right() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("user.toml");
        for (value, want) in [("left", NavPosition::Left), ("right", NavPosition::Right)] {
            fs::write(&path, format!("[nav]\nposition = \"{value}\"\n")).unwrap();
            let cfg = Config::load_with_xdg(tmp.path(), None, Some(&path));
            assert_eq!(cfg.nav.position, want);
            assert!(cfg.diagnostics.is_empty(), "diags={:?}", cfg.diagnostics);
        }
    }

    #[test]
    fn invalid_nav_position_in_a_later_file_keeps_the_earlier_value() {
        let tmp = tempdir().unwrap();
        let xdg = tmp.path().join("user.toml");
        fs::write(&xdg, "[nav]\nposition = \"right\"\n").unwrap();
        fs::write(
            tmp.path().join(".wiki-reader.toml"),
            "[nav]\nposition = \"top\"\n",
        )
        .unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), None, Some(&xdg));
        assert_eq!(cfg.nav.position, NavPosition::Right);
        assert!(
            cfg.diagnostics
                .iter()
                .any(|d| d.starts_with("nav.position:")),
            "diags={:?}",
            cfg.diagnostics
        );
        // A later file that only sets labels must not wipe position.
        fs::write(
            tmp.path().join(".wiki-reader.toml"),
            "[nav]\nlabels = \"filename\"\n",
        )
        .unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), None, Some(&xdg));
        assert_eq!(cfg.nav.position, NavPosition::Right);
        assert_eq!(cfg.nav.labels, LabelMode::Filename);
    }

    #[test]
    fn copy_path_key_selects_relative_or_absolute() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("user.toml");
        for (value, want) in [
            ("relative", CopyPathMode::Relative),
            ("absolute", CopyPathMode::Absolute),
        ] {
            fs::write(&path, format!("[copy]\npath = \"{value}\"\n")).unwrap();
            let cfg = Config::load_with_xdg(tmp.path(), None, Some(&path));
            assert_eq!(cfg.copy.path, want);
            assert!(cfg.diagnostics.is_empty(), "diags={:?}", cfg.diagnostics);
        }
    }

    #[test]
    fn invalid_copy_path_stays_relative_with_a_diagnostic() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("user.toml");
        fs::write(&path, "[copy]\npath = \"bogus\"\n").unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), None, Some(&path));
        assert_eq!(cfg.copy.path, CopyPathMode::Relative);
        assert!(
            cfg.diagnostics.iter().any(|d| d.starts_with("copy.path:")),
            "diags={:?}",
            cfg.diagnostics
        );
    }

    #[test]
    fn invalid_copy_path_in_a_later_file_keeps_the_earlier_value() {
        let tmp = tempdir().unwrap();
        let xdg = tmp.path().join("user.toml");
        fs::write(&xdg, "[copy]\npath = \"absolute\"\n").unwrap();
        fs::write(
            tmp.path().join(".wiki-reader.toml"),
            "[copy]\npath = \"bogus\"\n",
        )
        .unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), None, Some(&xdg));
        assert_eq!(
            cfg.copy.path,
            CopyPathMode::Absolute,
            "a typo must not flip the user's copy.path"
        );
        assert!(
            cfg.diagnostics.iter().any(|d| d.starts_with("copy.path:")),
            "diags={:?}",
            cfg.diagnostics
        );
        fs::write(
            tmp.path().join(".wiki-reader.toml"),
            "[copy]\npath = \"relative\"\n",
        )
        .unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), None, Some(&xdg));
        assert_eq!(cfg.copy.path, CopyPathMode::Relative);
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

    #[test]
    fn write_target_prefers_explicit_config() {
        let path = PathBuf::from("/tmp/explicit.toml");
        assert_eq!(write_target(Some(&path)), Some(path));
    }

    #[test]
    fn write_patch_creates_file_and_round_trips() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("nested/config.toml");
        write_patch(
            &path,
            &ConfigPatch {
                theme: Some(ThemeName::Light),
                nav_position: Some(NavPosition::Right),
                nav_labels: Some(LabelMode::Filename),
                diagrams: Some(DiagramMode::Text),
                copy_path: Some(CopyPathMode::Absolute),
                images_enabled: Some(false),
                images_max_slot_rows: Some(40),
            },
        )
        .unwrap();
        let cfg = Config::load_with_xdg(tmp.path(), Some(&path), None);
        assert_eq!(cfg.theme, ThemeName::Light);
        assert_eq!(cfg.nav.position, NavPosition::Right);
        assert_eq!(cfg.nav.labels, LabelMode::Filename);
        assert_eq!(cfg.diagrams, DiagramMode::Text);
        assert_eq!(cfg.copy.path, CopyPathMode::Absolute);
        assert!(!cfg.images.enabled);
        assert_eq!(cfg.images.max_slot_rows, 40);
        assert!(cfg.diagnostics.is_empty(), "diags={:?}", cfg.diagnostics);
    }

    #[test]
    fn write_patch_is_atomic_and_cleans_temp_on_success() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        fs::write(&path, "theme = \"dark\"\n").unwrap();
        write_patch(
            &path,
            &ConfigPatch {
                theme: Some(ThemeName::Light),
                ..ConfigPatch::default()
            },
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("light"), "text={text}");
        let leftovers: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .filter(|n| n.to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "temp left behind: {leftovers:?}");
    }

    #[test]
    fn write_patch_preserves_symlink() {
        let tmp = tempdir().unwrap();
        let real = tmp.path().join("real.toml");
        let link = tmp.path().join("config.toml");
        fs::write(&real, "theme = \"dark\"\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        #[cfg(not(unix))]
        {
            // Windows: skip if symlink creation needs elevation.
            if std::os::windows::fs::symlink_file(&real, &link).is_err() {
                return;
            }
        }
        write_patch(
            &link,
            &ConfigPatch {
                theme: Some(ThemeName::Light),
                ..ConfigPatch::default()
            },
        )
        .unwrap();
        assert!(
            link.symlink_metadata().unwrap().file_type().is_symlink(),
            "config path must stay a symlink"
        );
        let text = fs::read_to_string(&real).unwrap();
        assert!(text.contains("light"), "target text={text}");
        assert_eq!(fs::read_to_string(&link).unwrap(), text);
    }

    #[cfg(unix)]
    #[test]
    fn write_patch_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        fs::write(&path, "theme = \"dark\"\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        write_patch(
            &path,
            &ConfigPatch {
                theme: Some(ThemeName::Light),
                ..ConfigPatch::default()
            },
        )
        .unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "mode={mode:#o}");
        assert!(fs::read_to_string(&path).unwrap().contains("light"));
    }

    #[test]
    fn write_patch_keeps_comments_and_unknown_keys() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        fs::write(
            &path,
            "# keep me\ntheme = \"dark\"\nunknown_thing = 1\n[nav]\n# labels note\nlabels = \"title\"\n",
        )
        .unwrap();
        write_patch(
            &path,
            &ConfigPatch {
                theme: Some(ThemeName::Herdr),
                nav_position: Some(NavPosition::Right),
                ..ConfigPatch::default()
            },
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("# keep me"), "text={text}");
        assert!(text.contains("# labels note"), "text={text}");
        assert!(text.contains("unknown_thing"), "text={text}");
        assert!(text.contains("herdr"), "text={text}");
        assert!(text.contains("right"), "text={text}");
        let cfg = Config::load_with_xdg(tmp.path(), Some(&path), None);
        assert_eq!(cfg.theme, ThemeName::Herdr);
        assert_eq!(cfg.nav.position, NavPosition::Right);
        assert_eq!(cfg.nav.labels, LabelMode::Title);
        assert!(
            cfg.diagnostics.iter().any(|d| d.contains("unknown_thing")),
            "diags={:?}",
            cfg.diagnostics
        );
    }

    #[test]
    fn shadowed_keys_lists_only_patched_keys_the_collection_file_sets() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join(".wiki-reader.toml");
        fs::write(
            &path,
            "theme = \"light\"\n[nav]\nposition = \"right\"\n[images]\nenabled = false\n",
        )
        .unwrap();
        let update = ConfigPatch {
            theme: Some(ThemeName::Dark),
            nav_position: Some(NavPosition::Left),
            nav_labels: Some(LabelMode::Filename),
            copy_path: Some(CopyPathMode::Absolute),
            ..ConfigPatch::default()
        };
        // `nav.labels` is patched but not set in the file; `images.enabled` is set but not patched.
        assert_eq!(shadowed_keys(&path, &update), vec!["theme", "nav.position"]);
        assert!(shadowed_keys(&tmp.path().join("missing.toml"), &update).is_empty());
        fs::write(&path, "not = [valid").unwrap();
        assert!(shadowed_keys(&path, &update).is_empty());
    }
}
