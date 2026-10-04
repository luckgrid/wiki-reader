//! Canonical frontmatter `status:` values for this wiki.
//!
//! User collections may use other vocabularies; [`DocStatus::parse`] returns
//! [`None`] for unknowns so the UI can fall back to the plain text colour.

/// Document lifecycle status from frontmatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocStatus {
    /// Being written, not reviewed.
    Draft,
    /// Written, awaiting a decision, or planned but not started.
    Proposed,
    /// Reviewed and settled; for ADRs, immutable.
    Accepted,
    /// Living doc, in progress now.
    Active,
    /// Finished work item / spike / phase.
    Done,
    /// Postponed.
    Deferred,
    /// Replaced by another doc.
    Superseded,
    /// Kept for the record, no longer maintained.
    Historical,
}

impl DocStatus {
    /// Parse a frontmatter status string (case-insensitive). Unknown → [`None`].
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw.trim().to_ascii_lowercase().as_str() {
            "draft" => Self::Draft,
            "proposed" => Self::Proposed,
            "accepted" => Self::Accepted,
            "active" => Self::Active,
            "done" => Self::Done,
            "deferred" => Self::Deferred,
            "superseded" => Self::Superseded,
            "historical" => Self::Historical,
            _ => return None,
        })
    }

    /// Canonical lowercase spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Active => "active",
            Self::Done => "done",
            Self::Deferred => "deferred",
            Self::Superseded => "superseded",
            Self::Historical => "historical",
        }
    }

    /// Every canonical value, in docs-table order.
    pub const ALL: [Self; 8] = [
        Self::Draft,
        Self::Proposed,
        Self::Accepted,
        Self::Active,
        Self::Done,
        Self::Deferred,
        Self::Superseded,
        Self::Historical,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_canonical_value_round_trips() {
        for s in DocStatus::ALL {
            assert_eq!(DocStatus::parse(s.as_str()), Some(s));
            assert_eq!(DocStatus::parse(&s.as_str().to_ascii_uppercase()), Some(s));
        }
    }

    #[test]
    fn unknown_and_legacy_aliases_are_none() {
        for raw in [
            "", "planned", "complete", "stable", "wip", "todo", "archived",
        ] {
            assert_eq!(DocStatus::parse(raw), None, "{raw}");
        }
    }
}
