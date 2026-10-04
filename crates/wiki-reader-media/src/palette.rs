//! Diagram colours: plain RGB, no heavy deps. The raster module adds the theme overlay.

/// Colours a Mermaid diagram is drawn with, so the picture matches the active theme.
///
/// Plain RGB so the renderer stays terminal-free; the TUI fills it from its theme tokens. It is
/// part of every diagram cache key, so changing preset re-measures and re-rasterises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagramPalette {
    /// Card background, also used to fill the slot's padding so the picture is opaque.
    pub bg: (u8, u8, u8),
    /// Labels and titles.
    pub text: (u8, u8, u8),
    /// Edges and axes.
    pub line: (u8, u8, u8),
    /// Node fill.
    pub node_fill: (u8, u8, u8),
    /// Node outline.
    pub node_border: (u8, u8, u8),
    /// Subgraph / cluster fill.
    pub cluster_fill: (u8, u8, u8),
    /// Subgraph / cluster outline.
    pub cluster_border: (u8, u8, u8),
    /// Sequence / state note fill (its text uses [`text`](Self::text)).
    pub note_fill: (u8, u8, u8),
    /// Sequence / state note outline.
    pub note_border: (u8, u8, u8),
}

impl Default for DiagramPalette {
    /// Dark card, matching the default (dark) theme's code background.
    fn default() -> Self {
        Self {
            bg: (30, 32, 36),
            text: (226, 229, 236),
            line: (140, 148, 164),
            node_fill: (44, 48, 58),
            node_border: (110, 120, 140),
            cluster_fill: (38, 41, 50),
            cluster_border: (80, 88, 104),
            note_fill: (58, 54, 40),
            note_border: (150, 130, 80),
        }
    }
}
