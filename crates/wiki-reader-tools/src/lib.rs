//! Repository checks for the docs: relative links, heading anchors, required
//! frontmatter, and stale `docs/` references across `wiki/` and the root
//! `README.md`.
//!
//! Parsing is delegated to [`wiki_reader_core::parse`], so heading slugs and
//! link extraction match what the reader itself resolves.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use thiserror::Error;
use wiki_reader_core::parse::{MdLinkKind, ParsedPage, parse};

/// Why the check could not run (as opposed to finding problems in the docs).
#[derive(Debug, Error)]
pub enum Error {
    /// `wiki/` or `README.md` is missing from the given root.
    #[error("{} not found; expected a wiki-reader checkout", .0.display())]
    MissingRoot(PathBuf),
    /// A file or directory could not be read.
    #[error("cannot read {}: {source}", .path.display())]
    Io {
        /// The path that failed.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
}

/// Outcome of a completed check.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Number of markdown files scanned.
    pub files: usize,
    /// Problems found, in a stable order (sorted by file, then by kind and line).
    pub errors: Vec<String>,
}

struct Doc {
    path: PathBuf,
    rel: PathBuf,
    dir: PathBuf,
    page: ParsedPage,
}

/// Check `wiki/**/*.md` and `README.md` under `root`.
///
/// # Errors
///
/// Returns [`Error::MissingRoot`] when `wiki/` or `README.md` is absent, and
/// [`Error::Io`] when a file cannot be read (including invalid UTF-8), so an
/// unreadable file never shrinks the scan silently.
pub fn check(root: &Path) -> Result<Report, Error> {
    let root = root.canonicalize().map_err(io_error(root))?;
    let wiki = root.join("wiki");
    let readme = root.join("README.md");
    for required in [&wiki, &readme] {
        if !required.exists() {
            return Err(Error::MissingRoot(required.clone()));
        }
    }

    let mut paths = Vec::new();
    collect(&wiki, &mut paths)?;
    paths.push(readme);

    let mut docs = BTreeMap::new();
    for path in paths {
        let path = path.canonicalize().map_err(io_error(&path))?;
        let text = fs::read_to_string(&path).map_err(io_error(&path))?;
        let rel = path.strip_prefix(&root).unwrap_or(&path).to_path_buf();
        let dir = path.parent().unwrap_or(&root).to_path_buf();
        let page = parse(&text);
        docs.insert(
            path.clone(),
            Doc {
                path,
                rel,
                dir,
                page,
            },
        );
    }

    let mut anchors: BTreeMap<PathBuf, HashSet<String>> = docs
        .iter()
        .map(|(path, doc)| (path.clone(), heading_slugs(&doc.page)))
        .collect();
    let mut errors = Vec::new();
    for doc in docs.values() {
        check_doc(doc, &mut anchors, &mut errors)?;
    }
    check_version_sync(&root, &mut errors)?;
    check_license_files(&root, &mut errors)?;
    check_third_party_notices(&root, &mut errors)?;
    Ok(Report {
        files: docs.len(),
        errors,
    })
}

/// Keep release-facing version strings aligned with `[workspace.package].version`.
fn check_version_sync(root: &Path, errors: &mut Vec<String>) -> Result<(), Error> {
    let cargo = root.join("Cargo.toml");
    // Link-check unit trees only create wiki pages; skip when there is no workspace.
    if !cargo.is_file() {
        return Ok(());
    }
    let cargo_text = fs::read_to_string(&cargo).map_err(io_error(&cargo))?;
    let Some(workspace) = workspace_package_version(&cargo_text) else {
        errors.push("Cargo.toml: missing [workspace.package] version".into());
        return Ok(());
    };

    for (name, ver) in workspace_path_dep_versions(&cargo_text) {
        if ver != workspace {
            errors.push(format!(
                "Cargo.toml: [workspace.dependencies] {name} version {ver} != workspace {workspace}"
            ));
        }
    }

    let plugin = root.join("integrations/herdr/herdr-plugin.toml");
    let plugin_text = fs::read_to_string(&plugin).map_err(io_error(&plugin))?;
    match toml_string_value(&plugin_text, "version") {
        Some(ver) if ver == workspace => {}
        Some(ver) => errors.push(format!(
            "integrations/herdr/herdr-plugin.toml: version {ver} != workspace {workspace}"
        )),
        None => errors.push("integrations/herdr/herdr-plugin.toml: missing version".into()),
    }

    let readme = root.join("README.md");
    let readme_text = fs::read_to_string(&readme).map_err(io_error(&readme))?;
    if let Some(status) = readme_status_section(&readme_text) {
        let needle = format!("v{workspace}");
        if !status.contains(&needle) {
            errors.push(format!(
                "README.md: Status section must name workspace version {needle}"
            ));
        }
    } else {
        errors.push("README.md: missing ## Status section".into());
    }

    let dogfood = root.join("wiki/roadmap/dogfood-log.md");
    let dogfood_text = fs::read_to_string(&dogfood).map_err(io_error(&dogfood))?;
    if let Some(latest) = latest_dogfood_release(&dogfood_text)
        && more_than_one_patch_ahead(&latest, &workspace)
    {
        errors.push(format!(
            "wiki/roadmap/dogfood-log.md: latest ## {latest} is more than one patch ahead of workspace {workspace}"
        ));
    }
    Ok(())
}

/// Licence texts every published crate must carry, kept identical to the repository root.
const LICENSE_FILES: [&str; 3] = ["LICENSE-MIT", "LICENSE-APACHE", "NOTICE"];

/// crates.io packages only the crate directory, so each published crate carries its own copy of
/// the licence files. The copies must match the root files byte for byte.
fn check_license_files(root: &Path, errors: &mut Vec<String>) -> Result<(), Error> {
    let crates = root.join("crates");
    // Link-check unit trees have no workspace.
    if !root.join("Cargo.toml").is_file() || !crates.is_dir() {
        return Ok(());
    }
    let mut dirs: Vec<PathBuf> = fs::read_dir(&crates)
        .map_err(io_error(&crates))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.join("Cargo.toml").is_file())
        .collect();
    dirs.sort();
    for dir in dirs {
        let manifest = dir.join("Cargo.toml");
        let manifest_text = fs::read_to_string(&manifest).map_err(io_error(&manifest))?;
        if manifest_text
            .lines()
            .any(|l| l.trim().replace(' ', "") == "publish=false")
        {
            continue;
        }
        let name = dir
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        for file in LICENSE_FILES {
            let want_path = root.join(file);
            let want = fs::read(&want_path).map_err(io_error(&want_path))?;
            match fs::read(dir.join(file)) {
                Ok(have) if have == want => {}
                Ok(_) => errors.push(format!(
                    "crates/{name}/{file}: differs from the root {file}; copy the root file"
                )),
                Err(_) => errors.push(format!(
                    "crates/{name}/{file}: missing (published crates carry the root {file})"
                )),
            }
        }
    }
    Ok(())
}

/// Third-party (non-path) names in `[workspace.dependencies]`.
fn workspace_third_party_deps(cargo: &str) -> Vec<String> {
    let mut in_deps = false;
    let mut names = Vec::new();
    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[workspace.dependencies]";
            continue;
        }
        if !in_deps || trimmed.is_empty() || trimmed.starts_with('#') || trimmed.contains("path =")
        {
            continue;
        }
        if let Some((name, _)) = trimmed.split_once('=') {
            names.push(name.trim().to_owned());
        }
    }
    names
}

/// Dependency names in a crate manifest's normal `[dependencies]` tables (including
/// `[target.*.dependencies]`), not dev- or build-dependencies.
fn crate_normal_deps(manifest: &str) -> Vec<String> {
    let mut in_deps = false;
    let mut names = Vec::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]"
                || (trimmed.starts_with("[target.") && trimmed.ends_with(".dependencies]"));
            continue;
        }
        if in_deps
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && let Some((name, _)) = trimmed.split_once('=')
        {
            names.push(name.trim().to_owned());
        }
    }
    names
}

/// Versions per package name from `Cargo.lock` (several when a crate appears twice).
fn lock_versions(lock: &str) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut name: Option<String> = None;
    for line in lock.lines() {
        if let Some(n) = quoted_assignment(line, "name") {
            name = Some(n);
        } else if let (Some(v), Some(n)) = (quoted_assignment(line, "version"), name.take()) {
            map.entry(n).or_default().push(v);
        }
    }
    map
}

/// Binary releases ship `THIRD-PARTY-LICENSES`; it must list every direct third-party dependency
/// at its locked version, so a dependency change without `scripts/gen-third-party-licenses.sh`
/// fails here.
fn check_third_party_notices(root: &Path, errors: &mut Vec<String>) -> Result<(), Error> {
    let cargo = root.join("Cargo.toml");
    let lock = root.join("Cargo.lock");
    if !cargo.is_file() || !lock.is_file() {
        return Ok(());
    }
    let notices = root.join("THIRD-PARTY-LICENSES");
    let Ok(text) = fs::read_to_string(&notices) else {
        errors
            .push("THIRD-PARTY-LICENSES: missing; run scripts/gen-third-party-licenses.sh".into());
        return Ok(());
    };
    let cargo_text = fs::read_to_string(&cargo).map_err(io_error(&cargo))?;
    let lock_text = fs::read_to_string(&lock).map_err(io_error(&lock))?;
    let versions = lock_versions(&lock_text);
    let third_party: HashSet<String> = workspace_third_party_deps(&cargo_text)
        .into_iter()
        .collect();
    // Only what a published crate links at runtime ships in the binary; dev-dependencies and the
    // unpublished tools crate do not.
    let mut shipped: BTreeSet<String> = BTreeSet::new();
    let crates = root.join("crates");
    if let Ok(entries) = fs::read_dir(&crates) {
        for entry in entries.filter_map(Result::ok) {
            let manifest = entry.path().join("Cargo.toml");
            let Ok(text) = fs::read_to_string(&manifest) else {
                continue;
            };
            if text
                .lines()
                .any(|l| l.trim().replace(' ', "") == "publish=false")
            {
                continue;
            }
            shipped.extend(
                crate_normal_deps(&text)
                    .into_iter()
                    .filter(|d| third_party.contains(d)),
            );
        }
    }
    for dep in shipped {
        let Some(vers) = versions.get(&dep) else {
            continue;
        };
        // Every locked version counts: a crate can be both a direct dependency and a transitive
        // one at another version (signal-hook 0.3 via crossterm, 0.4 direct), and a bump of one
        // must not hide behind the other.
        let missing: Vec<&String> = vers
            .iter()
            .filter(|v| !text.contains(&format!("* {dep} {v}")))
            .collect();
        if !missing.is_empty() {
            errors.push(format!(
                "THIRD-PARTY-LICENSES: {dep} {} is not listed; run scripts/gen-third-party-licenses.sh",
                missing.iter().map(|v| v.as_str()).collect::<Vec<_>>().join(" / ")
            ));
        }
    }
    Ok(())
}

fn workspace_package_version(cargo: &str) -> Option<String> {
    let mut in_pkg = false;
    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_pkg = trimmed == "[workspace.package]";
            continue;
        }
        if in_pkg && let Some(ver) = quoted_assignment(trimmed, "version") {
            return Some(ver);
        }
    }
    None
}

fn workspace_path_dep_versions(cargo: &str) -> Vec<(String, String)> {
    let mut in_deps = false;
    let mut out = Vec::new();
    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[workspace.dependencies]";
            continue;
        }
        if !in_deps || trimmed.starts_with('#') || !trimmed.contains("path =") {
            continue;
        }
        let Some((name, rest)) = trimmed.split_once('=') else {
            continue;
        };
        let name = name.trim().to_owned();
        // wiki-reader-core = { path = "...", version = "0.1.9" }
        if let Some(ver) = rest
            .split(',')
            .find_map(|part| quoted_assignment(part.trim(), "version"))
        {
            out.push((name, ver));
        }
    }
    out
}

fn toml_string_value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find_map(|line| quoted_assignment(line.trim(), key))
}

fn quoted_assignment(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = \"");
    let rest = line.strip_prefix(&prefix)?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn readme_status_section(readme: &str) -> Option<&str> {
    let start = readme.find("\n## Status\n")?;
    let after = &readme[start + "\n## Status\n".len()..];
    let end = after.find("\n## ").unwrap_or(after.len());
    Some(&after[..end])
}

fn latest_dogfood_release(dogfood: &str) -> Option<String> {
    dogfood
        .lines()
        .filter_map(|line| line.strip_prefix("## v"))
        .filter(|v| is_plain_semver(v))
        .max_by(|a, b| version_triple(a).cmp(&version_triple(b)))
        .map(str::to_owned)
}

fn is_plain_semver(v: &str) -> bool {
    // Alpha tags like 0.1.0-alpha.1 are ignored; only plain 0.1.N headings.
    let mut parts = v.split('.');
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    parts.next().is_some_and(digits)
        && parts.next().is_some_and(digits)
        && parts.next().is_some_and(digits)
        && parts.next().is_none()
}

fn version_triple(ver: &str) -> (u32, u32, u32) {
    let mut parts = ver.split('.');
    let parse = |p: Option<&str>| p.and_then(|s| s.parse().ok()).unwrap_or(0);
    (
        parse(parts.next()),
        parse(parts.next()),
        parse(parts.next()),
    )
}

/// True when `latest` is newer than workspace by more than one patch (same or
/// higher major/minor). Equal and one patch ahead are allowed so a dogfood
/// heading can land before the Cargo bump.
fn more_than_one_patch_ahead(latest: &str, workspace: &str) -> bool {
    let (lm, lmi, lp) = version_triple(latest);
    let (wm, wmi, wp) = version_triple(workspace);
    (lm, lmi, lp) > (wm, wmi, wp.saturating_add(1))
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> Error + use<> {
    let path = path.to_path_buf();
    move |source| Error::Io { path, source }
}

/// Markdown files under `dir`, sorted. Symlinks are skipped, which also rules
/// out directory cycles.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), Error> {
    let mut entries = fs::read_dir(dir)
        .and_then(Iterator::collect::<io::Result<Vec<_>>>)
        .map_err(io_error(dir))?;
    entries.sort_by_cached_key(fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        let kind = entry.file_type().map_err(io_error(&path))?;
        if kind.is_dir() {
            collect(&path, out)?;
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
    Ok(())
}

fn heading_slugs(page: &ParsedPage) -> HashSet<String> {
    page.headings.iter().map(|h| h.slug.clone()).collect()
}

fn check_doc(
    doc: &Doc,
    anchors: &mut BTreeMap<PathBuf, HashSet<String>>,
    errors: &mut Vec<String>,
) -> Result<(), Error> {
    let rel = doc.rel.display();

    let is_readme = doc.rel.file_name().is_some_and(|name| name == "README.md");
    if doc.rel.starts_with("wiki") && !is_readme {
        let fm = &doc.page.frontmatter;
        for (key, present) in [
            ("id", fm.id.is_some()),
            ("title", fm.title.is_some()),
            ("summary", fm.summary.is_some()),
            ("status", fm.status.is_some()),
        ] {
            if !present {
                errors.push(format!("{rel}: missing frontmatter `{key}`"));
            }
        }
        if let Some(status) = fm.status.as_deref()
            && wiki_reader_core::DocStatus::parse(status).is_none()
        {
            errors.push(format!("{rel}: unknown status '{status}'"));
        }
    }

    for (line, reference) in stale_docs_refs(&doc.page) {
        errors.push(format!(
            "{rel}:{line}: lingering docs/ reference `{reference}`"
        ));
    }

    for link in &doc.page.links {
        let target = link.target.trim();
        if link.kind == MdLinkKind::External || is_foreign_scheme(target) {
            continue;
        }
        let line = link.source_line;
        let (path_part, fragment) = target.split_once('#').unwrap_or((target, ""));
        let dest = if path_part.is_empty() {
            doc.path.clone()
        } else {
            match doc.dir.join(path_part).canonicalize() {
                Ok(dest) => dest,
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    errors.push(format!("{rel}:{line}: broken link `{target}`"));
                    continue;
                }
                Err(err) => return Err(io_error(&doc.dir.join(path_part))(err)),
            }
        };
        if fragment.is_empty() {
            continue;
        }
        if let Some(slugs) = anchor_set(&dest, anchors)?
            && !slugs.contains(fragment)
        {
            let what = if path_part.is_empty() {
                "anchor"
            } else {
                "anchor in"
            };
            errors.push(format!("{rel}:{line}: broken {what} `{target}`"));
        }
    }
    Ok(())
}

/// Schemes the reader does not resolve as files. Aligns with
/// [`wiki_reader_core::nav::uri_scheme`]: any URI scheme other than those already
/// classified as external by core (`http` / `https` / `mailto`).
fn is_foreign_scheme(target: &str) -> bool {
    wiki_reader_core::nav::uri_scheme(target).is_some()
}

/// Heading slugs of a link destination, read on demand for markdown files
/// outside the scanned set. Non-markdown destinations have no anchors to check.
fn anchor_set<'a>(
    dest: &Path,
    anchors: &'a mut BTreeMap<PathBuf, HashSet<String>>,
) -> Result<Option<&'a HashSet<String>>, Error> {
    if !anchors.contains_key(dest) {
        if dest.extension().is_none_or(|ext| ext != "md") {
            return Ok(None);
        }
        let text = fs::read_to_string(dest).map_err(io_error(dest))?;
        anchors.insert(dest.to_path_buf(), heading_slugs(&parse(&text)));
    }
    Ok(anchors.get(dest))
}

/// `docs/…` paths in prose and link destinations (the directory was renamed to
/// `wiki/`), skipping code blocks, inline code, and longer paths such as
/// `../docs/x` or `mydocs/x`. Returns `(source line, matched path)`.
fn stale_docs_refs(page: &ParsedPage) -> Vec<(u32, String)> {
    const NEEDLE: &str = "docs/";
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES;
    // `byte` mixes a source offset with an offset inside decoded text, so it may
    // be off a char boundary or past the end; counting over bytes never panics.
    let line_at = |byte: usize| {
        let newlines = page.body.bytes().take(byte).filter(|&b| b == b'\n').count();
        page.body_line_offset + u32::try_from(newlines).unwrap_or(u32::MAX)
    };

    let mut found = Vec::new();
    let mut scan = |text: &str, start: usize| {
        for (at, _) in text.match_indices(NEEDLE) {
            let preceded = text[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'));
            let len: usize = text[at..]
                .chars()
                .take_while(|c| {
                    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '#' | '-')
                })
                .map(char::len_utf8)
                .sum();
            if !preceded && len > NEEDLE.len() {
                found.push((line_at(start + at), text[at..at + len].to_owned()));
            }
        }
    };

    let mut in_code_block = false;
    for (event, range) in Parser::new_ext(&page.body, options).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
            Event::End(TagEnd::CodeBlock) => in_code_block = false,
            Event::Text(text) if !in_code_block => scan(&text, range.start),
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                scan(&dest_url, range.start);
            }
            _ => {}
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const FRONTMATTER: &str = "---\nid: x\ntitle: X\nsummary: s\nstatus: draft\n---\n";

    fn page(body: &str) -> String {
        format!("{FRONTMATTER}{body}")
    }

    /// A checkout with `wiki/` and a bare `README.md`, plus `files`.
    fn tree(files: &[(&str, &str)]) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("wiki")).unwrap();
        fs::write(dir.path().join("README.md"), "# Root\n").unwrap();
        for (name, body) in files {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, body).unwrap();
        }
        dir
    }

    fn errors(dir: &TempDir) -> Vec<String> {
        check(dir.path()).unwrap().errors
    }

    fn assert_clean(files: &[(&str, &str)]) {
        assert_eq!(errors(&tree(files)), Vec::<String>::new());
    }

    #[test]
    fn clean_tree_counts_readme_and_wiki_pages() {
        let dir = tree(&[
            ("wiki/a.md", &page("# A\n\n[b](b.md#b)\n")),
            ("wiki/b.md", &page("# B\n")),
        ]);
        let report = check(dir.path()).unwrap();
        assert_eq!(
            report,
            Report {
                files: 3,
                errors: vec![]
            }
        );
    }

    #[test]
    fn missing_root_pieces_are_reported_not_ok() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(check(dir.path()), Err(Error::MissingRoot(_))));
        fs::create_dir(dir.path().join("wiki")).unwrap();
        assert!(matches!(check(dir.path()), Err(Error::MissingRoot(_))));
    }

    #[test]
    fn broken_link_is_reported_with_line() {
        let dir = tree(&[("wiki/a.md", &page("# A\n\n[gone](nope.md)\n"))]);
        assert_eq!(errors(&dir), ["wiki/a.md:9: broken link `nope.md`"]);
    }

    #[test]
    fn same_page_anchors_are_checked() {
        assert_clean(&[("wiki/a.md", &page("# A Title\n\n[ok](#a-title)\n"))]);
        let dir = tree(&[("wiki/a.md", &page("# A\n\n[bad](#nope)\n"))]);
        assert_eq!(errors(&dir), ["wiki/a.md:9: broken anchor `#nope`"]);
    }

    #[test]
    fn duplicate_headings_get_numbered_slugs() {
        assert_clean(&[(
            "wiki/a.md",
            &page("# A\n\n## Same\n\n## Same\n\n[two](#same-1)\n"),
        )]);
    }

    #[test]
    fn anchors_into_other_pages_are_checked() {
        assert_clean(&[
            ("wiki/a.md", &page("# A\n\n[b](b.md#deep-dive)\n")),
            ("wiki/b.md", &page("# B\n\n## Deep dive\n")),
        ]);
        let dir = tree(&[
            ("wiki/a.md", &page("# A\n\n[b](b.md#missing)\n")),
            ("wiki/b.md", &page("# B\n")),
        ]);
        assert_eq!(
            errors(&dir),
            ["wiki/a.md:9: broken anchor in `b.md#missing`"]
        );
    }

    #[test]
    fn anchors_are_checked_in_markdown_outside_the_scan_set() {
        // README.md links to a root-level file that is not under wiki/.
        let dir = tree(&[("NOTES.md", "# Notes\n\n## Known\n")]);
        fs::write(
            dir.path().join("README.md"),
            "# Root\n\n[ok](NOTES.md#known) [bad](NOTES.md#nope)\n",
        )
        .unwrap();
        assert_eq!(
            errors(&dir),
            ["README.md:3: broken anchor in `NOTES.md#nope`"]
        );
    }

    #[test]
    fn code_and_images_are_not_checked_but_lookalike_links_are() {
        assert_clean(&[(
            "wiki/a.md",
            &page(
                "# A\n\n`[x](gone.md)` and ![img](gone.png)\n\n````md\n```\n[y](gone.md)\n```\n[z](gone.md)\n````\n\n~~~\n[w](gone.md)\n~~~\n",
            ),
        )]);
        // A trailing `!` in link text must not hide the link.
        let dir = tree(&[("wiki/a.md", &page("# A\n\n[Hello!](gone.md)\n"))]);
        assert_eq!(errors(&dir), ["wiki/a.md:9: broken link `gone.md`"]);
    }

    #[test]
    fn non_file_schemes_are_skipped() {
        assert_clean(&[(
            "wiki/a.md",
            &page(
                "# A\n\n[a](ftp://host/x) [b](tel:+15550100) [c](https://example.com) [d](mailto:a@b.c)\n",
            ),
        )]);
    }

    #[test]
    fn relative_links_named_like_schemes_are_still_checked() {
        let dir = tree(&[("wiki/a.md", &page("# A\n\n[e](http-notes.md)\n"))]);
        assert_eq!(errors(&dir), ["wiki/a.md:9: broken link `http-notes.md`"]);
    }

    #[test]
    fn stale_docs_paths_are_reported_with_the_matched_text() {
        let dir = tree(&[(
            "wiki/a.md",
            &page("# A\n\nSee docs/old/page.md for more.\n"),
        )]);
        assert_eq!(
            errors(&dir),
            ["wiki/a.md:9: lingering docs/ reference `docs/old/page.md`"]
        );
    }

    #[test]
    fn every_stale_docs_path_in_a_file_is_reported() {
        let dir = tree(&[("wiki/a.md", &page("# A\n\ndocs/one.md\n\ndocs/two.md\n"))]);
        assert_eq!(errors(&dir).len(), 2);
    }

    #[test]
    fn docs_paths_in_code_or_longer_paths_are_fine() {
        assert_clean(&[(
            "wiki/a.md",
            &page(
                "# A\n\n`docs/x.md` and ../docs/x.md and mydocs/x.md and a-docs/x.md\n\n```\ndocs/y.md\n```\n",
            ),
        )]);
    }

    #[test]
    fn non_ascii_before_a_docs_path_does_not_shift_the_match() {
        // Multi-byte characters in an earlier code span used to skew offsets.
        let dir = tree(&[("wiki/a.md", &page("# A\n\n`→ ☃ é` then docs/x.md\n"))]);
        assert_eq!(
            errors(&dir),
            ["wiki/a.md:9: lingering docs/ reference `docs/x.md`"]
        );
    }

    #[test]
    fn entities_and_escapes_before_a_docs_path_do_not_panic() {
        // Decoded text is shorter than its source, so offsets inside it are
        // only approximate; the report must still be produced.
        let dir = tree(&[(
            "wiki/a.md",
            &page("# A\n\n&amp; &copy; \\_ é ☃ &#x1F600; docs/x.md\n"),
        )]);
        let found = errors(&dir);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("lingering docs/ reference `docs/x.md`"));
    }

    #[test]
    fn frontmatter_is_required_for_wiki_pages_but_not_readmes() {
        let dir = tree(&[
            ("wiki/a.md", "---\nid: x\ntitle: X\n---\n# A\n"),
            ("wiki/sub/README.md", "# Index\n"),
        ]);
        assert_eq!(
            errors(&dir),
            [
                "wiki/a.md: missing frontmatter `summary`",
                "wiki/a.md: missing frontmatter `status`"
            ]
        );
    }

    #[test]
    fn unknown_status_is_rejected_for_wiki_pages() {
        let dir = tree(&[(
            "wiki/a.md",
            "---\nid: x\ntitle: X\nsummary: s\nstatus: complete\n---\n# A\n",
        )]);
        assert_eq!(errors(&dir), ["wiki/a.md: unknown status 'complete'"]);
    }

    #[test]
    fn output_order_is_deterministic() {
        let files = [
            ("wiki/z.md", page("# Z\n\n[a](a1.md)\n")),
            ("wiki/a.md", page("# A\n\n[b](b1.md)\n")),
            ("wiki/m/x.md", page("# X\n\n[c](c1.md)\n")),
        ];
        let owned: Vec<(&str, &str)> = files.iter().map(|(n, b)| (*n, b.as_str())).collect();
        let dir = tree(&owned);
        let first = errors(&dir);
        assert_eq!(first, errors(&dir));
        assert_eq!(
            first,
            [
                "wiki/a.md:9: broken link `b1.md`",
                "wiki/m/x.md:9: broken link `c1.md`",
                "wiki/z.md:9: broken link `a1.md`",
            ]
        );
    }

    #[test]
    fn unreadable_files_fail_the_run_instead_of_shrinking_it() {
        let dir = tree(&[]);
        fs::write(dir.path().join("wiki/bad.md"), [0xff, 0xfe, 0x00]).unwrap();
        assert!(matches!(check(dir.path()), Err(Error::Io { .. })));
    }
}

/// The UI diagram is shown in two places; keep them byte-identical.
#[cfg(test)]
mod docs_sync {
    use std::path::Path;

    use super::{
        is_plain_semver, latest_dogfood_release, more_than_one_patch_ahead, readme_status_section,
        version_triple, workspace_package_version, workspace_path_dep_versions,
    };

    const START: &str = "<!-- ui-diagram:start -->";
    const END: &str = "<!-- ui-diagram:end -->";

    fn diagram(rel: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        let doc = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        let start = doc
            .find(START)
            .unwrap_or_else(|| panic!("{rel}: missing {START}"));
        let end = doc
            .find(END)
            .unwrap_or_else(|| panic!("{rel}: missing {END}"));
        assert!(start < end, "{rel}: diagram markers out of order");
        doc[start + START.len()..end].to_owned()
    }

    fn license_tree(crate_files: &[(&str, &str)], manifest: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let w = |rel: &str, body: &str| {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        };
        w("Cargo.toml", "[workspace]\n");
        for f in ["LICENSE-MIT", "LICENSE-APACHE", "NOTICE"] {
            w(f, &format!("root {f}"));
        }
        w("crates/c/Cargo.toml", manifest);
        for (f, body) in crate_files {
            w(&format!("crates/c/{f}"), body);
        }
        dir
    }

    #[test]
    fn license_files_must_match_the_root_in_published_crates() {
        let good = license_tree(
            &[
                ("LICENSE-MIT", "root LICENSE-MIT"),
                ("LICENSE-APACHE", "root LICENSE-APACHE"),
                ("NOTICE", "root NOTICE"),
            ],
            "[package]\nname = \"c\"\n",
        );
        let mut errors = Vec::new();
        super::check_license_files(good.path(), &mut errors).unwrap();
        assert!(errors.is_empty(), "{errors:?}");

        let bad = license_tree(
            &[("LICENSE-MIT", "stale"), ("NOTICE", "root NOTICE")],
            "[package]\nname = \"c\"\n",
        );
        let mut errors = Vec::new();
        super::check_license_files(bad.path(), &mut errors).unwrap();
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("LICENSE-MIT") && e.contains("differs"))
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("LICENSE-APACHE") && e.contains("missing"))
        );
    }

    #[test]
    fn third_party_notices_must_list_every_direct_dependency() {
        let dir = tempfile::tempdir().unwrap();
        let w = |rel: &str, body: &str| std::fs::write(dir.path().join(rel), body).unwrap();
        w(
            "Cargo.toml",
            "[workspace.dependencies]\nserde = \"1\"\ninsta = \"1\"\nlocal = { path = \"crates/local\", version = \"0.1.0\" }\n",
        );
        w(
            "Cargo.lock",
            "[[package]]\nname = \"serde\"\nversion = \"1.0.2\"\n\n[[package]]\nname = \"insta\"\nversion = \"1.48.0\"\n\n[[package]]\nname = \"local\"\nversion = \"0.1.0\"\n",
        );
        std::fs::create_dir_all(dir.path().join("crates/local")).unwrap();
        w(
            "crates/local/Cargo.toml",
            "[package]\nname = \"local\"\n\n[dependencies]\nserde = { workspace = true }\n\n[dev-dependencies]\ninsta = { workspace = true }\n",
        );
        let mut errors = Vec::new();
        super::check_third_party_notices(dir.path(), &mut errors).unwrap();
        assert_eq!(errors.len(), 1, "missing file: {errors:?}");
        assert!(errors[0].contains("missing"));

        w("THIRD-PARTY-LICENSES", "  * serde 1.0.1 (old)\n");
        let mut errors = Vec::new();
        super::check_third_party_notices(dir.path(), &mut errors).unwrap();
        assert_eq!(errors.len(), 1, "stale version: {errors:?}");
        assert!(errors[0].contains("serde 1.0.2"));

        w(
            "THIRD-PARTY-LICENSES",
            "  * serde 1.0.2 (https://example.test)\n",
        );
        let mut errors = Vec::new();
        super::check_third_party_notices(dir.path(), &mut errors).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn unpublished_crates_need_no_license_copies() {
        let dir = license_tree(&[], "[package]\nname = \"c\"\npublish = false\n");
        let mut errors = Vec::new();
        super::check_license_files(dir.path(), &mut errors).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn readme_and_ui_spec_share_one_diagram() {
        let readme = diagram("../../README.md");
        let ui_spec = diagram("../../wiki/product/ui-spec.md");
        assert!(readme.contains("```text"), "README diagram block is empty");
        assert_eq!(
            readme, ui_spec,
            "README.md and wiki/product/ui-spec.md diagrams differ; edit both together"
        );
    }

    #[test]
    fn workspace_version_helpers_parse_path_deps_and_status() {
        let cargo = r#"
[workspace.package]
version = "0.1.9"

[workspace.dependencies]
wiki-reader-core = { path = "crates/wiki-reader-core", version = "0.1.9" }
other = "1.0"
"#;
        assert_eq!(workspace_package_version(cargo).as_deref(), Some("0.1.9"));
        assert_eq!(
            workspace_path_dep_versions(cargo),
            vec![("wiki-reader-core".into(), "0.1.9".into())]
        );
        let status =
            readme_status_section("intro\n\n## Status\n\nshipped through v0.1.9\n\n## Install\n");
        assert!(status.unwrap().contains("v0.1.9"));
        assert!(is_plain_semver("0.1.9"));
        assert!(!is_plain_semver("0.1.0-alpha.1"));
        assert_eq!(
            latest_dogfood_release("## v0.1.0-alpha.5\n## v0.1.8\n## v0.1.9\n").as_deref(),
            Some("0.1.9")
        );
        assert!(version_triple("0.1.10") > version_triple("0.1.9"));
        // Equal and one patch ahead OK; two patches ahead fails.
        assert!(!more_than_one_patch_ahead("0.1.9", "0.1.9"));
        assert!(!more_than_one_patch_ahead("0.1.10", "0.1.9"));
        assert!(more_than_one_patch_ahead("0.1.11", "0.1.9"));
        assert!(!more_than_one_patch_ahead("0.1.8", "0.1.9"));
    }
}
