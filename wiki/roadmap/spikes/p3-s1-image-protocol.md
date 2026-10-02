---
id: WR-SPIKE-P3-S1
title: P3-S1 terminal image protocol spike
summary: ratatui-image 11.1 works with ratatui 0.30.2; Kitty probing and PNG drawing work through Herdr on Ghostty.
status: active
updated: 2026-10-02
related: [phase-3-alpha, 0004-diagram-rendering]
---

# P3-S1 — terminal image protocol spike

## Spike program

Run the temporary example from the repository root:

```bash
cargo run -p wiki-reader --example image-protocol
cargo run -p wiki-reader --example image-protocol -- --force-kitty
WIKI_READER_IMAGE_QUERY_TIMEOUT_MS=500 cargo run -p wiki-reader --example image-protocol
```

It enters the alternate screen with `ratatui::try_init`, immediately runs the `ratatui-image` stdio query, then enables mouse capture and Kitty keyboard disambiguation. It reports the selected protocol, cell size, capabilities, relevant environment variables and query duration before drawing `assets/wiki-reader.png`.

## Findings

### Dependency compatibility

`ratatui-image` 11.1.0 declares `ratatui = "^0.30.1"`. The spike compiles with the workspace's ratatui 0.30.2, and `cargo tree -p wiki-reader -i ratatui --depth 2` shows one ratatui version shared by `wiki-reader` and `ratatui-image`.

The spike disables `ratatui-image` default features and enables only `crossterm`. Its direct `image` dependency enables only PNG. This avoids the default image codec set and Chafa integration while P3-12b decides the production feature set.

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

The default ratatui-image query timeout is two seconds and the spike makes it tunable through `WIKI_READER_IMAGE_QUERY_TIMEOUT_MS`.

The library returns `NoStdinResponse` on timeout, but its 11.1.0 implementation cannot cancel the worker thread blocked on stdin. A timed-out worker may later consume application input. This is not a clean failure mode for a long-running event loop. Avoid launching the query in environments that are known not to forward it (`$TMUX` in wiki-reader policy). The tested Herdr path answers immediately; if field reports find a Herdr configuration that does not answer, P3-12d must disable probing there until the upstream behavior changes.

## Detection rule for ADR-0004

1. Read the configured requested tier (`auto | image | text | source`). `text` and `source` need no terminal probe.
2. If `$TMUX` is set, the available graphics tier is text, including for an explicit `image` request; report the fallback.
3. If `HERDR_ENV=1`, run Picker in the startup position above. Kitty confirms the image tier; every other result selects text. Never use Sixel or iTerm2 under Herdr.
4. Otherwise use Picker auto-detection. An image-tier setup failure falls through to text and then source.

## Matrix

| Host | Probe | PNG | Status |
|------|-------|-----|--------|
| Ghostty → Herdr 0.9.0 | Kitty, 8×17 cells, 1–3 ms | real image | verified |
| Ghostty → Herdr → tmux | Kitty, 8×17 cells, 7.38 ms | ignored by policy | verified |
| Ghostty direct | | | pending manual run |
| Non-Kitty terminal | | | pending manual run |

ADR-0004 remains proposed until the two direct-terminal rows are checked. The completed Herdr result removes the main blocker for P3-12b implementation, but P3-12d must retain a text fallback for query/setup failures.

## Risks passed to P3-12b/c

- P3-12b must treat protocol setup as fallible and keep the placeholder path available. Partial image clipping and explicit clearing remain the highest rendering risks.
- P3-12b should carry the detected `Picker`/cell size from startup rather than querying during render or after event reads.
- P3-12c must not assume a Kitty protocol merely because `HERDR_ENV=1`; the successful query is the capability proof.
- A Picker timeout can leave a blocked stdin reader in ratatui-image 11.1.0. Do not retry the probe in-process.
