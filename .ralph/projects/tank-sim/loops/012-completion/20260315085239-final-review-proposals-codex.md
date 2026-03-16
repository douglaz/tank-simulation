---
artifact: final-review-proposals
loop: 12
project: tank-sim
backend: codex
role: final_reviewer
created_at: 2026-03-15T08:52:39Z
---

Reviewing the diff against `5fc621ceaf10b727133b212c30611636b0fa8340` first, then I’ll trace the changed implementation files end-to-end and validate behavior against the simulator contract.
The base diff is effectively the whole workspace plus committed build artifacts, so I’m narrowing to source and test files under `crates/` and tracing those directly.
I’ve isolated the real review surface to the Rust crates and tests. Next I’m reading the engine and system modules with line numbers so I can trace the simulation pipeline against the contract.
I’m in the core engine now. After the pipeline and state types, I’ll inspect the system modules that mutate chemistry, temperature, biology, and queued actions.
I’ve read the outer systems. Now I’m tracing the two riskiest modules: nitrogen-cycle mass flow and shrimp reproduction/mortality, since most spec regressions will hide there.
The test suite is useful here because it shows what behaviors are asserted versus what the code actually enforces. I’m checking the preset/fixture materialization next, since startup configuration and scenario overrides feed directly into simulation correctness.
# Final Review: AMENDMENTS

## Amendment: TS-REV-001 [P1]

### Problem
[`crates/tank_core/src/engine.rs`](/root/tank-simulation/crates/tank_core/src/engine.rs#L243) only validates water-change actions syntactically in `apply_action`; the source-profile existence/validity check is deferred to [`crates/tank_core/src/systems/water_change.rs`](/root/tank-simulation/crates/tank_core/src/systems/water_change.rs#L6) and only runs from [`crates/tank_core/src/engine.rs`](/root/tank-simulation/crates/tank_core/src/engine.rs#L45) during stepping. That breaks the public API contract that invalid actions return a typed validation error immediately, because `WaterChangePercent { source_profile_id: "bogus", ... }` is accepted into the queue and only fails later on `step_hours`.

### Proposed Change
Validate `WaterChangePercent` against `state.source_water_catalog` inside `apply_action` before enqueueing. Keep the step-time validation as a defensive check for deserialized queues, but do not rely on it for normal API correctness.

### Affected Files
- [`crates/tank_core/src/engine.rs`](/root/tank-simulation/crates/tank_core/src/engine.rs) - reject invalid water-change actions at enqueue time.
- [`crates/tank_core/src/systems/water_change.rs`](/root/tank-simulation/crates/tank_core/src/systems/water_change.rs) - expose shared single-action validation if you want to avoid duplicating logic.

## Amendment: TS-REV-002 [P1]

### Problem
The nitrification code double-charges alkalinity on the AOB->NOB path. It spends alkalinity budget for AOB at [`crates/tank_core/src/systems/nitrogen_cycle.rs`](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L160) and comammox at [`crates/tank_core/src/systems/nitrogen_cycle.rs`](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L194), but it also uses alkalinity to limit NOB at [`crates/tank_core/src/systems/nitrogen_cycle.rs`](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L228) and subtracts `aob_rate + comammox_rate + nob_rate` again at [`crates/tank_core/src/systems/nitrogen_cycle.rs`](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L271). That over-depletes KH/pH and can incorrectly stall nitrite oxidation when alkalinity is low.

### Proposed Change
Apply `alkalinity_meq_per_mg_n_nitrified` exactly once per nitrogen atom oxidized from ammonia, not again during nitrite oxidation. In practice, remove alkalinity from the NOB limiter and subtract alkalinity based on the ammonia-oxidation steps only, or on one full-path total, but not both.

### Affected Files
- [`crates/tank_core/src/systems/nitrogen_cycle.rs`](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs) - fix alkalinity budgeting and final subtraction.

## Amendment: TS-REV-003 [P1]

### Problem
[`crates/tank_core/src/systems/chemistry.rs`](/root/tank-simulation/crates/tank_core/src/systems/chemistry.rs#L48) changes DIC from respiration/photosynthesis, then [`crates/tank_core/src/systems/chemistry.rs`](/root/tank-simulation/crates/tank_core/src/systems/chemistry.rs#L52) also changes alkalinity in response to that DIC swing. The prompt only makes alkalinity a sink of nitrification and water changes; photosynthesis and respiration should move DIC and therefore pH, but they should not invent KH/alkalinity drift on their own.

### Proposed Change
Leave alkalinity unchanged in `step_hourly_chemistry` and recompute pH from the updated DIC plus the existing alkalinity total. Let nitrification and water changes remain the alkalinity drivers.

### Affected Files
- [`crates/tank_core/src/systems/chemistry.rs`](/root/tank-simulation/crates/tank_core/src/systems/chemistry.rs) - remove the DIC-driven alkalinity feedback term.

## Amendment: TS-REV-004 [P2]

### Problem
The clogging/maintenance loop is effectively dead in normal play. `cleanliness_index` starts at `1.0` in [`crates/tank_core/src/types/hardware.rs`](/root/tank-simulation/crates/tank_core/src/types/hardware.rs#L73), cleaning resets it to `1.0` in [`crates/tank_core/src/engine.rs`](/root/tank-simulation/crates/tank_core/src/engine.rs#L178), and the only daily clogging update multiplies by `1.0 - cleanliness_index` at [`crates/tank_core/src/systems/nitrogen_cycle.rs`](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L376). Because nothing ever lowers cleanliness, detritus never produces clogging unless a test manually seeds a dirty filter.

### Proposed Change
Either decay `cleanliness_index` over time from detritus/load, or drive clogging directly from detritus and derive cleanliness from that state. Cleaning should then restore both values consistently, so dirty-filter penalties can emerge without test-only setup.

### Affected Files
- [`crates/tank_core/src/systems/nitrogen_cycle.rs`](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs) - make clogging accumulate during normal simulation.
- [`crates/tank_core/src/engine.rs`](/root/tank-simulation/crates/tank_core/src/engine.rs) - keep the clean-filter action consistent with the new model.
- [`crates/tank_core/src/types/hardware.rs`](/root/tank-simulation/crates/tank_core/src/types/hardware.rs) - clarify/reset semantics for cleanliness if needed.

## Amendment: TS-REV-005 [P2]

### Problem
[`crates/tank_core/src/systems/water_change.rs`](/root/tank-simulation/crates/tank_core/src/systems/water_change.rs#L45) removes and replaces every major dissolved pool except `dissolved_oxygen_mg_total`. That means even a 100% water change preserves the pre-change oxygen mass. This is inconsistent with initial fills, which derive source-water oxygen from saturation at [`crates/tank_core/src/types/water.rs`](/root/tank-simulation/crates/tank_core/src/types/water.rs#L67), and it creates nonphysical oxygen states around maintenance actions.

### Proposed Change
Remove the exchanged fraction of `dissolved_oxygen_mg_total` during water changes and replace it with oxygen from the source water, at minimum derived from saturation at the source temperature to match initial-fill behavior.

### Affected Files
- [`crates/tank_core/src/systems/water_change.rs`](/root/tank-simulation/crates/tank_core/src/systems/water_change.rs) - mix DO along with the other dissolved pools.
- [`crates/tank_core/src/types/water.rs`](/root/tank-simulation/crates/tank_core/src/types/water.rs) - reuse the existing source-water saturation logic if you centralize it.

Tests were not run because this environment does not have `cargo` installed (`cargo: command not found`).
