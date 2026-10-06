---
id: WR-SPIKE-P3-S2
title: P3-S2 herdr integration spike
summary: Ordinary-pane split launcher and plain-pane metadata are viable on Herdr 0.9.0; plugin overlay/popup are text-only (no cell metrics / no images); live theme-name following needs a config watcher, not a plugin.
status: done
updated: 2026-10-05
related: [phase-3-alpha, phase-4-beta, p3-s1-image-protocol]
---

# P3-S2 — herdr integration spike

**Shipped:** [P3-09](../phase-3-alpha.md) / [P3-10](../phase-3-alpha.md) in v0.1.4 (ordinary-pane split default; overlay/popup text-only).

## Spike program

Run from repository commit `731d43f` with the installed wiki-reader **0.1.3** and Herdr **0.9.0** client/server (protocol 22, compatible). Stable documentation currently describes **0.9.3**; installed commands and live responses are the authority for this run. No upgrade was performed.

The operator approved temporarily linking a locally authored plugin under `/tmp/wiki-reader-p3-s2`. Its manifest declared overlay, popup and split entrypoints, with an 80% × 80% popup. Each launched a Python context-logging wrapper and then the installed wiki-reader. There were no build/startup/event hooks or persistent keybinding changes.

```bash
herdr plugin link /tmp/wiki-reader-p3-s2
herdr plugin pane open --plugin wiki-reader.p3-s2-scratch --entrypoint overlay --no-focus
herdr plugin pane open --plugin wiki-reader.p3-s2-scratch --entrypoint popup --no-focus
herdr plugin unlink wiki-reader.p3-s2-scratch
```

The wrapper captured `HERDR_PLUGIN_CONTEXT_JSON` and plugin environment variables, changed directory to `focused_pane_cwd` (falling back to `workspace_cwd`), then launched `wiki-reader .`. Subsequent media checks used `fixtures/mermaid` relative to that directory and an explicit scratch-only config:

```toml
diagrams = "image"
[images]
enabled = true
```

No user or collection config was changed. Temporary context logs and the bundled API schema remain in the scratch directory for this run, not in the repository.

## Findings

### P3-09 — plugin pane

**Decision (revised after the retests below): plugin panes are viable for text, but not for images on herdr 0.9.x. Ship an ordinary-pane split launcher as the default and keep overlay and popup as text-only options.** The first spike pass reported real popup images; the shipping-plugin retest and the overlay and control runs below show that plugin panes of every placement get no cell metrics. Do not rely on the first popup image pass.

Manifest fields `id`, `name`, `version`, `min_herdr_version`, `platforms` and `[[panes]]` command argv are accepted. The 0.9.0 CLI help omits popup from `--placement`, but a manifest with `placement = "popup"`, `width = "80%"`, `height = "80%"` links and opens successfully. Do not infer lack of server support from that incomplete help.

Observed context contains `workspace_id`, `workspace_cwd`, `tab_id`, `focused_pane_id`, `focused_pane_cwd` and worktree checkout details. The command initially runs in the **plugin root**, not the collection directory; the launcher must explicitly use context. Logs confirm the resulting launch cwd is `/Users/dvz/Workspaces/wiki-reader`. `HERDR_BIN_PATH` and `HERDR_PLUGIN_ROOT` are injected.

Popup commands have **no `HERDR_PANE_ID`**. The context still identifies the underlying focused pane; that is not the popup's identity. Do not publish popup page metadata to that pane, which may host an unrelated agent.

Overlay and split commands do receive their own pane IDs. `--no-focus` preserved focus for split; overlay still took modal focus. Overlay restoration returned to the caller when closed.

The split CLI's `--target-pane` invocation returned `invalid_params: split and zoomed plugin panes target an existing pane; use target_pane_id`. The socket request succeeds using the schema's actual field:

```json
{"id":"p3-s2-split","method":"plugin.pane.open","params":{"plugin_id":"wiki-reader.p3-s2-scratch","entrypoint":"split","target_pane_id":"<caller pane ID>","direction":"right","focus":false}}
```

Use a newline-delimited JSON request over `HERDR_SOCKET_PATH`; obtain the new pane ID from the response. This workaround is specific evidence for 0.9.0, not a reason to hard-code pane IDs or require an upgrade.

### P3-10 — current-page metadata

**Decision: confirm feasibility for ordinary panes; omit publishing from popups.** A wiki-reader split pane stayed non-agent (`agent_status = unknown`, no agent identity) while accepting display metadata:

```bash
herdr pane report-metadata <reader-pane-id> --source wiki-reader \
  --title 'SPIKE PAGE themed.md' --token page=themed.md --ttl-ms 600000
herdr pane get <reader-pane-id>
```

`pane get` returned `title = "SPIKE PAGE themed.md"` and `tokens.page = "themed.md"`, with no agent registration. The operator confirmed that **title or token was visible in the sidebar**. The answer did not distinguish which one; do not claim both surfaces were individually verified.

Implementation must be best-effort and off the UI thread, use the reader's inherited pane ID, avoid reporting agent lifecycle state, and handle missing Herdr/binary/API without affecting navigation. Refresh on page changes and renew a finite TTL; validate expiry/clear behavior in implementation tests. Source isolation and TTL expiry were not separately exercised by this spike. Popup publishing is unavailable because popups have no pane ID; normal overlay/split/tab launches can publish their own page.

### P4-05 — live theme

**Decision: feasible for configured theme-name changes without a plugin; retain in Phase 4.** `crates/wiki-reader-core/src/herdr.rs` already reads `[theme] name` from herdr's config. Core already depends on `notify` and `notify-debouncer-mini`; watching and re-reading that file can update wiki-reader without a Herdr theme event/API.

This is a source/design finding, not a live-watch implementation or runtime test. A config watcher alone cannot observe terminal appearance changes under `auto_switch`, and custom palette mapping is separate work. Honour config replacement/atomic writes and retain the previous theme if re-reading fails. Also check `HERDR_CONFIG_PATH` handling when implementing: the current reader helper uses XDG/HOME rather than that override.

## P3-09 implementation retest — 2026-10-03

The operator tested the context-aware launcher from draft #128 with a temporary
linked copy, using the local debug binary by absolute path rather than replacing
the installed v0.1.3. Correct collection selection, Help/Esc/Ctrl+Enter/q and clean
dismissal passed. **Images failed**: placeholders say `no graphics protocol`, and
Mermaid uses text. Explicit `diagrams = "image"`, enabled images and a 2-second
probe timeout did not change the result. This blocks shipping acceptance despite
the earlier popup spike's visual pass; do not generalise that pass to this run.

A diagnostic popup captured the raw startup query response:

```text
query: ESC_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA ESC\\ ESC[c ESC[16t ESC[5n
reply: ESC_Gi=31;OK ESC\\ ESC[?62;22c ESC[0n
```

Kitty is confirmed, but the cell-size query has no reply. Separate `CSI 14t`,
`CSI 18t` and `CSI 16t` queries likewise produced only the final status reply.
`ratatui-image` 11.1.0 returns its default Halfblocks picker with a 10×20 font:
its source discards the confirmed protocol when no font size can be obtained
from the response or the PTY ioctl. That 10×20 is a library default, not measured
popup geometry. Client/server remain Herdr 0.9.0, protocol 22, no stale binary.

A second diagnostic waited 300 ms after entering the alternate screen before
querying. It still returned Halfblocks/10×20 with no capabilities. A fixed
startup sleep is therefore not an evidenced workaround.

Inspection of Herdr's tagged 0.9.0 source identifies a geometry-update gap:

- [Popup creation](https://github.com/herdrdev/herdr/blob/v0.9.0/src/app/popup.rs) starts the child with rows/columns but no initial cell metrics; the PTY/runtime start with zero pixel geometry.
- [`public_request_may_change_geometry` and `shell_endpoint_claims_geometry`](https://github.com/herdrdev/herdr/blob/v0.9.0/src/server/headless/client_views.rs) omit `PluginPaneOpen`. Geometry reapplication resizes the popup, but opening it does not request that reapplication.
- [Normal client-shell rendering](https://github.com/herdrdev/herdr/blob/v0.9.0/src/server/headless/render.rs) calls `render_client_shell_pane_surface` with `resize_panes = false`; merely drawing the new popup does not initialise its geometry.
- Herdr's own `xtwinops_size_queries_stay_silent_without_pixel_geometry` test in [pane terminal tests](https://github.com/herdrdev/herdr/blob/v0.9.0/src/pane/terminal.rs) asserts the silent replies we observed. The positive tests expect `CSI 6;height;width t` after a resize with real metrics.

The omission also remains in the inspected v0.9.3 and upstream master request
classifiers; an upgrade alone is not a verified fix. Proposed host-side fix:
initialise the popup PTY **and** virtual terminal with the owning client's
known cell geometry before child execution/probe handling, classify popup opens
as geometry-changing requests, and preserve updates on resize. Unknown geometry
must remain unknown, not guessed. A post-spawn resize alone still needs a test
for a child that queries immediately, because it can race startup.

Required upstream regression: launch a popup child that immediately sends the
Kitty/cell-size/status queries, assert confirmed Kitty and the actual host cell
size without a sleep or window resize, and repeat through both CLI/plugin action
and client keybinding paths. Check resize and multi-client ownership as well.
The initial investigation did not run a patched build; the subsequent local
patch below has native automated coverage, not real-popup visual acceptance.

No forced protocol, guessed cell geometry, Herdr upgrade, user config change or
dogfood binary replacement was performed. Both diagnostic runs' temporary
plugin was unlinked; `plugin list` was empty again. Resolve the capability/geometry
gap before completing P3-09; the initial feasibility verdict is not shipping proof.

### Local host patch — 2026-10-03

At the operator's request, prepared an uncommitted patch against upstream master
`5da0a01e1eedda054db0c81dd3a780000c40d9f0` (package 0.9.3), in an isolated scratch
checkout outside this repository. The patch and its validation notes are local
scratch artifacts: they are not part of wiki-reader, are not distributed and may no
longer exist. This section is the durable summary.

The patch initialises popup virtual-terminal and PTY pixel geometry before the
child starts, keeps size bookkeeping consistent, and classifies plugin pane
opens as geometry-changing on public and client endpoint paths. Public calls
use the tab's geometry controller; client endpoint calls use the invoking client,
restoring the foreground projection afterward. No dependencies, wire fields,
startup sleeps, forced protocol or guessed font metrics were added.

Five new regressions cover immediate argv/shell child cell-size queries plus
PTY ioctl pixel extents, unknown geometry, public/controller versus client
ownership, and request invalidation. Disabling initial metrics and the new
classifiers makes the four bug-detection tests fail; restoring them passes all
five and the native nextest suite (**3713 passed, 12 skipped**). Formatting,
native all-target clippy (installed 1.96.0, not pinned 1.96.1), six architecture
checks, 150 Python maintenance checks and Bun integration/docs tests pass.

Full `just check` remains unverified: `just` is missing (one workflow test cannot
run `just --dry-run`), and Windows cross-validation was not run. No SDK/license
downloads, install, restart or patched-client connection to the stable server
were performed. The patch remains uncommitted and unpublished; the authenticated
account is not on Herdr's approved-contributor list, so no upstream implementation
PR was opened. wiki-reader does not depend on this patch.

## P3-09 overlay and control retest — 2026-10-03

The operator repeated the check with the context-aware launcher in an **overlay**, and
ran the repository's `image-protocol` example (it prints the probe result on screen) in
an ordinary pane and in an overlay and a popup of a scratch plugin. herdr 0.9.0, Ghostty.

| Where it ran | `detected` / `selected` | Font | Capabilities | Picture |
|--------------|-------------------------|------|--------------|---------|
| Ordinary herdr pane (control) | Kitty / Kitty | 8×17 | Kitty, CellSize(8, 17) | drawn |
| Plugin overlay | Halfblocks / Halfblocks | 10×20 (library default) | none | halfblock fallback only |
| Plugin popup | Halfblocks / Halfblocks | 10×20 (library default) | none | halfblock fallback only |

Findings:

- The reader is not regressed: the same probe works in an ordinary pane.
- **Overlay is no better than popup.** The reader in the overlay showed the plain "no
  graphics protocol" reason and rendered Mermaid as text, as in the popup. Every pane
  that herdr starts for a plugin command lacks the terminal replies, whatever its placement.
  The local patch notes agree: ordinary command panes start with zero pixel metrics.
- Both plugin panes carry `HERDR_PLUGIN_ENTRYPOINT_ID` (checked in the spike's logged
  environments); only the overlay has `HERDR_PANE_ID`. wiki-reader therefore detects a plugin
  pane from `HERDR_ENV=1` plus `HERDR_PLUGIN_ENTRYPOINT_ID`, not from the missing pane id,
  and its "no graphics" placeholders and diagram headers now say plugin panes report no cell
  size and suggest a normal pane. Cell size is still never guessed.
- herdr's CLI can open an ordinary shell pane: `herdr pane split <pane> --direction right
  --cwd <dir> --focus` returns the new pane id (`.result.pane.pane_id`), and
  `herdr pane run <pane> <command>` submits a command to it. Both are documented in the 0.9.3
  CLI reference. The control run shows such panes answer the queries.

**Operator acceptance (2026-10-03).** With the shipping manifest linked from a scratch copy:
the `open` action's split pane and a plain ordinary pane both draw images and diagrams. The
overlay and popup entrypoints open, take keys and dismiss cleanly but are text-only, with the
new "herdr plugin panes report no cell size, open the reader in a normal pane" wording in
both, as predicted. That is the accepted behaviour for plugin panes on herdr 0.9.x.

**Decision:** the plugin's default action (`open`) runs `wiki-reader --herdr-split`, which
splits the focused pane, sets the new pane's cwd to the focused pane's, and runs
`exec wiki-reader` in it (so quitting closes the pane). Overlay and popup stay as
`open-overlay` and `open-popup`, documented as text-only until herdr starts plugin panes
with cell metrics. The ordinary-pane route has its own pane id, so P3-10 publishing works there.

## Matrix

| Placement / surface | Launch and context | Keys | Kitty images / cleanup | Status |
|---------------------|--------------------|------|------------------------|--------|
| Popup, 80% × 80% | logged caller cwd; no pane ID | operator: Help, Esc, Ctrl+Enter, q pass | first pass reported images; the shipping-plugin retest and the example both show no cell metrics, so no images | keys verified; images fail |
| Overlay | logged caller cwd; own pane ID; focus restored on close | operator: all keys pass | retest: no cell metrics, no images, Mermaid as text | keys verified; images fail |
| Plugin split | logged caller cwd; own pane ID; socket target workaround | automated: Help appears, Esc dismisses, Ctrl+Enter adds tabs | not visually checked; expected to match overlay | partial; no graphics claim |
| Ordinary shell pane (control, and the `open` action's target) | caller cwd via `pane split --cwd` | normal | example: Kitty, 8×17, picture drawn; operator 2026-10-03: the `open` action's split pane draws images and diagrams, and they survive theme switches | verified |
| Plain-pane metadata | API stores title and page token without agent registration | not applicable | operator: title or token visible in sidebar | display feasibility verified |

The earlier [P3-S1](p3-s1-image-protocol.md) verified Kitty in ordinary Herdr panes, which the control run reconfirms. Direct Ghostty, iTerm2 and tmux were not re-tested in this spike.

## Cleanup and implementation handoff

All test panes created by the spike were closed (the operator exited the verified popup); the scratch plugin was unlinked. `herdr plugin list` returned **No plugins installed**, matching the pre-spike state. Herdr retains plugin config/state directories after unlink by design; the scratch plugin stored no durable state there.

- P3-09: shipped in v0.1.4 — default action opens an ordinary split pane (`wiki-reader --herdr-split`); overlay and popup are text-only options. Documented the 0.9.0 `--placement` help omitting popup and the plugin-pane graphics limit.
- P3-10: shipped in v0.1.4 — publish from ordinary reader panes only; popup omission explicit in docs. Implemented as `Publisher` in `crates/wiki-reader/src/herdr.rs` ([ADR-0022](../../decisions/0022-herdr-launcher-and-page-publishing.md)); some title-versus-token / TTL checks remain for later dogfood.
- P4-05: config-watch seam is feasible independently of the plugin. Keep appearance/custom-palette limitations explicit.
- Spike closed; P3-09/10 done in v0.1.4. Phase 2 adoption clock unchanged.

## References

- [Herdr 0.9.3 plugin documentation](https://raw.githubusercontent.com/herdrdev/herdr/v0.9.3/docs/next/website/src/content/docs/plugins.mdx)
- [Herdr stable documentation index](https://herdr.dev/llms.txt)
- Installed `herdr plugin`, `plugin pane open`, `pane report-metadata` help and `herdr api schema` captured during the run.
