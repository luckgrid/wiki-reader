//! Options overlay (P3-13): edit typed config live and persist via ADR-0018.

use wiki_reader_core::config::{CopyPathMode, DiagramMode, LabelMode, NavPosition, ThemeName};

/// One editable setting row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionRow {
    Theme,
    NavPosition,
    NavLabels,
    Diagrams,
    ImagesEnabled,
    ImagesMaxSlotRows,
    CopyPath,
}

impl OptionRow {
    pub const ALL: [Self; 7] = [
        Self::Theme,
        Self::NavPosition,
        Self::NavLabels,
        Self::Diagrams,
        Self::ImagesEnabled,
        Self::ImagesMaxSlotRows,
        Self::CopyPath,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Theme => "Theme",
            Self::NavPosition => "Nav position",
            Self::NavLabels => "Nav labels",
            Self::Diagrams => "Diagrams",
            Self::ImagesEnabled => "Images",
            Self::ImagesMaxSlotRows => "Max image rows",
            Self::CopyPath => "Copy path",
        }
    }
}

/// Open options overlay.
#[derive(Debug, Clone)]
pub struct OptionsOverlay {
    pub selected: usize,
    pub scroll: usize,
    pub list_height: usize,
}

impl OptionsOverlay {
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: 0,
            scroll: 0,
            list_height: 10,
        }
    }

    #[must_use]
    pub fn current(&self) -> OptionRow {
        OptionRow::ALL[self.selected.min(OptionRow::ALL.len() - 1)]
    }

    pub fn select_delta(&mut self, delta: i32) {
        let n = i32::try_from(OptionRow::ALL.len()).unwrap_or(1);
        let cur = i32::try_from(self.selected).unwrap_or(0);
        let next = (cur + delta).rem_euclid(n);
        self.selected = usize::try_from(next).unwrap_or(0);
        self.ensure_visible();
    }

    fn ensure_visible(&mut self) {
        let h = self.list_height.max(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + h {
            self.scroll = self.selected + 1 - h;
        }
    }
}

impl Default for OptionsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub fn cycle_theme(cur: ThemeName, dir: i32) -> ThemeName {
    cycle(
        &[ThemeName::Dark, ThemeName::Light, ThemeName::Herdr],
        cur,
        dir,
    )
}

#[must_use]
pub fn cycle_nav_position(cur: NavPosition, dir: i32) -> NavPosition {
    cycle(&[NavPosition::Left, NavPosition::Right], cur, dir)
}

#[must_use]
pub fn cycle_nav_labels(cur: LabelMode, dir: i32) -> LabelMode {
    cycle(
        &[
            LabelMode::Title,
            LabelMode::Filename,
            LabelMode::TitleFilename,
        ],
        cur,
        dir,
    )
}

#[must_use]
pub fn cycle_diagrams(cur: DiagramMode, dir: i32) -> DiagramMode {
    cycle(
        &[
            DiagramMode::Auto,
            DiagramMode::Image,
            DiagramMode::Text,
            DiagramMode::Source,
        ],
        cur,
        dir,
    )
}

#[must_use]
pub fn cycle_copy_path(cur: CopyPathMode, dir: i32) -> CopyPathMode {
    cycle(&[CopyPathMode::Relative, CopyPathMode::Absolute], cur, dir)
}

#[must_use]
pub fn cycle_max_slot_rows(cur: u16, dir: i32) -> u16 {
    const STEPS: [u16; 6] = [10, 20, 30, 40, 50, 60];
    let idx = STEPS.iter().position(|&n| n >= cur).unwrap_or(2);
    let next = if dir >= 0 {
        (idx + 1) % STEPS.len()
    } else {
        (idx + STEPS.len() - 1) % STEPS.len()
    };
    STEPS[next]
}

fn cycle<T: Copy + PartialEq>(items: &[T], cur: T, dir: i32) -> T {
    let idx = items.iter().position(|x| *x == cur).unwrap_or(0);
    let n = items.len();
    let next = if dir >= 0 {
        (idx + 1) % n
    } else {
        (idx + n - 1) % n
    };
    items[next]
}

#[must_use]
pub fn theme_label(v: ThemeName) -> &'static str {
    v.as_str()
}

#[must_use]
pub fn nav_position_label(v: NavPosition) -> &'static str {
    v.as_str()
}

#[must_use]
pub fn nav_labels_label(v: LabelMode) -> &'static str {
    v.as_str()
}

#[must_use]
pub fn diagrams_label(v: DiagramMode) -> &'static str {
    v.as_str()
}

#[must_use]
pub fn copy_path_label(v: CopyPathMode) -> &'static str {
    v.as_str()
}
