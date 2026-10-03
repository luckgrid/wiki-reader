//! Options overlay (P3-13): grouped radio choices that edit typed config live and persist
//! via ADR-0018.

use wiki_reader_core::config::{CopyPathMode, DiagramMode, LabelMode, NavPosition, ThemeName};

/// Image slot heights offered in the "Max image rows" group (config accepts 1–60).
pub const MAX_ROW_STEPS: [u16; 6] = [10, 20, 30, 40, 50, 60];

/// One selectable row. Radio groups pick one value; `ImagesEnabled` is an on/off toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionChoice {
    Theme(ThemeName),
    NavPosition(NavPosition),
    NavLabels(LabelMode),
    Diagrams(DiagramMode),
    ImagesEnabled,
    ImagesMaxRows(u16),
    CopyPath(CopyPathMode),
}

impl OptionChoice {
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Theme(ThemeName::Dark) => "Dark (luckgrid)".into(),
            Self::Theme(ThemeName::Light) => "Light (luckgrid)".into(),
            Self::Theme(ThemeName::Herdr) => "Herdr (follows herdr)".into(),
            Self::NavPosition(NavPosition::Left) => "Nav left".into(),
            Self::NavPosition(NavPosition::Right) => "Nav right".into(),
            Self::NavLabels(LabelMode::Title) => "Page titles".into(),
            Self::NavLabels(LabelMode::Filename) => "File names".into(),
            Self::NavLabels(LabelMode::TitleFilename) => "Titles and file names".into(),
            Self::Diagrams(DiagramMode::Auto) => "Auto".into(),
            Self::Diagrams(DiagramMode::Image) => "Image only".into(),
            Self::Diagrams(DiagramMode::Text) => "Text only".into(),
            Self::Diagrams(DiagramMode::Source) => "Source only".into(),
            Self::ImagesEnabled => "Show images".into(),
            Self::ImagesMaxRows(n) => format!("{n} rows"),
            Self::CopyPath(CopyPathMode::Relative) => "Relative to the collection".into(),
            Self::CopyPath(CopyPathMode::Absolute) => "Absolute path".into(),
        }
    }
}

/// One line of the overlay body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionLine {
    Heading(&'static str),
    Blank,
    Choice(OptionChoice),
}

/// The overlay body, top to bottom: a heading, its choices, then a blank row before the next group.
#[must_use]
pub fn lines() -> Vec<OptionLine> {
    let mut out = Vec::new();
    let mut group = |heading: &'static str, choices: Vec<OptionChoice>| {
        if !out.is_empty() {
            out.push(OptionLine::Blank);
        }
        out.push(OptionLine::Heading(heading));
        out.extend(choices.into_iter().map(OptionLine::Choice));
    };
    group(
        "Theme",
        [ThemeName::Dark, ThemeName::Light, ThemeName::Herdr]
            .map(OptionChoice::Theme)
            .into(),
    );
    group(
        "Panels",
        [NavPosition::Left, NavPosition::Right]
            .map(OptionChoice::NavPosition)
            .into(),
    );
    group(
        "Nav labels",
        [
            LabelMode::Title,
            LabelMode::Filename,
            LabelMode::TitleFilename,
        ]
        .map(OptionChoice::NavLabels)
        .into(),
    );
    group(
        "Mermaid",
        [
            DiagramMode::Auto,
            DiagramMode::Image,
            DiagramMode::Text,
            DiagramMode::Source,
        ]
        .map(OptionChoice::Diagrams)
        .into(),
    );
    group("Images", vec![OptionChoice::ImagesEnabled]);
    group(
        "Max image rows",
        MAX_ROW_STEPS.map(OptionChoice::ImagesMaxRows).into(),
    );
    group(
        "Copy path (y)",
        [CopyPathMode::Relative, CopyPathMode::Absolute]
            .map(OptionChoice::CopyPath)
            .into(),
    );
    out
}

/// Every selectable row, in display order.
#[must_use]
pub fn choices() -> Vec<OptionChoice> {
    lines()
        .into_iter()
        .filter_map(|l| match l {
            OptionLine::Choice(c) => Some(c),
            _ => None,
        })
        .collect()
}

/// Open options overlay. `selected` indexes [`choices`]; `scroll` is a line offset into [`lines`].
#[derive(Debug, Clone)]
pub struct OptionsOverlay {
    pub selected: usize,
    pub scroll: usize,
    pub list_height: usize,
}

impl OptionsOverlay {
    /// Starts on the row for the current theme, so the cursor opens on a live value.
    #[must_use]
    pub fn new(theme: ThemeName) -> Self {
        let selected = choices()
            .iter()
            .position(|c| *c == OptionChoice::Theme(theme))
            .unwrap_or(0);
        Self {
            selected,
            scroll: 0,
            list_height: 10,
        }
    }

    #[must_use]
    pub fn current(&self) -> OptionChoice {
        let all = choices();
        all[self.selected.min(all.len() - 1)]
    }

    pub fn select_delta(&mut self, delta: i32) {
        let n = i32::try_from(choices().len()).unwrap_or(1);
        let cur = i32::try_from(self.selected).unwrap_or(0);
        self.selected = usize::try_from((cur + delta).rem_euclid(n)).unwrap_or(0);
    }

    /// `(top, row)` line indexes of the selected row: `top` is pulled up to its group heading so
    /// scrolling never leaves a group without its title.
    #[must_use]
    pub fn selected_line(&self) -> (usize, usize) {
        let all = lines();
        let mut seen = 0;
        for (i, l) in all.iter().enumerate() {
            if matches!(l, OptionLine::Choice(_)) {
                if seen == self.selected {
                    let top = if i > 0 && matches!(all[i - 1], OptionLine::Heading(_)) {
                        i - 1
                    } else {
                        i
                    };
                    return (top, i);
                }
                seen += 1;
            }
        }
        (0, 0)
    }
}

impl Default for OptionsOverlay {
    fn default() -> Self {
        Self::new(ThemeName::Dark)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_cover_every_value_once() {
        let all = choices();
        assert_eq!(all.len(), 3 + 2 + 3 + 4 + 1 + MAX_ROW_STEPS.len() + 2);
        for (i, a) in all.iter().enumerate() {
            assert!(!all[i + 1..].contains(a), "duplicate {a:?}");
        }
    }

    #[test]
    fn selection_wraps_and_opens_on_the_current_theme() {
        let mut o = OptionsOverlay::new(ThemeName::Light);
        assert_eq!(o.current(), OptionChoice::Theme(ThemeName::Light));
        o.select_delta(-2);
        assert_eq!(o.current(), *choices().last().unwrap());
        o.select_delta(1);
        assert_eq!(o.current(), choices()[0]);
    }

    #[test]
    fn first_choice_of_a_group_keeps_its_heading_in_view() {
        let mut o = OptionsOverlay::new(ThemeName::Dark);
        o.selected = 3; // first "Panels" row
        let (top, row) = o.selected_line();
        assert_eq!(top + 1, row);
        assert!(matches!(lines()[top], OptionLine::Heading("Panels")));
    }
}
