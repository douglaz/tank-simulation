○ tanksim-6e5.2.1 · Decide internal unit taxonomy and display policy   [● P0 · OPEN]
Owner: master · Type: spike
Created: 2026-03-26 · Updated: 2026-03-27
Labels: api, phase-1, scientific-core, tui, units

Context:
- Internally, several nitrogen pools are stored as mass of nitrogen (water.rs:12-19: ammonia_total_mg_n_total, etc.), while snapshots and UI labels currently read like full-ion concentrations (snapshot.rs:15-16: nitrite_mg_l, nitrate_mg_l).
- TDS and conductivity also need a clearer story about what is estimated, what is tracked, and what remains out of scope.

Deliverable:
- A written unit policy covering internal storage, helper method naming, and display naming.
- Clear decisions on when to use mg N/L, when to convert to ion-style mg/L for display, and how to label estimated TDS/conductivity.
- A pragmatic strategy for strong types/newtypes versus lighter naming conventions.

Key decisions to make:
1. Internal storage: keep as mg-element-total (current), or switch to mg-element-per-L?
   → Recommendation: keep totals internally (avoids recomputing volume on every read), provide concentration helpers.
2. Display: show mg N/L (scientific, compatible with EPA/test kit reporting) or mg-ion/L (hobby-friendly)?
   → Recommendation: offer both, label clearly. Conversion factors: NO3_ion = N × 62/14 ≈ 4.43×; NO2_ion = N × 46/14 ≈ 3.29×.
3. Type safety: full newtypes (MgNTotal, MgNPerL, MgIonPerL) or naming convention?
   → Recommendation: start with naming convention + a few key newtypes for the most error-prone conversions.

Likely touch points:
- crates/tank_core/src/types/water.rs
- crates/tank_core/src/types/snapshot.rs
- crates/tank_core/src/types/process.rs
- crates/tank_api
- crates/tank_tui

## Acceptance Criteria


Acceptance Criteria:
- field names and helper names have a consistent convention
- display policy is written down before refactors start
- the chosen strategy is explicit enough to prevent semantic drift in future data files

Dependencies:
  -> tanksim-6e5.1.1 (blocks) - Inventory current scientific semantics, units, invariants, and shortcuts
  -> tanksim-6e5.2 (parent-child) - Phase 1A — units, concentration semantics, and kinetic normalization

Dependents:
  <- tanksim-6e5.4.1 (blocks) - Define carbonate-state contract and solver strategy
  <- tanksim-6e5.2.2 (blocks) - Implement canonical concentration and compartment helper APIs
  <- tanksim-6e5.5.1 (blocks) - Introduce habitat registry and colonizable-area model
  <- tanksim-6e5.2.7 (blocks) - Update snapshot/API chemistry semantics, display conversions, and TDS/conductivity labeling

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- This is a design decision bead, not a bike-shed bead. Make the semantics obvious enough that future contributors do not have to guess what a field means.
- Prefer clarity over type-system maximalism. A smaller number of well-named helpers can beat a forest of wrapper types if the API is disciplined.
- Decide once, then apply consistently across engine, snapshots, data files, docs, and tests.

Scientific context on units:
- EPA ammonia criteria use TAN (Total Ammonia Nitrogen) in mg N/L. Most scientific literature uses N-as-element.
- Hobby test kits typically report NO3 and NO2 as full-ion mg/L (e.g., API kit reads NO3 as ion).
- The simulator serves both audiences: developers working with scientific literature, and players accustomed to hobby test-kit values.
- The unit policy should make this dual audience explicit.
