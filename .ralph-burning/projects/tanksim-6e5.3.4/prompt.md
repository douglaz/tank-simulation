○ tanksim-6e5.3.4 · Audit feeding, detritus, DOC, and mineralization bookkeeping end to end   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: detritus, mass-balance, nitrogen, phase-1, scientific-core

Context:
- Fixing consumer loops (C2, C3) is not enough if the broader feed -> waste -> detritus -> DOC/mineralization pathway still has silent sinks or double-counted steps.
- C7 now owns organism death / senescence routing into detritus. This audit bead must verify that those episodic inputs join the same downstream bookkeeping cleanly instead of bypassing or duplicating decomposition logic.
- This bead reconciles the full short-loop bookkeeping around feeding and decomposition.

Deliverable:
- Trace how feed inputs, uneaten food, feces, organism-death / senescence inputs, fine detritus, DOC, mineralization, and nitrification connect.
- Fix ownership boundaries so each transfer is represented once and in the right place.
- Update comments/docs around the detritus and DOC model, including the interface from C7 death/senescence routing into the same downstream pools.
- Produce a documented mass-flow map that future developers can reference.

Key pathway to trace:
  Feed (player action) -> uneaten fraction -> DOC/DON leaching -> fine detritus
                        -> consumed by shrimp/microfauna -> assimilation + feces + excretion + respiration
  Death/senescence (C7) -> fine detritus / decay inputs -> same DOC + mineralization pathway
  Fine detritus -> decomposer mineralization -> DOC -> DIC/TAN (via decomposition)
  Fine detritus -> dissolution -> DOC/DON
  Decomposition -> O2 demand + DIC + TAN

Likely touch points:
- crates/tank_core/src/systems/nitrogen_cycle.rs (decomposer section)
- crates/tank_core/src/systems/shrimp.rs (feeding section)
- crates/tank_core/src/systems/microfauna.rs
- crates/tank_core/src/systems/plant_growth.rs
- crates/tank_core/src/systems/algae_growth.rs
- crates/tank_core/src/engine.rs (Feed action handler)
- crates/tank_core/src/types/process.rs (leaching rates)

## Acceptance Criteria


Acceptance Criteria:
A feed pulse and a death/senescence-derived detritus input can both be followed conceptually through the major pools.
Hidden sinks and double-counts are removed, or documented explicitly if a simplification still lumps them.
A documented mass-flow map traces feed and death/senescence inputs through detritus -> DOC -> decomposition -> DIC/TAN for both N and C.
An integration test feeds a tank with known food mass and runs 100+ hours, then verifies total N in all tracked pools (water + biomass + detritus + DOC/DON) is conserved within tolerance (< 1e-6 mg) apart from named exports.
A second integration test verifies the DOC pathway: feed leaching rate x time ~= DOC produced, decomposer consumption ~= DOC consumed, and the difference matches DOC pool change.
The audit explicitly verifies that C7 death/senescence routing enters the same downstream bookkeeping as feed-derived detritus without double-counting mineralization or DOC production.
Tests use budget tracking (A2a) and tracing (A2d) to log per-system contributions; failures dump the full flow map for diagnosis.

Dependencies:
  -> tanksim-6e5.3.7 (blocks) - Implement organism death → detritus routing for shrimp, plants, and algae
  -> tanksim-6e5.3.2 (blocks) - Implement shrimp ingestion → assimilation → excretion → feces → respiration loop
  -> tanksim-6e5.2.3 (blocks) - Normalize nitrogen-cycle kinetics to concentration-based terms
  -> tanksim-6e5.1.2.4 (blocks) - Add structured simulation tracing with per-system deltas and configurable verbosity
  -> tanksim-6e5.1.2.1 (blocks) - Implement per-tick mass budget tracking for N, C, and O2
  -> tanksim-6e5.3 (parent-child) - Phase 1B — mass conservation and husbandry action semantics
  -> tanksim-6e5.3.3 (blocks) - Implement microfauna matter routing and recycling

Dependents:
  <- tanksim-6e5.5.3 (blocks) - Split periphyton and decomposer pools by habitat
  <- tanksim-6e5.4.4 (blocks) - Connect photosynthesis and respiration to DIC / CO2 and day–night pH behavior
  <- tanksim-6e5.3.6 (blocks) - Add conservation diagnostics and regression tests for grazing and maintenance loops

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Future-self notes:
- This is where simplifications should become explicit. It is acceptable to lump some dissolved organics or waste classes as long as the path is coherent.
- Audit with the instrumentation from A2 turned on: feed the tank, run the budget tracker, and see if total N/C stays constant.
- Treat the result as a mass-flow map that later developers can reason about quickly.
- Pay special attention to the DOC pathway: feed leaching (process.rs:28 feed_leach_rate_per_hour) and detritus dissolution (process.rs:30 fine_detritus_dissolution_rate_per_hour) both produce DOC. Decomposers then consume DOC. Make sure these three processes are consistent.
