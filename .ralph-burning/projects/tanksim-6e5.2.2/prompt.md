○ tanksim-6e5.2.2 · Implement canonical concentration and compartment helper APIs   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: chemistry, helpers, phase-1, scientific-core, units

Context:
- Multiple systems currently compute limitations directly from tank totals. For example, nitrogen_cycle.rs:90 computes doc_total / (doc_total + k_doc) using state.water.dissolved_organic_carbon_mg_c_total directly.
- The code needs one canonical place to ask for concentrations, areal densities, or compartment-specific views so later systems stop re-deriving them ad hoc.

Deliverable:
- Helper methods for canonical concentration queries such as TAN, nitrite, nitrate, phosphate, DOC, DIC, and dissolved oxygen.
- Volume helpers on TankState or a ConcentrationView struct.
- Where needed, helpers for habitat/compartment area- or volume-normalized quantities.
- A small API that later systems can use instead of touching raw totals directly.

Specific helpers to implement (at minimum):
  tan_mg_n_per_l()          — TAN concentration as mg N/L
  nitrite_mg_n_per_l()      — nitrite concentration as mg N/L
  nitrate_mg_n_per_l()      — nitrate concentration as mg N/L
  doc_mg_c_per_l()          — dissolved organic carbon as mg C/L
  dic_mg_c_per_l()          — dissolved inorganic carbon as mg C/L
  do_mg_per_l()             — dissolved oxygen as mg O2/L
  phosphate_mg_p_per_l()    — phosphate as mg P/L
  alkalinity_meq_per_l()    — alkalinity as meq/L
  water_volume_l()          — from geometry (length × width × fill_height - substrate_volume)

Likely touch points:
- crates/tank_core/src/types/water.rs (new impl block or module)
- crates/tank_core/src/types/state.rs
- crates/tank_core/src/types/geometry.rs

## Acceptance Criteria


Acceptance Criteria:
- major process systems can call helpers instead of reimplementing concentration math
- helper naming matches the unit policy from B1
- new helper coverage is enough to support nitrogen, plant, algae, and later carbonate refactors
- unit tests for every helper method: known total ÷ known volume = expected concentration, with edge cases (zero volume → graceful handling, empty tank, very large tank)
- helpers never return negative values (property test)

Dependencies:
  -> tanksim-6e5.2.1 (blocks) - Decide internal unit taxonomy and display policy
  -> tanksim-6e5.2 (parent-child) - Phase 1A — units, concentration semantics, and kinetic normalization

Dependents:
  <- tanksim-6e5.4.2 (blocks) - Implement explicit carbonate equilibrium and pH solver
  <- tanksim-6e5.3.2 (blocks) - Implement shrimp ingestion → assimilation → excretion → feces → respiration loop
  <- tanksim-6e5.2.3 (blocks) - Normalize nitrogen-cycle kinetics to concentration-based terms
  <- tanksim-6e5.4.7 (blocks) - Validate nitrification-driven alkalinity depletion with the carbonate solver, expose diagnostics, and prepare the denitrification return path
  <- tanksim-6e5.3.3 (blocks) - Implement microfauna matter routing and recycling
  <- tanksim-6e5.2.7 (blocks) - Update snapshot/API chemistry semantics, display conversions, and TDS/conductivity labeling
  <- tanksim-6e5.2.5 (blocks) - Normalize algae kinetics around concentration, light, and temperature interactions
  <- tanksim-6e5.2.4 (blocks) - Normalize plant nutrient uptake and growth limitation semantics

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Future-self notes:
- Treat these helpers as the anti-regression layer that prevents total-mass kinetics from creeping back in later.
- Keep the API narrow and opinionated; it is better to have a small number of clearly named helpers than many nearly-duplicate accessors.
- Where a quantity is only an estimate, encode that in the name or docs rather than implying false precision.
- The helpers need access to both WaterState (for pool totals) and TankGeometry (for volume). Consider impl on TankState or a method that takes both.
- snapshot.rs already does some of these conversions (lines 66-78). Consolidate there to avoid duplication.
