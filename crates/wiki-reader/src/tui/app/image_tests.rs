//! Image slots end to end through the real draw path. A halfblocks picker needs no terminal
//! query and paints `▀` cells, so `TestBackend` can see where pictures land.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui_image::picker::{Picker, ProtocolType};

use wiki_reader_render::SlotSource;

use super::App;
use super::draw::draw;
use crate::tui::images::ImageManager;

/// Halfblocks paint `▀` or `▄` depending on which half of the cell is opaque.
const HALVES: [&str; 2] = ["▀", "▄"];

fn slot_ends_with(slot: &wiki_reader_render::ImageSlot, name: &str) -> bool {
    matches!(&slot.source, SlotSource::File { path, .. } if path.ends_with(name))
}

fn images_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/images")
}

fn graphics_app(root: &Path) -> App {
    let mut app = App::for_tests(root).expect("app");
    app.enable_graphics(Picker::halfblocks());
    app
}

fn draw_to(terminal: &mut Terminal<TestBackend>, app: &mut App) {
    terminal.draw(|frame| draw(frame, app)).expect("draw");
}

fn terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(width, height)).expect("terminal")
}

/// Block until the decode worker has finished everything queued.
fn wait_for_images(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.images.has_pending() {
        assert!(Instant::now() < deadline, "decode worker timed out");
        app.images.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

fn has_half(buf: &Buffer, y: u16) -> bool {
    (0..buf.area.width).any(|x| HALVES.contains(&buf[(x, y)].symbol()))
}

fn any_text(terminal: &Terminal<TestBackend>, needle: &str) -> bool {
    let buf = terminal.backend().buffer();
    (0..buf.area.height).any(|y| row_text(buf, y).contains(needle))
}

#[test]
fn without_graphics_the_page_shows_text_placeholders() {
    let mut app = App::for_tests(&images_fixture()).expect("app");
    let mut term = terminal(100, 40);
    draw_to(&mut term, &mut app);
    assert!(app.doc.image_slots().is_empty());
    assert!(any_text(
        &term,
        "[image: Small grid] img/small.png — no graphics protocol"
    ));
    let buf = term.backend().buffer();
    assert!((0..buf.area.height).all(|y| !has_half(buf, y)));
}

#[test]
fn slot_shows_its_placeholder_until_the_decode_lands() {
    let mut app = graphics_app(&images_fixture());
    let mut term = terminal(100, 40);
    draw_to(&mut term, &mut app);
    assert!(!app.doc.image_slots().is_empty(), "graphics reserve rows");
    assert!(app.images.has_pending());
    assert!(any_text(&term, "[image: Small grid]"));

    wait_for_images(&mut app);
    draw_to(&mut term, &mut app);
    assert!(
        !any_text(&term, "[image: Small grid]"),
        "picture replaces it"
    );
}

#[test]
fn ready_picture_paints_exactly_the_reserved_rows() {
    let mut app = graphics_app(&images_fixture());
    let mut term = terminal(100, 60);
    draw_to(&mut term, &mut app);
    wait_for_images(&mut app);
    draw_to(&mut term, &mut app);

    let geom = app.viewer_geom;
    let slot = app.doc.image_slots()[0].clone();
    let buf = term.backend().buffer();
    let first = geom.top_y + u16::try_from(slot.line).expect("line");
    for dy in 0..slot.rows {
        assert!(has_half(buf, first + dy), "slot row {dy} has pixels");
    }
    assert!(!has_half(buf, first + slot.rows), "the gap row stays blank");
    // Pictures start at the text column, not under the cursor marker.
    let x = (0..buf.area.width)
        .find(|&x| HALVES.contains(&buf[(x, first)].symbol()))
        .expect("a pixel");
    assert_eq!(x, geom.text_x);
}

#[test]
fn slot_scrolled_off_the_top_is_cropped_to_its_visible_rows() {
    let mut app = graphics_app(&images_fixture());
    let mut term = terminal(100, 60);
    draw_to(&mut term, &mut app);
    wait_for_images(&mut app);
    let slot = app.doc.image_slots()[0].clone();
    // Hide the slot's first two rows: rows 2 and 3 remain at the top of the viewer.
    app.scroll = slot.line + 2;
    app.cursor_line = app.scroll;
    draw_to(&mut term, &mut app);

    let top = app.viewer_geom.top_y;
    let buf = term.backend().buffer();
    assert!(has_half(buf, top) && has_half(buf, top + 1));
    assert!(
        !has_half(buf, top + 2),
        "only two rows of the slot are visible"
    );
}

#[test]
fn slot_below_the_fold_paints_nothing_until_scrolled_into_view() {
    let mut app = graphics_app(&images_fixture());
    // 20 rows: the tall slot sits far below the first screen.
    let mut term = terminal(100, 20);
    draw_to(&mut term, &mut app);
    wait_for_images(&mut app);
    draw_to(&mut term, &mut app);
    let tall = app
        .doc
        .image_slots()
        .iter()
        .find(|s| slot_ends_with(s, "tall.png"))
        .expect("tall slot")
        .clone();
    let geom = app.viewer_geom;
    assert!(
        tall.line > u32::from(geom.rows),
        "tall slot starts offscreen"
    );

    app.scroll = tall.line;
    app.cursor_line = tall.line;
    draw_to(&mut term, &mut app);
    assert!(
        app.images.has_pending(),
        "an offscreen picture is queued only when it becomes visible"
    );
    assert!(
        !has_half(term.backend().buffer(), geom.top_y),
        "placeholder remains while the newly visible picture decodes"
    );
    wait_for_images(&mut app);
    draw_to(&mut term, &mut app);
    let buf = term.backend().buffer();
    assert!(has_half(buf, geom.top_y), "visible once decoded");
}

#[test]
fn corrupt_file_keeps_the_placeholder_and_says_why() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("README.md"),
        "# Page\n\n![Broken](bad.png)\n",
    )
    .expect("page");
    // A real 8×8 PNG with its pixel data corrupted: the header still reads, the decode fails.
    let path = dir.path().join("bad.png");
    image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255]))
        .save(&path)
        .expect("png");
    let mut png = std::fs::read(&path).expect("read");
    let idat = png
        .windows(4)
        .position(|w| w == b"IDAT")
        .expect("IDAT chunk");
    for byte in &mut png[idat + 4..idat + 8] {
        *byte ^= 0xFF;
    }
    std::fs::write(&path, png).expect("corrupt");

    let mut app = graphics_app(dir.path());
    let mut term = terminal(80, 20);
    draw_to(&mut term, &mut app);
    assert_eq!(
        app.doc.image_slots().len(),
        1,
        "header is readable, so a slot is reserved"
    );
    wait_for_images(&mut app);
    draw_to(&mut term, &mut app);
    assert!(
        any_text(&term, "[image: Broken] — "),
        "failure reason shown"
    );
    let buf = term.backend().buffer();
    assert!((0..buf.area.height).all(|y| !has_half(buf, y)));
}

#[test]
fn leaving_the_page_releases_its_pictures() {
    let mut app = graphics_app(&images_fixture());
    let mut term = terminal(100, 60);
    draw_to(&mut term, &mut app);
    wait_for_images(&mut app);
    app.images.retain_for(&[]);
    assert!(!app.images.has_pending());
    // A fresh draw queues them again.
    draw_to(&mut term, &mut app);
    assert!(app.images.has_pending());
}

#[test]
fn disabled_manager_has_no_cell_size() {
    assert!(ImageManager::disabled().cell_px().is_none());
    assert_eq!(
        ImageManager::enabled_with_sizes(
            Picker::halfblocks(),
            Arc::new(wiki_reader_render::DiagramSizeCache::new()),
        )
        .cell_px(),
        Some((10, 20))
    );
}

#[test]
fn popup_clear_covers_a_picture_and_it_returns_when_the_popup_closes() {
    use crate::tui::action::Action;
    let mut app = graphics_app(&images_fixture());
    let mut term = terminal(100, 30);
    draw_to(&mut term, &mut app);
    wait_for_images(&mut app);
    let slot = app.doc.image_slots()[0].clone();
    // Put the slot a few rows below the top of the viewer: under a centred popup.
    app.scroll = slot.line.saturating_sub(8);
    app.cursor_line = app.scroll;
    draw_to(&mut term, &mut app);
    let first = app.viewer_geom.top_y + 8;
    let slot_rows = first..first + slot.rows;
    let painted = |term: &Terminal<TestBackend>| {
        let buf = term.backend().buffer();
        slot_rows.clone().filter(|&y| has_half(buf, y)).count()
    };
    assert_eq!(
        painted(&term),
        usize::from(slot.rows),
        "visible before the popup"
    );

    app.update(Action::OpenHelp);
    draw_to(&mut term, &mut app);
    assert_eq!(painted(&term), 0, "the popup's Clear covers the picture");

    app.update(Action::Back);
    app.help = None;
    draw_to(&mut term, &mut app);
    assert_eq!(
        painted(&term),
        usize::from(slot.rows),
        "picture returns after the popup"
    );
}

#[test]
fn scrolling_back_shows_the_cached_picture_without_a_new_decode() {
    let mut app = graphics_app(&images_fixture());
    let mut term = terminal(100, 20);
    draw_to(&mut term, &mut app);
    let first = app.doc.image_slots()[0].clone();
    let tall = app
        .doc
        .image_slots()
        .iter()
        .find(|s| slot_ends_with(s, "tall.png"))
        .expect("tall slot")
        .clone();
    let near_first = first.line.saturating_sub(2);
    app.scroll = near_first;
    app.cursor_line = near_first;
    draw_to(&mut term, &mut app);
    wait_for_images(&mut app);

    // Scroll far away so the first picture is off screen, and let the new one decode.
    app.scroll = tall.line;
    app.cursor_line = tall.line;
    draw_to(&mut term, &mut app);
    wait_for_images(&mut app);

    // Back again: painted on the very first draw, nothing queued.
    app.scroll = near_first;
    app.cursor_line = near_first;
    draw_to(&mut term, &mut app);
    assert!(!app.images.has_pending(), "no second decode");
    let y = app.viewer_geom.top_y + 2;
    assert!(
        has_half(term.backend().buffer(), y),
        "cached picture, no placeholder flash"
    );
}

#[test]
fn decode_failures_are_not_retried_when_scrolling_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let filler = "filler line\n\n".repeat(40);
    std::fs::write(
        dir.path().join("README.md"),
        format!("# Page\n\n![Broken](bad.png)\n\n{filler}"),
    )
    .expect("page");
    let path = dir.path().join("bad.png");
    image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255]))
        .save(&path)
        .expect("png");
    let mut png = std::fs::read(&path).expect("read");
    let idat = png
        .windows(4)
        .position(|w| w == b"IDAT")
        .expect("IDAT chunk");
    for byte in &mut png[idat + 4..idat + 8] {
        *byte ^= 0xFF;
    }
    std::fs::write(&path, png).expect("corrupt");

    let mut app = graphics_app(dir.path());
    let mut term = terminal(80, 16);
    draw_to(&mut term, &mut app);
    wait_for_images(&mut app);
    draw_to(&mut term, &mut app);
    assert!(any_text(&term, "[image: Broken] — "));

    app.scroll = 40;
    app.cursor_line = 40;
    draw_to(&mut term, &mut app);
    assert!(!any_text(&term, "[image: Broken]"), "off screen");

    app.scroll = 0;
    app.cursor_line = 0;
    draw_to(&mut term, &mut app);
    assert!(
        !app.images.has_pending(),
        "the failure is remembered, not retried"
    );
    assert!(
        any_text(&term, "[image: Broken] — "),
        "reason shown at once"
    );
}

fn mermaid_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/mermaid")
}

#[test]
fn mermaid_diagram_swaps_from_text_to_halfblocks_slot() {
    use crate::tui::action::Action;

    let mut app = graphics_app(&mermaid_fixture());
    // Open the compact common-types page so a small flowchart can become a slot.
    let key = wiki_reader_core::provider::PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("common-types.md"),
    };
    app.load_page(&key);
    let mut term = terminal(120, 40);
    draw_to(&mut term, &mut app);
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        app.images.poll();
        if app.images.take_diagram_relayout() {
            app.relayout_after_diagram_size();
        }
        draw_to(&mut term, &mut app);
        if !app.doc.image_slots().is_empty() && !app.images.has_pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !app.doc.image_slots().is_empty(),
        "warm size cache should produce Mermaid slots"
    );
    wait_for_images(&mut app);
    draw_to(&mut term, &mut app);
    let buf = term.backend().buffer();
    assert!(
        (0..buf.area.height).any(|y| has_half(buf, y)),
        "halfblocks paint the Mermaid slot"
    );

    let slot = app.doc.image_slots()[0].clone();
    app.scroll = slot.line.saturating_sub(2);
    app.cursor_line = app.scroll;
    draw_to(&mut term, &mut app);
    wait_for_images(&mut app);
    draw_to(&mut term, &mut app);
    assert!((0..term.backend().buffer().area.height).any(|y| has_half(term.backend().buffer(), y)));

    app.update(Action::OpenHelp);
    draw_to(&mut term, &mut app);
    app.help = None;
    draw_to(&mut term, &mut app);
    wait_for_images(&mut app);
    draw_to(&mut term, &mut app);
    assert!(
        (0..term.backend().buffer().area.height).any(|y| has_half(term.backend().buffer(), y)),
        "diagram returns after the popup closes"
    );
}

/// Drive the same measure / re-layout / decode cycle as the event loop, without input.
fn settle_diagrams(term: &mut Terminal<TestBackend>, app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        app.images.poll();
        if app.images.take_diagram_relayout() {
            app.relayout_after_diagram_size();
        }
        draw_to(term, app);
        if !app.images.has_pending() {
            return;
        }
        assert!(Instant::now() < deadline, "diagram worker timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn theme_switch_requeues_visible_mermaid_slots() {
    use crate::tui::action::Action;
    use crate::tui::options_ui::OptionChoice;
    use wiki_reader_core::config::ThemeName;

    let mut app = graphics_app(&mermaid_fixture());
    let key = wiki_reader_core::provider::PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("themed.md"),
    };
    app.update(Action::GoToPage(key));
    // Keep all three cards visible, including Pie and Git graph below Sequence.
    let mut term = terminal(140, 80);
    settle_diagrams(&mut term, &mut app);
    assert_eq!(app.doc.image_slots().len(), 3);

    // Measurements may finish while options input is being handled, before the next
    // frame consumes their re-layout notification. Switching back must not lose them.
    app.update(Action::OpenOptions);
    app.options_set(OptionChoice::Theme(ThemeName::Light));
    wait_for_images(&mut app);
    app.options_set(OptionChoice::Theme(ThemeName::Dark));

    for theme in [ThemeName::Light, ThemeName::Dark, ThemeName::Herdr] {
        app.update(Action::OpenOptions);
        app.options_set(OptionChoice::Theme(theme));
        app.close_options();
        draw_to(&mut term, &mut app);
        assert!(app.images.has_pending(), "new palette queues work");
        settle_diagrams(&mut term, &mut app);
        assert_eq!(app.doc.image_slots().len(), 3, "slots after {theme:?}");
        for slot in app.doc.image_slots() {
            assert!(
                matches!(&slot.source, SlotSource::Mermaid { palette, .. } if *palette == app.theme.diagram)
            );
            let row = app.viewer_geom.top_y + u16::try_from(slot.line).expect("slot line");
            assert!(
                has_half(term.backend().buffer(), row),
                "{} paints after {theme:?}",
                slot.alt
            );
        }
    }
}

#[test]
fn diagram_relayout_preserves_text_selection() {
    use crate::tui::selection::{Pos, Selection};

    let mut app = graphics_app(&mermaid_fixture());
    let key = wiki_reader_core::provider::PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("common-types.md"),
    };
    app.load_page(&key);
    let sel = Selection {
        anchor: Pos { line: 1, col: 0 },
        head: Pos { line: 2, col: 5 },
    };
    app.selection = Some(sel);
    app.selecting = true;
    app.relayout_after_diagram_size();
    assert_eq!(app.selection, Some(sel));
    assert!(
        app.selecting,
        "drag must survive a measure-driven re-layout"
    );
}

fn modal_press(app: &mut App, code: ratatui::crossterm::event::KeyCode) {
    use crate::tui::action::Action;
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};
    app.update(Action::ModalKey(KeyEvent::new(code, KeyModifiers::NONE)));
}

/// Draw until the open modal has nothing pending.
fn settle_modal(term: &mut Terminal<TestBackend>, app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        draw_to(term, app);
        if !app.modal.as_ref().expect("modal").busy() {
            return;
        }
        assert!(Instant::now() < deadline, "modal worker timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn enter_on_a_picture_opens_it_zooms_within_caps_and_esc_closes() {
    use crate::tui::action::Action;
    use ratatui::crossterm::event::KeyCode;

    let mut app = graphics_app(&images_fixture());
    let mut term = terminal(100, 40);
    draw_to(&mut term, &mut app);
    let slot = app.doc.image_slots()[0].clone();
    app.update(Action::FocusViewer);
    app.focused_item = None;
    app.cursor_line = slot.line + 1;
    app.update(Action::ViewerActivate);
    assert!(app.modal.is_some(), "Enter inside a slot opens the viewer");
    assert_eq!(app.input_mode, crate::tui::keymap::InputMode::Modal);
    settle_modal(&mut term, &mut app);
    let buf = term.backend().buffer();
    assert!(
        (0..buf.area.height).any(|y| has_half(buf, y)),
        "picture in the modal"
    );
    assert!(any_text(&term, "100%") && any_text(&term, "IMAGE"));

    // Zoom clamps at both ends; the picture is still within the pixel cap at the top step.
    for _ in 0..20 {
        modal_press(&mut app, KeyCode::Char('+'));
    }
    settle_modal(&mut term, &mut app);
    assert!(any_text(&term, "800%"));
    for _ in 0..20 {
        modal_press(&mut app, KeyCode::Char('-'));
    }
    settle_modal(&mut term, &mut app);
    assert!(any_text(&term, "100%"));
    // Panning never panics at the edges.
    modal_press(&mut app, KeyCode::Right);
    modal_press(&mut app, KeyCode::Down);
    settle_modal(&mut term, &mut app);

    // Carousel (P3-21): every block image of the page, opened one first; sizing is per item.
    assert!(any_text(&term, "1/9") && any_text(&term, "fit 100%"));
    modal_press(&mut app, KeyCode::Char('a'));
    settle_modal(&mut term, &mut app);
    assert!(any_text(&term, "actual 100%"), "a toggles actual size");
    modal_press(&mut app, KeyCode::Tab);
    settle_modal(&mut term, &mut app);
    assert!(any_text(&term, "2/9") && any_text(&term, "fit 100%"));
    // The remote image is in the inventory too, as a note instead of a picture.
    for _ in 0..4 {
        modal_press(&mut app, KeyCode::Tab);
    }
    draw_to(&mut term, &mut app);
    assert!(any_text(&term, "6/9") && any_text(&term, "logo.png: no preview"));
    modal_press(&mut app, KeyCode::BackTab);
    settle_modal(&mut term, &mut app);
    assert!(any_text(&term, "5/9"));

    modal_press(&mut app, KeyCode::Esc);
    assert!(app.modal.is_none(), "Esc closes and drops the viewer");
    assert_eq!(app.input_mode, crate::tui::keymap::InputMode::Normal);
}

#[test]
fn text_tier_expand_diagram_shows_the_source() {
    use crate::tui::action::Action;
    use ratatui::crossterm::event::KeyCode;

    let mut app = App::for_tests(&mermaid_fixture()).expect("app");
    let key = wiki_reader_core::provider::PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("common-types.md"),
    };
    app.load_page(&key);
    let mut term = terminal(120, 40);
    draw_to(&mut term, &mut app);
    let crate::tui::page_doc::PageDoc::Rendered(doc) = &app.doc else {
        panic!("rendered page");
    };
    let line = doc
        .block_actions()
        .iter()
        .find(|a| a.kind == wiki_reader_render::BlockActionKind::ExpandDiagram)
        .expect("a diagram action")
        .line;
    app.update(Action::FocusViewer);
    app.focused_item = None;
    app.cursor_line = line;
    app.update(Action::ViewerActivate);
    assert!(app.modal.is_some());
    draw_to(&mut term, &mut app);
    assert!(any_text(&term, "source view") && any_text(&term, "DIAGRAM"));
    assert!(any_text(&term, "-->"), "the Mermaid source is listed");
    modal_press(&mut app, KeyCode::Esc);
    assert!(app.modal.is_none());
}

#[test]
fn failed_mermaid_modal_stays_short() {
    use crate::tui::action::Action;
    use ratatui::crossterm::event::KeyCode;

    let mut app = graphics_app(&mermaid_fixture());
    let key = wiki_reader_core::provider::PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("malformed.md"),
    };
    app.update(Action::GoToPage(key));
    let mut term = terminal(120, 40);
    draw_to(&mut term, &mut app);
    let crate::tui::page_doc::PageDoc::Rendered(doc) = &app.doc else {
        panic!("rendered page");
    };
    let line = doc
        .block_actions()
        .iter()
        .find(|a| a.kind == wiki_reader_render::BlockActionKind::ExpandDiagram)
        .expect("a diagram action")
        .line;
    app.update(Action::FocusViewer);
    app.focused_item = None;
    app.cursor_line = line;
    app.update(Action::ViewerActivate);
    assert!(app.modal.is_some());
    settle_modal(&mut term, &mut app);
    assert!(any_text(&term, "cannot render:"), "error note");
    assert!(any_text(&term, "not a diagram"), "source listed");
    // Content-sized: the panel is a few rows, not the full 40-row terminal.
    let want = app.modal.as_mut().unwrap().want((118, 37));
    assert!(
        want.1 <= 6,
        "failed diagram wants a short panel, got {want:?}"
    );
    modal_press(&mut app, KeyCode::Esc);
    assert!(app.modal.is_none());
}

/// The Kitty upload rides in the picture's first cell on its first render, and
/// `ratatui-image` never sends it again. A picture first drawn while a popup covers that cell
/// must therefore wait until the cell is visible, or it stays blank after the popup closes.
#[test]
fn picture_first_drawn_under_a_popup_is_uploaded_once_the_popup_closes() {
    use crate::tui::action::Action;

    const KITTY_APC: &str = "\u{1b}_G";
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    let mut app = App::for_tests(&images_fixture()).expect("app");
    app.enable_graphics(picker);
    let mut term = terminal(40, 30);
    draw_to(&mut term, &mut app);
    assert!(
        !app.doc.image_slots().is_empty(),
        "the fixture has pictures"
    );

    // The popup opens while the pictures are still decoding.
    app.update(Action::OpenHelp);
    wait_for_images(&mut app);
    draw_to(&mut term, &mut app);
    let carrier = |app: &App| {
        let slot = &app.doc.image_slots()[0];
        let (rect, _) = crate::tui::images::slot_visible(
            slot,
            app.scroll,
            app.viewer_geom.text_x,
            app.viewer_geom.top_y,
            app.viewer_geom.rows,
        )
        .expect("the first picture is on screen");
        (rect.x, rect.y)
    };
    let covered = carrier(&app);
    assert!(
        !term.backend().buffer()[covered]
            .symbol()
            .contains('\u{10EEEE}'),
        "precondition: the help popup covers the first picture's first cell"
    );

    app.update(Action::CloseHelp);
    draw_to(&mut term, &mut app);
    let buf = term.backend().buffer();
    let first_cell = carrier(&app);
    assert!(
        buf[first_cell].symbol().contains(KITTY_APC),
        "the upload must reach the frame once nothing covers the first cell, got {:?}",
        buf[first_cell].symbol()
    );
}

/// A theme switch re-renders every diagram while the options window is open over the page.
/// Each diagram that ends up on screen must have had its Kitty upload land in a visible first
/// cell, or it stays blank until the page is left or scrolled.
#[test]
fn diagrams_switched_under_the_options_window_are_all_uploaded() {
    use crate::tui::action::Action;
    use crate::tui::options_ui::OptionChoice;
    use std::collections::HashSet;
    use wiki_reader_core::config::ThemeName;

    const KITTY_APC: &str = "\u{1b}_G";
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    let mut app = App::for_tests(&mermaid_fixture()).expect("app");
    app.enable_graphics(picker);
    let key = wiki_reader_core::provider::PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("themed.md"),
    };
    app.update(Action::GoToPage(key));
    let mut term = terminal(80, 120);

    let uploads = |term: &Terminal<TestBackend>| -> HashSet<(u16, u16)> {
        let buf = term.backend().buffer();
        let mut found = HashSet::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if buf[(x, y)].symbol().contains(KITTY_APC) {
                    found.insert((x, y));
                }
            }
        }
        found
    };
    settle_diagrams(&mut term, &mut app);
    assert_eq!(app.doc.image_slots().len(), 3);

    // Switch while the window covers the middle of the page, and let every frame upload.
    app.update(Action::OpenOptions);
    app.options_set(OptionChoice::Theme(ThemeName::Light));
    let mut uploaded = HashSet::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        app.images.poll();
        if app.images.take_diagram_relayout() {
            app.relayout_after_diagram_size();
        }
        draw_to(&mut term, &mut app);
        uploaded.extend(uploads(&term));
        if !app.images.has_pending() {
            break;
        }
        assert!(Instant::now() < deadline, "diagram worker timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    app.close_options();
    draw_to(&mut term, &mut app);
    uploaded.extend(uploads(&term));

    assert_eq!(app.doc.image_slots().len(), 3);
    for slot in app.doc.image_slots() {
        let (rect, _) = crate::tui::images::slot_visible(
            slot,
            app.scroll,
            app.viewer_geom.text_x,
            app.viewer_geom.top_y,
            app.viewer_geom.rows,
        )
        .unwrap_or_else(|| panic!("{} is on screen", slot.alt));
        assert!(
            uploaded.contains(&(rect.x, rect.y)),
            "{} was never uploaded at its first cell {:?}",
            slot.alt,
            (rect.x, rect.y)
        );
    }
}
