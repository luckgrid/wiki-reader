---
id: WR-SPIKE-P2-S1
title: P2-S1 Mermaid-text spike
summary: Pin mermaid-text ≥ 0.56.1; flowchart + sequence render; licence OK for dependency.
status: done
updated: 2026-09-30
---

# P2-S1 — mermaid-text spike

Decision: **depend** on `mermaid-text = "0.56.1"` (not port). MIT/Apache dual via crates.io; API is `mermaid_text::render(&str) -> Result<String, Error>`.

Verified in-tree via `wiki_reader_render::diagrams` unit tests: flowchart (`graph LR`) and `sequenceDiagram` produce readable Unicode text.

Image tier (mermaid-rs-renderer → resvg → Kitty) remains unwired; ADR-0004 auto currently selects text unless Kitty graphics is probed later.
