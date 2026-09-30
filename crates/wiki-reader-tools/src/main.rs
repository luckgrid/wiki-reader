use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|e| e == "md") {
            out.push(path.to_path_buf());
        }
        return;
    }
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            collect(&entry.path(), out);
        }
    }
}
fn slug(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '_')
        .collect::<String>()
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .replace('_', "-")
}
fn prose(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut fence = false;
    let mut inline = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '`' {
            if chars.peek() == Some(&'`') {
                fence = !fence;
                chars.next();
            } else {
                inline = !inline;
            }
            out.push(' ');
        } else {
            out.push(if fence || inline {
                if c == '\n' { '\n' } else { ' ' }
            } else {
                c
            });
        }
    }
    out
}
fn anchors(s: &str) -> Vec<String> {
    s.lines()
        .filter_map(|line| {
            let t = line.trim_start();
            let n = t.chars().take_while(|c| *c == '#').count();
            (n > 0 && t.chars().nth(n) == Some(' ')).then(|| slug(&t[n + 1..]))
        })
        .collect()
}
fn main() -> ExitCode {
    let root = std::env::current_dir().expect("current directory");
    let mut paths = Vec::new();
    collect(&root.join("wiki"), &mut paths);
    paths.push(root.join("README.md"));
    let docs: HashMap<_, _> = paths
        .into_iter()
        .filter_map(|p| p.canonicalize().ok())
        .filter_map(|p| fs::read_to_string(&p).ok().map(|s| (p, s)))
        .collect();
    let mut errors = Vec::new();
    for (path, text) in &docs {
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let readme = path.file_name().is_some_and(|n| n == "README.md");
        if rel.starts_with("wiki") && !readme {
            let fm = text
                .strip_prefix("---\n")
                .and_then(|s| s.split_once("\n---\n"))
                .map_or(String::new(), |x| x.0.to_owned());
            for key in ["id", "title", "summary", "status"] {
                if !fm
                    .lines()
                    .any(|line| line.starts_with(key) && line[key.len()..].starts_with(':'))
                {
                    errors.push(format!("{}: missing frontmatter `{key}`", rel.display()));
                }
            }
        }
        let checkable = prose(text);
        for (start, _) in checkable.match_indices("docs/") {
            if start == 0 || !matches!(text.as_bytes()[start - 1], b'/' | b'.' | b'-' | b'_') {
                errors.push(format!("{}: lingering docs/ reference", rel.display()));
                break;
            }
        }
        let mut rest = checkable.as_str();
        while let Some(i) = rest.find("](") {
            let tail = &rest[i + 2..];
            let Some(end) = tail.find(')') else { break };
            let target = tail[..end].trim();
            if !target.starts_with("http")
                && !target.starts_with("mailto:")
                && !rest[..i].ends_with('!')
            {
                let (p, fragment) = target.split_once('#').unwrap_or((target, ""));
                if !p.is_empty() {
                    let dest = rel.parent().unwrap_or(Path::new(".")).join(p);
                    if let Ok(dest) = dest.canonicalize() {
                        if !fragment.is_empty()
                            && docs
                                .get(&dest)
                                .is_some_and(|s| !anchors(s).contains(&fragment.to_owned()))
                        {
                            errors.push(format!("{}: broken anchor in `{target}`", rel.display()));
                        }
                    } else {
                        errors.push(format!("{}: broken link `{target}`", rel.display()));
                    }
                }
            }
            rest = &tail[end + 1..];
        }
    }
    if errors.is_empty() {
        println!("ok: {} markdown files", docs.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("link-check failed:");
        for e in errors {
            eprintln!("  {e}");
        }
        ExitCode::FAILURE
    }
}
