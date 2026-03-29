○ tanksim-6e5.3.7 · Implement organism death → detritus routing for shrimp, plants, and algae   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-26
Labels: ecology, mass-balance, phase-1, scientific-core

Context:
- The reviewer comment on C1 identified a significant conservation gap: when shrimp die (shrimp.rs lines 440-480), population counts decrement but dead organism body mass is NOT returned to the detritus pool. Dead shrimp biomass currently vanishes.
- Plant senescence partially routes biomass to detritus in plant_growth.rs but should be verified.
- Algae death/senescence routing should also be verified.
- This routing is separate from the feeding/waste loop (C2) and needs its own implementation.

Deliverable:
- Implement death → detritus routing for shrimp: when shrimp die (any cause), body mass enters fine_detritus_g_total with appropriate C:N composition based on shrimp body composition.
- Verify and fix plant senescence → detritus routing in plant_growth.rs.
- Verify and fix algae death → detritus routing in algae_growth.rs.
- Add a named routing parameter: death_biomass_to_detritus_fraction = 1.0 (all dead biomass becomes detritus unless explicitly exported, e.g., "remove dead shrimp" action).
- Add a future-ready "remove dead organisms" player action concept (deferred to later, but routing must distinguish in-tank decay from export).

Key calculations for shrimp death routing:
  dead_shrimp_count × average_individual_mass_g → fine_detritus_g_total
  N content: shrimp body ≈ 10-12% N by dry mass, ~2-3% N by wet mass
  C content: shrimp body ≈ 40-45% C by dry mass, ~10-12% C by wet mass
  These composition factors should be named parameters in process or species data.

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs (mortality logic, lines 440-480)
- crates/tank_core/src/systems/plant_growth.rs (senescence routing)
- crates/tank_core/src/systems/algae_growth.rs (death routing)
- crates/tank_core/src/types/process.rs (new routing parameters)
- crates/tank_core/src/types/biology.rs (body composition parameters)

## Acceptance Criteria
- shrimp death returns body mass to fine_detritus_g_total proportional to dead count × average body mass
- plant senescence routing to detritus is verified correct (N and C both routed)
- algae death routing to detritus is verified correct
- a named death_biomass_to_detritus_fraction parameter exists (default 1.0)
- body composition parameters (N fraction, C fraction) are named and documented
- unit test: kill N shrimp in a closed system, verify total_N and total_C are conserved (detritus increases by expected amount)
- unit test: plant senescence in a closed system conserves N and C
- unit test: algae biomass decrease in a closed system conserves N and C
- integration test: run 200 hours of a high-mortality scenario (high temp + poor water), verify detritus accumulates as population declines and total N/C are conserved throughout

Dependencies:
  -> tanksim-6e5.3.1 (blocks) - Define closed-loop matter-routing conventions for consumers and maintenance actions
  -> tanksim-6e5.3 (parent-child) - Phase 1B — mass conservation and husbandry action semantics

Dependents:
  <- tanksim-6e5.3.6 (blocks) - Add conservation diagnostics and regression tests for grazing and maintenance loops
  <- tanksim-6e5.3.4 (blocks) - Audit feeding, detritus, DOC, and mineralization bookkeeping end to end

Comments:
  [2026-03-26 18:12 UTC] reviewer: Dependency corrections:
- Removed C4 → C7 and C6 → C7 blockers. C7 (death routing) is stated as separate from the feeding/waste loop. C4 (feed/DOC audit) and C6 (grazing/maintenance conservation tests) can proceed independently — they test different pathways. C7 carries its own acceptance criteria with conservation tests for the death pathway.
- Removed C7 → B2 (concentration helpers). C7's deliverables are mass-based (dead_count × body_mass → detritus, body composition fractions). No concentration math needed.

C7 now depends only on C1 (routing conventions) and its Phase 1B parent. This keeps it as a parallel workstream alongside C2/C3 rather than injecting it into the critical path for feed/DOC auditing.
