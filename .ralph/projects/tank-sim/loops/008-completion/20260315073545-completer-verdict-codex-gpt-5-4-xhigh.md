---
artifact: completer-verdict
loop: 8
project: tank-sim
backend: codex(gpt-5.4-xhigh)
role: completer
created_at: 2026-03-15T07:35:45Z
---

Reviewing `prompt.md` and the workspace from a validation angle, not the planner’s milestone summary. I’m starting by locating the spec, crate structure, tests, and the public API/TUI boundaries before deciding whether any requirement is still missing.
I’m gathering the repo shape and the actual prompt file now, then I’ll cross-check the code against the mandatory surface area instead of relying on loop artifacts.
`prompt.md` is not at the repo root, so I’m locating the canonical copy and then inspecting the core crate surfaces and tests against it. After that I’ll run validation commands from the code’s point of view, not the planner’s.
I have the public engine/action surface. Next I’m resolving the prompt file location, then I’ll inspect state, save/load, presets, scenarios, and the TUI screens for concrete coverage.
The save surface is straightforward: full `TankState` already embeds the RNG, and the save wrapper includes schema/app version plus queued actions. I’m now checking whether the repo actually exposes all mandatory presets, scenarios, and TUI controls/screens, and whether the prompt snapshot inside the repo matches the prompt you provided.
I’ve confirmed the preset inventory exists. The next pass is on scenario materialization and the TUI boundary, because those are where “looks complete” projects usually still miss user-facing requirements.
The TUI startup flow already suggests a likely gap: it exposes scenario, size, and fill, but not independent selection of substrate, source water, plants, filter, light, heater, or initial shrimp count. I’m confirming that against the input/actions implementation before I decide the verdict.
The action screen covers runtime actions, but it does not solve initial configuration. I’m checking the display screens now to make sure the required observability is present, and then I’ll run the workspace tests to separate “feature gap” from “broken implementation.”
The display side is present. I’m checking scenario materialization now because if hardware, plants, or substrate are hard-coded into scenarios with no user-selectable path, that is a concrete v1 requirement miss rather than a polish issue.
Scenario data confirms those choices are baked into three scenario presets. I’m running the workspace tests now and then I’ll make the call based on the prompt contract, not just green tests.
`cargo` is not on `PATH` in this shell, so I’m locating the toolchain directly and retrying. That won’t affect the feature verdict either way, but it matters for confirming the code still passes cleanly.
I found a complete Rust toolchain under `/nix/store`. I’m rerunning the checks with that toolchain on `PATH`; the important validation question remains the user-facing requirement coverage, but this will confirm the repo isn’t also failing mechanically.
The workspace tests are green. I’m doing one last pass on initialization and hardware defaults, because this repo can still fail the prompt even with a passing suite if the startup path hard-codes required user choices.
There’s a stronger initialization gap than I expected: scenario materialization does not carry any startup shrimp count or startup hardware choices at all. I’m checking the scenario helpers to see whether this was intentionally left to tests only, which would still leave the user-facing requirement unsatisfied.
I have enough to make the core call: tests pass, but startup configuration is still bundled into scenarios and defaults rather than exposing the required per-component choices. I’m collecting exact line references now so the verdict is concrete.
I’ve pinned the startup and scenario code paths. One more set of references on hardware defaults will make the missing requirements explicit instead of inferential.
There is another likely user-facing miss: the TUI exposes a heater setpoint action, but I haven’t seen any path that actually enables the heater for a player-started tank. I’m confirming that before finalizing the verdict.
I’ve confirmed the heater issue too: in the shipped startup path it stays at defaults, and there’s no player control to enable it. I’m checking one last implementation detail on event retention, then I’ll write the verdict.
# Verdict: CONTINUE

## Missing Requirements
1. `The MVP must allow a user to choose substrate, source water, plants, filter, light, heater, and shrimp count`: not satisfied. The startup flow only exposes `Scenario`, `Tank size`, `Fill height`, and `Launch` in [main.rs](/root/tank-simulation/crates/tank_tui/src/main.rs#L105) and [main.rs](/root/tank-simulation/crates/tank_tui/src/main.rs#L243). The scenario model hard-bundles `source_water_id`, `substrate_ids`, and `plant_ids` and has no fields for filter, light, heater, or shrimp count in [presets.rs](/root/tank-simulation/crates/tank_data/src/presets.rs#L750). Materialization then applies those bundled values and otherwise falls back to defaults in [lib.rs](/root/tank-simulation/crates/tank_scenarios/src/lib.rs#L172) and [lib.rs](/root/tank-simulation/crates/tank_scenarios/src/lib.rs#L295), so the user cannot independently choose the required components.
2. `choose ... heater` is still unmet in the playable product. Player-started tanks inherit `HeaterState::default()` with `enabled: false` in [hardware.rs](/root/tank-simulation/crates/tank_core/src/types/hardware.rs#L60), and the TUI only exposes `ChangeHeaterSetpoint` rather than any way to enable or choose a heater in [actions.rs](/root/tank-simulation/crates/tank_tui/src/screens/actions.rs#L18) and [actions.rs](/root/tank-simulation/crates/tank_tui/src/screens/actions.rs#L193). The same default path also leaves shrimp count at zero unless the user adds shrimp later via runtime action in [biology.rs](/root/tank-simulation/crates/tank_core/src/types/biology.rs#L152), which is not the required startup choice.

## Recommended Next Features
1. Add explicit startup configuration controls for source water, 1-3 substrate layers, plant pack, filter preset, light preset, heater enabled/preset, and initial shrimp count, then materialize those choices into `TankState` instead of relying on scenario-bundled defaults.
2. Extend the scenario/startup config model to include initial hardware and population settings, and add TUI-level tests that cover heater-enabled launches and non-zero initial shrimp counts.
