---
id: WR-SPIKE-P3-S1
title: P3-S1 terminal image protocol spike
summary: Image protocols and Mermaid rendering are viable with an allowlisted terminal probe, explicit iTerm2 selection, and per-block text fallback.
status: done
updated: 2026-10-02
related: [phase-3-alpha, 0004-diagram-rendering]
---

# P3-S1 — terminal image protocol spike

## Spike program

Run the temporary examples from the repository root:

```bash
cargo run -p wiki-reader-tui --example image-protocol
cargo run -p wiki-reader-tui --example image-protocol -- --force-kitty
WIKI_READER_IMAGE_QUERY_TIMEOUT_MS=500 cargo run -p wiki-reader-tui --example image-protocol
cargo run -p wiki-reader-tui --example mermaid-image -- fixtures/elements/README.md
cargo run -p wiki-reader-tui --example mermaid-image -- fixtures/mermaid/common-types.md wiki/architecture/rendering.md
```

`image-protocol` enters the alternate screen with `ratatui::try_init`, immediately runs the `ratatui-image` stdio query, then enables mouse capture and Kitty keyboard disambiguation. It reports the selected protocol, cell size, capabilities, relevant environment variables and query duration before drawing `wiki/assets/wiki-reader.png`. `j`/`k` vary a simulated top clip, `o` overlays `Clear` plus a popup, and `n` replaces the image.

`mermaid-image` extracts Mermaid fences from Markdown (or reads a `.mmd` as one diagram), produces SVG with `mermaid-rs-renderer`, rasterises it with `resvg` and an embedded OFL Noto Sans font, and displays each successful block through the same Picker. `--render-only --output-dir DIR` writes SVG/PNG pairs for repeatability and visual inspection.

## Findings

### Dependency compatibility

`ratatui-image` 11.1.0 declares `ratatui = "^0.30.1"`. The spike compiles with the workspace's ratatui 0.30.2, and `cargo tree -p wiki-reader-tui -i ratatui --depth 2` shows one ratatui version shared by `wiki-reader` and `ratatui-image`.

The spike disables `ratatui-image` default features and enables only `crossterm`. Its direct `image` dependency enables only PNG. This avoids the default image codec set and Chafa integration while P3-12b decides the production feature set.

### Mermaid renderer go/no-go

**Decision: mixed, continue with per-block fallback.** The renderer is viable for ordinary diagrams, but naturally wide graphs become too small when fitted into an 80-column pane. P3-12c must reject an image result whose effective text size would be illegible and use the existing text tier for that block.

The fixture run covered the `fixtures/elements` good, wide and malformed cases; six common types in `fixtures/mermaid/common-types.md` (flowchart, sequence, state, class, ER and gantt); and five real diagrams from `wiki/`. Results:

- all 12 valid blocks rendered to SVG and PNG; the malformed fixture returned a useful strict parse error and cleanly took the fallback path;
- flowchart, sequence, state, class, ER and gantt output was structurally correct in PNG inspection;
- natural sizes ranged from 177×304 to 1767×274 pixels. Compact/common cases remain legible; the 1120–1767 px wide real graphs are not legible when scaled to roughly 640 px (80 columns at the observed 8 px cell width), and are marginal at roughly 960 px (120 columns);
- SVG and PNG files were byte-identical across two separate runs. The renderer cannot accept font bytes for layout and otherwise scans system fonts, so the spike sets an intentionally private family to force deterministic fallback metrics, then rewrites that family to embedded Noto Sans before `resvg`. No system font is required for the rendered result, though `mermaid-rs-renderer` still performs its system-font scan;
- the first diagram in a process paid roughly 100–310 ms for font discovery; warm simple diagrams were sub-millisecond to 18 ms, while the largest real graph took about 0.86 s. P3-12c therefore must keep generation and rasterisation off the UI thread as planned.

Footprint measured from commit `1f6d2f2` with fresh target directories and a warm crates.io source cache:

- lockfile: 322 → 361 packages (+39), including two `fontdb` versions (0.23 via `mermaid-rs-renderer`, 0.24 via `resvg`/`usvg`);
- cold release example build: 26.5 s for `image-protocol` at the baseline versus 48.9 s for `mermaid-image` (+22.4 s, about +85% on this machine);
- release example binary: 1,654,128 bytes versus 10,485,664 bytes (+8,831,536 bytes). This includes the 569,208-byte font and is a proxy for the production delta; measure the actual release binary again when P3-12b/c move these crates to normal dependencies.

### Query ordering

`Picker::from_query_stdio_with_options` must run:

1. after `ratatui::try_init` has entered raw mode and the alternate screen;
2. before `EnableMouseCapture`;
3. before `supports_keyboard_enhancement` / `PushKeyboardEnhancementFlags`;
4. before the first `crossterm::event::read`.

This matches ratatui-image's API documentation and avoids two terminal-query consumers competing for stdin. Production startup should preserve this exact order.

### Ghostty through Herdr 0.9.0

Observed inside a Herdr pane hosted by Ghostty:

```text
detected=Kitty selected=Kitty query=1.48ms
font=FontSize { width: 8, height: 17 }
capabilities=[Kitty, CellSize(Some((8, 17)))]
TERM=xterm-256color TERM_PROGRAM=ghostty TMUX=<unset> HERDR_ENV=1
```

A second run completed in 2.18 ms. The PNG appeared as a real image in the Herdr pane, confirmed visually from a screenshot. No forced protocol was needed.

Herdr's generated default config has `terminal.kitty_graphics = true`. Herdr exports `HERDR_ENV=1`, but it does **not** export the value of `terminal.kitty_graphics`; there is no reliable boolean env hint for that setting. The successful Kitty query is therefore the runtime confirmation. Under Herdr, accept only a queried `ProtocolType::Kitty`; never pass through Sixel or iTerm2.

### tmux

A nested tmux run still reported Kitty in 7.38 ms (`TERM=tmux-256color`, `TERM_PROGRAM=tmux`, `$TMUX` set). That confirms Picker can see Kitty through this particular tmux configuration, but it does not make tmux passthrough portable. The application must check `$TMUX` before probing and choose the text tier.

### Timeout behavior

The ratatui-image library default is two seconds. The spike now defaults to 250 ms and makes it tunable through `WIKI_READER_IMAGE_QUERY_TIMEOUT_MS`.

The library returns a Halfblocks fallback on timeout, but its 11.1.0 implementation cannot cancel the worker thread blocked on stdin. A timed-out worker may later consume application input. In a pseudo-terminal with no responder, 50, 250 and 500 ms configurations all returned from Picker and then hung in the immediately following keyboard-enhancement query for more than two seconds; each process required termination. The timeout is therefore only a guard, not safe general auto-detection.

For the alpha, do not probe unknown terminals. Probe only under Herdr (the verified path) or when environment hints identify a known graphics candidate such as Ghostty, Kitty, WezTerm or a maintained Sixel candidate; `$TMUX`, Terminal.app and unknown terminals select text without probing. The tested Herdr path answers immediately; if field reports find a Herdr configuration that does not answer, P3-12d must disable probing there until the upstream behavior changes.

## Detection rule for ADR-0004

1. Read the configured requested tier (`auto | image | text | source`). `text` and `source` need no terminal probe.
2. If `$TMUX` is set, the available graphics tier is text, including for an explicit `image` request; report the fallback.
3. If `HERDR_ENV=1`, run Picker in the startup position above. Kitty confirms the image tier; every other result selects text. Never use Sixel or iTerm2 under Herdr.
4. Outside Herdr, `TERM_PROGRAM=iTerm.app` selects iTerm2 explicitly: iTerm2 3.6.6 answered the Kitty probe but did not render the Kitty payload, while forced iTerm2 rendered correctly.
5. Probe other known graphics-terminal candidates. Kitty and iTerm2 select the supported image tier; Sixel selects the experimental image tier; Halfblocks selects text. Terminal.app and unknown terminals select text without probing.
6. Any image setup failure falls through to text and then source.

### Clipping, replacement and popup clearing

`StatefulImage` plus `Resize::Crop(Some(CropOptions { clip_top: true, .. }))` correctly removed rows from the top while keeping the remainder visible in Ghostty and Herdr. Partial slot visibility can therefore use crop rather than the full-slot-only fallback.

Popup and replacement checks were also clean in Ghostty and Herdr. The working draw/clear sequence is: render the image, render `Clear` over the popup rectangle, then render the popup block; closing the popup redraws the image. Replacing the `StatefulProtocol` before the next draw left no stale placement.

## Matrix

| Host | Probe | PNG | Status |
|------|-------|-----|--------|
| Ghostty → Herdr 0.9.0 | Kitty, 8×17 cells, 1–3 ms | real image | verified |
| Ghostty → Herdr → tmux | Kitty, 8×17 cells, 7.38 ms | ignored by policy | verified |
| Ghostty direct | Kitty, 0.984 ms | real image | verified |
| Terminal.app | Halfblocks, 1.18 ms | blurry halfblocks; text by policy | verified |
| iTerm2 3.6.6 | false Kitty, 17.84 ms; forced iTerm2, 26.52 ms | missing as Kitty; real image as iTerm2 | verified; explicit iTerm2 selection required |
| No-response pseudo-terminal | timeout at 50/250/500 ms, then later query hangs | none | verified; unknown terminals must not be probed |

ADR-0004 is accepted with the allowlisted-probe rule above. P3-12d must still retain a text fallback for query/setup failures.

## Risks passed to P3-12b/c

- P3-12b must treat protocol setup as fallible and keep the placeholder path available. `StatefulImage` top crop, popup clearing and protocol replacement are verified in Ghostty and Herdr; preserve the tested image → `Clear` → popup ordering.
- P3-12b should carry the detected `Picker`/cell size from startup rather than querying during render or after event reads.
- P3-12c must not assume a Kitty protocol merely because `HERDR_ENV=1`; the successful query is the capability proof. Wide/low-effective-font-size diagrams must fall back per block, and work remains off-thread because large graphs approached one second in the spike.
- A Picker timeout can leave a blocked stdin reader in ratatui-image 11.1.0. Do not retry the probe in-process and do not probe unknown terminals. iTerm2 must be selected explicitly from `TERM_PROGRAM` because its query falsely preferred Kitty in the tested version.
- The Mermaid/resvg additions add 39 locked packages and an approximately 8.8 MiB release-example delta. Re-measure the production binary before promotion from dev-dependencies.
