○ tanksim-6e5.2.7 · Update snapshot/API chemistry semantics, display conversions, and TDS/conductivity labeling   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: api, phase-1, scientific-core, tui, units

Context:
- Snapshot and UI fields such as nitrite_mg_l and nitrate_mg_l (snapshot.rs:15-16) currently risk misleading users about whether values are nitrogen-as-N or full-ion concentrations.
- Current TDS/conductivity output also risks sounding more precise than it is, while undercounting relevant dissolved contributors.
- As the simulation gets more scientific, ambiguous output language becomes a product problem as well as an engineering problem.

Deliverable:
- Rename snapshot/API fields where needed or add explicit conversion/display fields so users can tell whether chemistry values are N-as-element, ion concentration, or another derived view.
- Centralize the conversion logic used by snapshot/API/TUI rather than scattering unit math through rendering code.
- Review TDS/conductivity against current model scope: add missing in-scope contributors where state already exists, and where the number remains approximate, relabel it honestly as an estimate or tracked-major-ions proxy.
- Update TUI/API/help text so surfaced chemistry metrics explain what is tracked directly, what is converted for display, and what important contributors are still omitted.
- Keep backward compatibility considerations visible if save/load or API consumers are affected.

Implementation options (per B1 decision):
  Option A: Rename to nitrite_mg_n_per_l, nitrate_mg_n_per_l (explicit N-as-element).
  Option B: Add dual fields: nitrite_mg_n_per_l (internal) + nitrite_mg_ion_per_l (display).
  Option C: Keep short names but add a unit label field or documentation.

Likely touch points:
- crates/tank_core/src/types/snapshot.rs
- crates/tank_api (response structs)
- crates/tank_tui (rendering logic)
- crates/tank_core/src/types/water.rs
- save/schema code if field names change

## Acceptance Criteria


Acceptance Criteria:
- a user reading the UI/API can tell what chemistry unit or metric is being shown, including whether a value is N-as-element, ion concentration, tracked major ions, or an estimate.
- internal and external naming plus conversion logic are centralized and consistent; unit tests verify NO3/NO2/NH4/PO4 conversion accuracy and round-trip behavior within tolerance.
- displayed TDS/conductivity either includes the currently in-scope tracked contributors or is explicitly labeled as an estimate/major-ion proxy rather than true total dissolved solids.
- tracked versus omitted contributors for TDS/conductivity are documented in user-facing text so future ions/fertilizers can extend the metric cleanly.

Dependencies:
  -> tanksim-6e5.2.2 (blocks) - Implement canonical concentration and compartment helper APIs
  -> tanksim-6e5.2.1 (blocks) - Decide internal unit taxonomy and display policy
  -> tanksim-6e5.2 (parent-child) - Phase 1A — units, concentration semantics, and kinetic normalization

Dependents:
  <- tanksim-6e5.7.5 (blocks) - Update developer docs, TUI/API messaging, and scientific-scope narrative

Comments:
  [2026-03-26 20:44 UTC] master: Canonicalization note: merged former B8 into B7 so user-facing chemistry semantics, display conversions, and TDS/conductivity honesty live in one place instead of two partially overlapping beads.
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- The point of scientific fidelity is undermined if the outputs are mislabeled.
- This bead should make the model easier to learn from, not just more correct internally.
- Keep old names only where compatibility truly demands it, and document any transitional shims clearly.

Conversion factors (for reference):
  NO3 as ion = N × (62.004/14.007) ≈ N × 4.427
  NO2 as ion = N × (46.005/14.007) ≈ N × 3.284
  NH4+ as ion = N × (18.039/14.007) ≈ N × 1.288
  NH3 as molecule = N × (17.031/14.007) ≈ N × 1.216
  PO4 as ion = P × (94.971/30.974) ≈ P × 3.066

These conversions should be defined as named constants, not magic numbers scattered through rendering code.
