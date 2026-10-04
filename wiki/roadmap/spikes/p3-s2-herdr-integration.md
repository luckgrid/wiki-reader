---
id: WR-SPIKE-P3-S2
title: P3-S2 herdr integration spike
summary: Plugin popup and plain-pane metadata are viable on Herdr 0.9.0; live theme-name following needs a config watcher, not a plugin.
status: complete
updated: 2026-10-03
related: [phase-3-alpha, phase-4-beta, p3-s1-image-protocol]
---

# P3-S2 — herdr integration spike

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

**Decision: confirm feasibility; implement the popup path.** The operator verified popup images, key delivery and clean dismissal on the real install. This is spike evidence, not completion of the shipping plugin task.

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
No patched Herdr build was run; this is a source-backed fix proposal, not a
validated implementation.

No forced protocol, guessed cell geometry, Herdr upgrade, user config change or
dogfood binary replacement was performed. Both diagnostic runs' temporary
plugin was unlinked; `plugin list` was empty again. Resolve the capability/geometry
gap before completing P3-09; the initial feasibility verdict is not shipping proof.

## Matrix

| Placement / surface | Launch and context | Keys | Kitty images / cleanup | Status |
|---------------------|--------------------|------|------------------------|--------|
| Popup, 80% × 80% | logged caller cwd; no pane ID | operator: Help, Esc, Ctrl+Enter, q pass | operator: real images, no fragments after Help or exit | verified |
| Overlay | logged caller cwd; own pane ID; focus restored on close | automated: Help appears, Esc dismisses, Ctrl+Enter adds a tab | operator could not verify this placement now | partial; no graphics claim |
| Split | logged caller cwd; own pane ID; socket target workaround | automated: Help appears, Esc dismisses, Ctrl+Enter adds tabs | explicit-image split not visually checked | partial; no graphics claim |
| Plain-pane metadata | API stores title and page token without agent registration | not applicable | operator: title or token visible in sidebar | display feasibility verified |

The earlier [P3-S1](p3-s1-image-protocol.md) verified Kitty in ordinary Herdr panes. It does not substitute for the outstanding placement-specific visual checks here. Direct Ghostty, iTerm2 and tmux were not re-tested in this spike.

## Cleanup and implementation handoff

All test panes created by the spike were closed (the operator exited the verified popup); the scratch plugin was unlinked. `herdr plugin list` returned **No plugins installed**, matching the pre-spike state. Herdr retains plugin config/state directories after unlink by design; the scratch plugin stored no durable state there.

- P3-09: ship a small manifest/launcher using context cwd; use popup as the verified default. Test missing/malformed context and command failure. Document the 0.9.0 help mismatch; do not promise unverified overlay/split media behavior.
- P3-10: publish from ordinary reader panes only; popup omission must be explicit in docs. Confirm title versus token display, update/renew/clear behavior and unsupported-server fallback before closing the row.
- P4-05: config-watch seam is feasible independently of the plugin. Keep appearance/custom-palette limitations explicit.
- No task row is marked done by this spike, no release is cut, and the Phase 2 adoption clock remains unchanged.

## References

- [Herdr 0.9.3 plugin documentation](https://raw.githubusercontent.com/herdrdev/herdr/v0.9.3/docs/next/website/src/content/docs/plugins.mdx)
- [Herdr stable documentation index](https://herdr.dev/llms.txt)
- Installed `herdr plugin`, `plugin pane open`, `pane report-metadata` help and `herdr api schema` captured during the run.
