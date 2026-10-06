//! P3-31 viewing-cost baseline: deterministic tempfile fixtures + ignored timings.
//!
//! Run (release, print table):
//! `cargo test -p wiki-reader --release -- --ignored --nocapture viewing_cost_baseline`

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use wiki_reader_core::Index;
use wiki_reader_core::nav::NodeId;
use wiki_reader_core::provider::PageKey;

use super::Action;
use super::App;
use super::draw::draw;

/// Fixture sizes for the ignored baseline run.
const FULL: Scale = Scale {
    groups: 50,
    pages_per_group: 100,
    table_rows: 10_000,
    code_lines: 50_000,
    token_bytes: 1_000_000,
    diagrams: 30,
    links: 500,
};

/// Tiny scale so CI compiles and proves the writer is hash-stable without writing 5k files.
const MINI: Scale = Scale {
    groups: 1,
    pages_per_group: 2,
    table_rows: 4,
    code_lines: 8,
    token_bytes: 32,
    diagrams: 1,
    links: 2,
};

#[derive(Clone, Copy)]
struct Scale {
    groups: usize,
    pages_per_group: usize,
    table_rows: usize,
    code_lines: usize,
    token_bytes: usize,
    diagrams: usize,
    links: usize,
}

fn write_collection(root: &Path, scale: Scale) {
    std::fs::create_dir_all(root).expect("root");
    std::fs::write(
        root.join("README.md"),
        "---\ntitle: Bench root\n---\n\n# Bench root\n\nSee `bench/`.\n",
    )
    .expect("readme");

    for g in 0..scale.groups {
        let dir = root.join(format!("bench/large-tree/d{g:02}"));
        std::fs::create_dir_all(&dir).expect("group");
        for p in 0..scale.pages_per_group {
            let path = dir.join(format!("p{p:02}.md"));
            let body =
                format!("---\ntitle: Page {g:02}-{p:02}\n---\n\n# Page {g:02}-{p:02}\n\nBody.\n");
            std::fs::write(path, body).expect("page");
        }
    }

    let mut table = String::from("# Big table\n\n| C0 | C1 | C2 |\n| --- | --- | --- |\n");
    for r in 0..scale.table_rows {
        let _ = writeln!(table, "| r{r} | cell-{r}-a | cell-{r}-b |");
    }
    std::fs::write(root.join("bench/big-table.md"), table).expect("table");

    let mut code = String::from("# Long code\n\n```\n");
    for i in 0..scale.code_lines {
        let _ = writeln!(code, "line {i:05}");
    }
    code.push_str("```\n");
    std::fs::write(root.join("bench/long-code.md"), code).expect("code");

    let mut token = String::from("# Huge token\n\n");
    token.push_str(&"a".repeat(scale.token_bytes));
    token.push('\n');
    std::fs::write(root.join("bench/huge-token.md"), token).expect("token");

    let mut diagrams = String::from("# Diagrams\n\n");
    for i in 0..scale.diagrams {
        let _ = write!(diagrams, "```mermaid\nflowchart LR\n  A{i}-->B{i}\n```\n\n");
    }
    std::fs::write(root.join("bench/diagrams.md"), diagrams).expect("diagrams");

    let mut links = String::from("# Link heavy\n\n");
    let mut n = 0usize;
    'outer: for g in 0..scale.groups {
        for p in 0..scale.pages_per_group {
            if n >= scale.links {
                break 'outer;
            }
            let _ = writeln!(links, "- [p{g:02}-{p:02}](large-tree/d{g:02}/p{p:02}.md)");
            n += 1;
        }
    }
    std::fs::write(root.join("bench/link-heavy.md"), links).expect("links");
}

fn fingerprint(root: &Path) -> u64 {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for ent in std::fs::read_dir(&dir).expect("read_dir") {
            let ent = ent.expect("entry");
            let path = ent.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(root).expect("rel").to_path_buf();
                let bytes = std::fs::read(&path).expect("read");
                files.insert(rel, bytes);
            }
        }
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (k, v) in &files {
        k.hash(&mut h);
        v.hash(&mut h);
    }
    h.finish()
}

fn expand_all_nav(app: &mut App) {
    fn walk(
        items: &[wiki_reader_core::nav::NavItem],
        expanded: &mut std::collections::HashSet<NodeId>,
    ) {
        for item in items {
            if let wiki_reader_core::nav::NavItem::Group { id, children, .. } = item {
                expanded.insert(id.clone());
                walk(children, expanded);
            }
        }
    }
    let tree_items = app.navigator.nav().tree.items.clone();
    let expanded = &mut app.navigator.nav_mut().expanded;
    walk(&tree_items, expanded);
}

fn go(app: &mut App, rel: &str) {
    let key = PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from(rel),
    };
    app.update(Action::GoToPage(key));
}

fn draw_once(app: &mut App, w: u16, h: u16) {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| draw(frame, app)).expect("draw");
}

fn median_ms(samples: &[Duration]) -> f64 {
    let mut v: Vec<f64> = samples.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    v.sort_by(|a, b| a.partial_cmp(b).expect("ord"));
    v[v.len() / 2]
}

fn time_draws(app: &mut App, w: u16, h: u16, n: usize) -> Duration {
    let start = Instant::now();
    for _ in 0..n {
        draw_once(app, w, h);
    }
    start.elapsed() / u32::try_from(n).unwrap_or(1)
}

fn rss_kb() -> u64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn modal_code(app: &mut App, code: KeyCode) {
    app.update(Action::ModalKey(KeyEvent::new(code, KeyModifiers::NONE)));
}

#[test]
fn bench_fixture_writer_is_deterministic() {
    let a = tempfile::tempdir().expect("a");
    let b = tempfile::tempdir().expect("b");
    write_collection(a.path(), MINI);
    write_collection(b.path(), MINI);
    assert_eq!(fingerprint(a.path()), fingerprint(b.path()));
}

/// Manual baseline: `cargo test -p wiki-reader --release -- --ignored --nocapture viewing_cost_baseline`
#[test]
#[ignore = "P3-31 baseline; run with --ignored --release --nocapture"]
#[allow(clippy::too_many_lines)] // one printable table beats six helper fns
fn viewing_cost_baseline() {
    let dir = tempfile::tempdir().expect("dir");
    write_collection(dir.path(), FULL);
    let dir2 = tempfile::tempdir().expect("dir2");
    write_collection(dir2.path(), FULL);
    assert_eq!(
        fingerprint(dir.path()),
        fingerprint(dir2.path()),
        "full fixture must be deterministic"
    );

    let mut rows: Vec<(&str, f64)> = Vec::new();

    let t0 = Instant::now();
    let mut app = App::for_tests(dir.path()).expect("app");
    let open_ms = t0.elapsed().as_secs_f64() * 1000.0;
    rows.push(("open_index_5k", open_ms));
    expand_all_nav(&mut app);
    let rss_after_open = rss_kb();

    draw_once(&mut app, 120, 40); // warm
    let mut samples = Vec::new();
    for _ in 0..5 {
        samples.push(time_draws(&mut app, 120, 40, 1));
    }
    rows.push(("frame_large_tree_expanded", median_ms(&samples)));

    for (label, rel) in [
        ("open_link_heavy", "bench/link-heavy.md"),
        ("open_huge_token", "bench/huge-token.md"),
        ("open_diagrams", "bench/diagrams.md"),
        ("open_long_code", "bench/long-code.md"),
        ("open_big_table", "bench/big-table.md"),
    ] {
        let mut s = Vec::new();
        for _ in 0..3 {
            let start = Instant::now();
            go(&mut app, rel);
            draw_once(&mut app, 120, 40);
            s.push(start.elapsed());
        }
        rows.push((label, median_ms(&s)));
    }

    go(&mut app, "bench/link-heavy.md");
    draw_once(&mut app, 120, 40);
    samples.clear();
    for _ in 0..5 {
        samples.push(time_draws(&mut app, 120, 40, 1));
    }
    rows.push(("frame_link_heavy", median_ms(&samples)));

    // Width relayout (rendered path reloads from disk).
    go(&mut app, "bench/diagrams.md");
    draw_once(&mut app, 120, 40);
    samples.clear();
    for (i, w) in [80u16, 90, 100, 80, 90].into_iter().enumerate() {
        let start = Instant::now();
        app.ensure_layout_width(w);
        draw_once(&mut app, 120 + u16::try_from(i).unwrap_or(0), 40);
        samples.push(start.elapsed());
    }
    rows.push(("width_relayout_diagrams", median_ms(&samples)));

    // Reindex UI thread: identical index (should be cheap) vs one-page change.
    go(&mut app, "bench/link-heavy.md");
    draw_once(&mut app, 120, 40);
    let identical = Index::build(&app.provider).expect("index");
    let view = app.view_state();
    let start = Instant::now();
    let effects = app.navigator.reindex(identical, view);
    app.apply_effects(effects);
    rows.push((
        "reindex_ui_identical",
        start.elapsed().as_secs_f64() * 1000.0,
    ));

    std::fs::write(
        dir.path().join("bench/link-heavy.md"),
        "# Link heavy\n\nchanged\n",
    )
    .expect("mutate");
    let changed = Index::build(&app.provider).expect("index2");
    let view = app.view_state();
    let start = Instant::now();
    let effects = app.navigator.reindex(changed, view);
    app.apply_effects(effects);
    rows.push(("reindex_ui_changed", start.elapsed().as_secs_f64() * 1000.0));

    // Table viewer G / PgDn / want.
    go(&mut app, "bench/big-table.md");
    draw_once(&mut app, 120, 40);
    app.open_table(0);
    assert!(app.modal.is_some(), "table modal");
    let start = Instant::now();
    if let Some(m) = app.modal.as_mut() {
        let _ = m.want((100, 30));
    }
    rows.push(("table_want", start.elapsed().as_secs_f64() * 1000.0));

    // keep_cursor_visible runs from draw, not from the key handler alone.
    samples.clear();
    for _ in 0..3 {
        modal_code(&mut app, KeyCode::Home);
        draw_once(&mut app, 120, 40);
        let start = Instant::now();
        modal_code(&mut app, KeyCode::Char('G'));
        draw_once(&mut app, 120, 40);
        samples.push(start.elapsed());
    }
    rows.push(("table_G_plus_draw", median_ms(&samples)));

    samples.clear();
    modal_code(&mut app, KeyCode::Home);
    draw_once(&mut app, 120, 40);
    for _ in 0..10 {
        let start = Instant::now();
        modal_code(&mut app, KeyCode::PageDown);
        draw_once(&mut app, 120, 40);
        samples.push(start.elapsed());
    }
    rows.push(("table_PgDn_plus_draw", median_ms(&samples)));

    eprintln!("\nP3-31 viewing_cost_baseline (median ms unless rss_kb_*)");
    eprintln!("fixture fingerprint {:016x}", fingerprint(dir.path()));
    for (name, v) in &rows {
        eprintln!("{name}\t{v:.3}");
    }
    eprintln!("rss_kb_after_5k_open\t{rss_after_open}");
    eprintln!("rss_kb_end\t{}", rss_kb());
}
