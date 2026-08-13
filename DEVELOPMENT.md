# Development Guide

Native Rust desktop analyzer for the VRChat world Ecliptica. It tails the newest VRChat log, calculates per-second and 30-second DPS, tracks the active boss lock, shows a transparent always-on-top overlay, and broadcasts a configurable message to VRChat's OSC Chatbox.

## Development on macOS

```bash
cargo run
cargo test
python3 scripts/check-i18n-literals.py
```

The i18n literal check audits direct string literals passed to UI, toast, and
system-event sinks. It uses rust-analyzer's syntax tree when that command is
available and otherwise uses the bundled balanced-token fallback. Brand and
protocol notation are explicitly allowlisted; game-log parsing is out of scope.

On macOS, the checked-in sample under `data/` is used for parser/UI development. Override it at runtime with:

```bash
ECLIPTICA_LOG_PATH=/absolute/path/to/mock.txt cargo run
```

The production Windows build automatically discovers the lexicographically newest `output_log_*.txt` in `%USERPROFILE%\AppData\LocalLow\VRChat\VRChat`.

## Bump the version

`Cargo.toml` is the single source of truth. The build filename reads its version
automatically, and the command also refreshes `Cargo.lock`:

```bash
just up-version          # bump patch
just up-version minor    # bump minor and reset patch
just up-version major    # bump major and reset minor/patch
just up-version 0.4.0    # set an explicit version
```

## Build the Windows executable on macOS

The repository uses `cargo-xwin`, which provides the Windows SDK import libraries needed by the MSVC target without requiring a Windows host:

```bash
cargo install cargo-xwin --version 0.19.2 --locked
rustup target add x86_64-pc-windows-msvc
brew install mingw-w64
RC=x86_64-w64-mingw32-windres cargo xwin build --release --target x86_64-pc-windows-msvc
```

The single GUI executable is written to:

`dist/ecliptica-data-analyzer-v<VERSION>-windows-x64.exe`

The Windows executable embeds a transparent, rounded multi-size application
icon for Explorer, shortcuts, the taskbar, and Alt+Tab. The same artwork is
also assigned to the native eframe window at runtime.

VRChat OSC must be enabled in the Action Menu. The default destination is `127.0.0.1:9000`.

Message templates use Handlebars. Optional sections can use an availability flag, for example
`{{#if has_latest_dps}}DPS: {{latest_dps}}{{/if}}`. Supported flags include
`has_latest_dps`, `has_avg_dps`, `has_round_avg_dps`, and `has_max_dps`.
String values such as `boss_lock` and `boss` are empty when unavailable, so they
can be used directly in conditions. `is_self_boss_locked`
is true only while the configured player is the active Boss lock target; without a
configured player name it is always false. Whitespace-only rendered messages are not sent.
`rapid_damage_danger` becomes true when the combat log records strictly more than
50 incoming damage in the rolling last 10 seconds. It is a rapid-damage signal,
not an estimate of remaining health, and resets outside combat.
`no_dps_for_10s` becomes true when a non-spectating player has dealt no
damage for 10 seconds during combat, measured from the round start or latest hit.
It stays false while syncing, in the lobby, or waiting for the next round.
`no_wasd_for_10s` becomes true after ten seconds without a W, A, S, or D
key-down/repeat event and resets immediately on activity. The global listener is
observe-only and never reserves, injects, or suppresses keys. If the listener is
unavailable, the condition remains false. The same listener records the longest
continuous no-WASD interval in each live round as the shield-oriented standstill
metric; historical rounds scanned during startup cannot reconstruct this input data.
Rendered messages are trimmed before the Chatbox limit is applied, so hidden leading
or trailing conditional blocks do not leave blank boundary lines. Variable chips in
the settings window copy their displayed `{{variable}}` token to the OS clipboard.
Lock uses a short rising two-note cue, while unlock uses a sustained low release
chord; both now have a stronger base gain across the existing 0–1 volume range.

The normal combat message and completed-round report have independent templates and
share the same variables and condition syntax. Each template type has three persistent
preset slots. Switching slots preserves the current edit, and saving keeps all six
texts plus both selected slots for the next launch. Down-state detection and dedicated
first-down/second-down messages are intentionally unsupported because Ecliptica logs
do not expose a reliable event across sessions.

Room entry begins in an explicit unknown/synchronizing phase. The log proves that
VRChat entered Ecliptica, but it does not expose whether the local player is alive,
downed, or waiting to respawn. Stage, Boss, and Lobby records describe world state,
not local-player state. The analyzer therefore does not infer a pre-game lobby after
a timeout and does not expose a mid-session/death flag.

Personal round metrics start only after an explicit Lobby/Intermission marker followed
by an explicit Stage marker. Stage or Boss restoration records seen immediately after
joining may update the displayed world phase and Boss, but cannot start personal round
metrics or OSC combat messages. This intentionally waits for the next authoritative
round boundary instead of guessing participation.

When an intermission/lobby marker ends a round with personal output or incoming damage, the analyzer
archives a report until the next stage begins. Report variables are `round_duration`
(stage to lobby), `round_total_damage`,
`round_report_avg_dps`, `round_max_dps`, `round_report_effective_dps`,
`round_report_burst_10s`, and `round_report_damage_taken`.
`round_longest_standstill` is the longest continuous no-WASD time observed in the
round; use `has_round_longest_standstill` because the value is unavailable when the
global keyboard listener did not observe that round.
The report template is selected only while a report exists, so total damage and
damage taken need no presence flag. Metrics that can still be unavailable have
dedicated flags. In particular, `has_round_report_avg_dps` requires personal output
records. `has_round_max_dps` and `has_round_report_effective_dps` have
the same requirement, while `has_round_report_burst_10s` additionally requires
a complete ten-second window.
The default compact report remains comfortably below the VRChat Chatbox limit for
normal numeric values; rendered output still passes through the existing final limit.
The normal Overlay keeps its compact single-row four-metric layout and 204px height.
Short labels, constrained card widths, and tighter padding prevent the empty/no-damage
state from overflowing. Round reports use a compact two-row grid. Large values are
abbreviated with `万`/`亿`; hovering a value shows its full label and exact number.

## Data semantics

- `latest_dps`: damage in the previous complete wall-clock second.
- `avg_dps`: damage in the previous 30 complete seconds divided by 30, including zero seconds.
- `round_avg_dps`: average DPS from the first damage of the current round.
- `round_effective_dps`: round damage divided by the union of three-second
  post-hit intervals, excluding walking and long waits.
- `round_burst_10s`: highest average DPS among complete ten-second windows in
  the current round; unavailable until the first full window exists.
- `dps_growth_rate`: percentage change in effective DPS from the previous
  output round. It does not use the ten-second burst metric.
- `round_damage_taken`: cumulative personal incoming damage in the current round.
- `round_longest_standstill`: longest continuous interval without W/A/S/D input in
  the completed round, measured from round start and reset after every movement input.
- `max_dps`: highest complete-second DPS during the current Ecliptica visit.
- Both `STRIKE` and `NON-STRIKE` `Dealing N ... damage` events count.
- Stage/intermission, room exit, truncation, deletion, or log switch clears combat state.
- Leaving a room or entering another Ecliptica instance also clears visit peaks,
  reports, damage, Boss/lock, and pending room synchronization state.
- A stale/unreadable log stops OSC; old startup events never trigger OSC or sound.
- Display names are trimmed, Unicode NFKC-normalized, and compared case-insensitively.
