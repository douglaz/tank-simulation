○ tanksim-6e5.3.5 · Split plant trimming into export vs leave-cuttings actions   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: actions, husbandry, phase-1, plants, scientific-core

Context:
- Current trimming behavior (engine.rs:189-197) turns all removed plant biomass into fine detritus, which only matches the specific case where clippings are left in the tank.
- Real aquarium maintenance often exports most trimmed biomass.

Deliverable:
- Replace the current single trimming action with explicit semantics for exported biomass versus in-tank cuttings.
- engine.rs currently has: PlayerAction::TrimPlants { fraction }. This should become two variants or a parameter:
  - TrimPlantsAndRemove { fraction } → biomass exits the system (export)
  - TrimPlantsAndLeaveCuttings { fraction } → biomass enters fine_detritus (current behavior)
- Update snapshots/events/UI wording so the player understands which maintenance action they chose.
- Ensure exported biomass is reflected correctly in conservation/budget logic (explicit export, not unexplained loss).

Likely touch points:
- crates/tank_core/src/engine.rs (lines 189-197)
- crates/tank_core/src/types/actions.rs (PlayerAction enum)
- crates/tank_core/src/types/events.rs (event descriptions)
- crates/tank_tui (trim action UI)

## Acceptance Criteria


Acceptance Criteria:
The action model distinguishes export from in-tank decay.
Event log / UI text reflects the distinction.
Budget instrumentation treats export as intentional removal rather than unexplained loss.
Unit test: TrimPlantsAndRemove reduces total N/C by exactly the exported biomass amount (within tolerance).
Unit test: TrimPlantsAndLeaveCuttings conserves total N/C (biomass moves to fine_detritus, nothing leaves the system).
Unit test: verify that old saves with the legacy TrimPlants action load and migrate correctly to the new action variant (via A2c migration path).
Integration test: a heavily planted scenario with weekly trim-and-remove shows gradual nutrient export over 500 hours — total N decreases proportional to cumulative exported biomass.

Dependencies:
  -> tanksim-6e5.3.1 (blocks) - Define closed-loop matter-routing conventions for consumers and maintenance actions
  -> tanksim-6e5.1.2.3 (blocks) - Add save-schema versioning and migration scaffolding
  -> tanksim-6e5.3 (parent-child) - Phase 1B — mass conservation and husbandry action semantics

Dependents:
  <- tanksim-6e5.3.6 (blocks) - Add conservation diagnostics and regression tests for grazing and maintenance loops

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- This bead is small but high-leverage because plant maintenance is common and strongly affects nutrient export.
- In real planted tanks, trimming-and-removing is a primary nutrient export mechanism. A heavily planted tank with regular trim-and-remove exports significant N and P, which reduces the need for water changes. The simulator should let the player experience and learn from this.
- Keep backward-compat/migration implications in mind if old saves reference the previous single trim action. The A2c migration scaffolding should handle this.

Design consideration:
- Two separate action variants is cleaner than a boolean flag, because the intent is more readable in event logs and action queues.
- Default should probably be TrimPlantsAndRemove (more common in practice) to avoid accidentally leaving clippings in the tank.
