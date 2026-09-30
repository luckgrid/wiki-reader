//! Link geometry shared by raw and rendered viewers (see rendering.md).

/// Stable id for hit map / Tab cycle within one document layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinkId(pub u32);

/// Resolved link styling class at layout time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkClass {
    /// Internal markdown target.
    Internal,
    /// External URL (`http` / `https` / `mailto`).
    External,
    /// Broken or non-markdown target.
    Broken,
    /// Non-openable URI scheme (`file:`, custom, …).
    Unsupported,
}

/// One link, possibly split across wrapped lines (rendered mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSpan {
    /// Hit / focus id.
    pub id: LinkId,
    /// Raw `href` from source.
    pub raw_target: String,
    /// Styling bucket.
    pub class: LinkClass,
    /// Visible segments: line index in the current view (0-based rendered or
    /// source line for raw), display columns `[start, end)`.
    pub segments: Vec<(u32, (u16, u16))>,
}
