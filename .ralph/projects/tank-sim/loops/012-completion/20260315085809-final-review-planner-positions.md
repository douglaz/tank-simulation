---
artifact: final-review-planner-positions
loop: 12
project: tank-sim
backend: codex(gpt-5.4-xhigh)
role: planner
created_at: 2026-03-15T08:58:09Z
---

Reviewing the cited Rust modules to verify each amendment against the actual implementation. No listed skill applies here; this is direct source inspection across `engine`, `water_change`, `nitrogen_cycle`, `chemistry`, and related types.
Initial source pass confirms the cited code paths exist. I’m now checking action validation types, error variants, and any tests/docs that define whether invalid actions must fail at `apply_action` rather than at `step_hours`.
I’ve confirmed `TS-REV-001` is not just speculative: the current tests explicitly expect bad source profiles to be accepted by `apply_action` and rejected only on `step_hours`, which is the opposite of the amendment’s claim about the public API. I’m checking whether the other amendments are actual defects or just disagreements with the chosen model.
I have enough structure now to evaluate each amendment. I’m pulling line-numbered excerpts for the exact decision points so the final positions cite the implemented behavior rather than paraphrasing it.
I’m checking the written model contract in `spec.md` for alkalinity and water-change semantics now. `TS-002` already looks like a real stoichiometry defect from the code itself; `TS-003` depends on whether the spec actually reserves alkalinity changes for nitrification and water changes.
The clogging issue is confirmed in code: `cleanliness_index` never decays anywhere, yet daily clogging is multiplied by `(1 - cleanliness_index)`. I’m checking `dissolved_oxygen` next because `TS-005` depends on whether a maintenance-time DO correction happens elsewhere in the same tick.
# Planner Positions

## Amendment: TS-REV-001

### Position
ACCEPT

### Rationale
The contract language in the amendment is stronger than what the repo documents, but the underlying defect is real. `apply_action` only runs syntactic validation for `WaterChangePercent` and then queues the action ([actions.rs](/root/tank-simulation/crates/tank_core/src/types/actions.rs#L86), [engine.rs](/root/tank-simulation/crates/tank_core/src/engine.rs#L243)), while source-profile existence and profile validity are only checked when stepping ([engine.rs](/root/tank-simulation/crates/tank_core/src/engine.rs#L45), [water_change.rs](/root/tank-simulation/crates/tank_core/src/systems/water_change.rs#L6)). That means the engine can report success for an action it already has enough state to reject, and the bad queued action then blocks later stepping; the current tests explicitly codify that deferred failure ([water_change_mass_balance.rs](/root/tank-simulation/crates/tank_core/tests/water_change_mass_balance.rs#L204)). Keeping step-time validation still makes sense for restored/arbitrary queues ([save.rs](/root/tank-simulation/crates/tank_core/src/save.rs#L50)), but enqueue-time rejection is a real robustness improvement.

## Amendment: TS-REV-002

### Position
ACCEPT

### Rationale
The code does double-charge the single alkalinity constant on the AOB→NOB path. `alk_budget` is reduced after AOB and comammox ([nitrogen_cycle.rs](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L160), [nitrogen_cycle.rs](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L194)), NOB is then still limited by the remaining alkalinity budget ([nitrogen_cycle.rs](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L228)), and the final state subtraction charges `(aob_rate + comammox_rate + nob_rate) * alk_per_mg_n` ([nitrogen_cycle.rs](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L271)). That is inconsistent with the single parameter definition `alkalinity_meq_per_mg_n_nitrified` ([process.rs](/root/tank-simulation/crates/tank_core/src/types/process.rs#L76)) and with the nearby comment that treats “fully nitrified” nitrogen as `nob_rate + comammox_rate` ([nitrogen_cycle.rs](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L241)). The amendment describes a real stoichiometry bug.

## Amendment: TS-REV-003

### Position
ACCEPT

### Rationale
`step_hourly_chemistry` changes DIC and then directly changes alkalinity from that same DIC delta ([chemistry.rs](/root/tank-simulation/crates/tank_core/src/systems/chemistry.rs#L50), [chemistry.rs](/root/tank-simulation/crates/tank_core/src/systems/chemistry.rs#L52)). The model spec separates alkalinity tracking from DIC tracking, calling out nitrification as the acidifying effect and photosynthesis/degassing as DIC effects ([spec.md](/root/tank-simulation/spec.md#L758), [spec.md](/root/tank-simulation/spec.md#L760), [spec.md](/root/tank-simulation/spec.md#L761)). As implemented, light/dark carbon swings can create or destroy KH even when no nitrification or water change occurred. That is a real chemistry-model correctness gap.

## Amendment: TS-REV-004

### Position
ACCEPT

### Rationale
The normal runtime does not appear to have any path that lowers `cleanliness_index`. It defaults to `1.0` ([hardware.rs](/root/tank-simulation/crates/tank_core/src/types/hardware.rs#L78)), cleaning resets it to `1.0` ([engine.rs](/root/tank-simulation/crates/tank_core/src/engine.rs#L179)), and daily clogging growth is multiplied by `1.0 - cleanliness_index` ([nitrogen_cycle.rs](/root/tank-simulation/crates/tank_core/src/systems/nitrogen_cycle.rs#L376)). I found only test/setup code assigning lower cleanliness or pre-seeded clogging ([substrate_filter_integration.rs](/root/tank-simulation/crates/tank_core/tests/substrate_filter_integration.rs#L63), [cycling.rs](/root/tank-simulation/crates/tank_core/tests/cycling.rs#L353)). So in ordinary play the clogging mechanic cannot emerge on its own, which makes this a real dead-state defect.

## Amendment: TS-REV-005

### Position
ACCEPT

### Rationale
`apply_water_change` removes and replaces many dissolved pools but does not touch `dissolved_oxygen_mg_total` at all ([water_change.rs](/root/tank-simulation/crates/tank_core/src/systems/water_change.rs#L45)). That means even a 100% water change preserves the old oxygen mass until later hourly reaeration runs ([dissolved_oxygen.rs](/root/tank-simulation/crates/tank_core/src/systems/dissolved_oxygen.rs#L15)). Initial fills are handled differently: `WaterState::from_source_profile` derives DO from saturation at the source temperature ([water.rs](/root/tank-simulation/crates/tank_core/src/types/water.rs#L67)). Given the spec’s “removes dissolved and suspended constituents proportionally” water-change semantics ([spec.md](/root/tank-simulation/spec.md#L1002)), excluding DO is a genuine consistency and physics bug.
