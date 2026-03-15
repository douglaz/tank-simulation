---
artifact: completer-verdict
loop: 12
project: tank-sim
backend: codex(gpt-5.4-xhigh)
role: completer
created_at: 2026-03-15T08:38:52Z
---

I’m validating the workspace directly against `prompt.md` and the code at `HEAD`, not the prior loop summaries. First I’ll inventory the crates, tests, presets, and public API, then I’ll run the current test suite and spot-check the causal paths the prompt makes mandatory.
`prompt.md` is not present at repo root, so I’m treating the prompt text embedded in your message as the canonical requirement source and checking whether a repo-local copy exists elsewhere. Next I’m reading the core/public API, save format, and test inventory directly rather than relying on loop artifacts.
The prompt text exists only in the task context, not as `prompt.md` in the repo. I’ve confirmed the workspace shape and public action/error/snapshot types; next I’m checking the engine pipeline, state/save contents, and whether the TUI stays on the `TankSnapshot` boundary the prompt requires.
The TUI side looks structurally compliant: it renders from `TankSnapshot`, dispatches `PlayerAction`s, and does not reach into `TankState`. I’m moving to the data/scenario layer and then the simulation internals that are easy to miss in a completion claim: save contents, event retention, update ordering, and specific causal hooks for substrate, filter, oxygen, and reproduction.
I’ve confirmed the state includes the required dissolved pools, hidden guild state, and 200-event retention with 20-event snapshots. Now I’m checking the simulation systems line-by-line against the prompt’s causal contract, because that is where completion claims usually hide a missing pathway or a “close enough” shortcut.
The code audit is in good shape, but the shell doesn’t have `cargo` on `PATH`, so I’m locating the toolchain explicitly rather than assuming the planner’s test run still applies to this environment. After that I’ll run the workspace tests from the current `HEAD` myself.
The host image has `nix-shell` but no Rust toolchain on `PATH`, so I’m running the test suite inside a transient Nix shell. That will give me a direct pass/fail signal for current `HEAD` instead of inheriting the planner’s earlier result.
# Verdict: COMPLETE

The project satisfies all requirements:
- Workspace deliverable: the Cargo workspace contains `tank_core`, `tank_data`, `tank_tui`, and `tank_scenarios`, and `tank_core` exposes the required `SimulationEngine`, `PlayerAction`, `SimSeed`, `TankState`, and `TankSnapshot` API.
- Deterministic simulation contract: the engine uses a fixed 1-hour tick, queues actions for the next tick in insertion order, stores explicit RNG state with `ChaCha8Rng`, enforces invariants after updates, and saves/loads JSON with schema version, app version, full state, queued actions, and RNG state.
- Core ecosystem model: geometry-derived volume/area ratios, ambient plus heater thermal dynamics, dissolved oxygen, TAN/NH3/nitrite/nitrate cycling, alkalinity/DIC/pH updates, derived GH/KH/TDS/conductivity, layered substrate, both required plant guilds, suspended algae plus periphyton, decomposers and nitrifier guilds including comammox, aggregate microfauna, and shrimp condition/reproduction/molt/mortality are all implemented.
- Required user functionality: startup configuration lets the user choose tank size, fill, source water, substrate, plants, filter, light, heater, aeration, and shrimp count; runtime actions cover feeding, water changes, trimming, siphoning detritus, filter cleaning, add/remove shrimp, light changes, heater setpoint, ambient temperature, and aeration.
- Observability: machine-readable `SimEvent` logging with required event kinds and cause codes is present, the state retains the last 200 events, snapshots expose the last 20, and the exposed chemistry/biology metrics cover the prompt’s required observables.
- Data layer: TOML preset packs exist for all required source-water, substrate, plant, shrimp, process, and scenario IDs, with provenance blocks included in the shipped data files.
- TUI MVP: `Overview`, `Chemistry`, `Biology`, `Actions`, and `Log` screens are implemented, with 1-hour/1-day/1-week stepping, pause/autoadvance, save/load, and quit; the UI renders from `TankSnapshot` and dispatches `PlayerAction` rather than mutating low-level state directly.
- Acceptance coverage: the mandatory regression suite is present and passes at current `HEAD` via `cargo test --workspace`, including determinism, save/load resume, water-change mass balance, tank-size response, ambient temperature action, seeded vs unseeded cycling, overfeeding effects, rooted substrate advantage, thermal reproduction penalty, and event generation.
- Earlier completion gaps are closed: substrate trapping/grazing and filter enabled/flow now materially affect detritus routing, periphyton/shrimp food access, gas exchange, and cycling, with dedicated passing integration tests.
