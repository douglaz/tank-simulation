○ tanksim-6e5.3.3 · Implement microfauna matter routing and recycling   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: ecology, mass-balance, microfauna, phase-1, scientific-core

Context:
- Microfauna currently consume periphyton/detritus in a way that also deletes matter from the system (microfauna.rs:45-55). Periphyton consumed at line 48 and detritus at line 52-53 are subtracted from their pools but never returned as waste.
- Even if the microfauna model remains abstract, it still needs explicit routing to waste, biomass, and respiration-like demand.

Deliverable:
- Refactor microfauna feeding to follow the same closed-loop conventions defined in C1.
- Keep the abstraction lightweight while ensuring it does not silently erase load from the tank.
- Ensure the new loop interacts correctly with detritus and nutrient recycling.
- Reuse the routing pattern established in C2 (assimilation/feces/excretion/respiration fractions).

Likely touch points:
- crates/tank_core/src/systems/microfauna.rs (lines 45-55)
- any shared helper code created for C2
- crates/tank_core/src/types/process.rs (microfauna parameters at lines 119-123)

## Acceptance Criteria


Acceptance Criteria:
- microfauna consumption no longer acts as a hidden sink
- routing is consistent with the shrimp conventions where appropriate
- a unit test runs a closed-system tick with active microfauna and asserts total N and total C are conserved within tolerance (same pattern as C2d conservation test)
- a second test verifies that microfauna consumption produces the expected TAN increase and DO decrease proportional to ingestion rate

Dependencies:
  -> tanksim-6e5.3.2 (blocks) - Implement shrimp ingestion → assimilation → excretion → feces → respiration loop
  -> tanksim-6e5.3.1 (blocks) - Define closed-loop matter-routing conventions for consumers and maintenance actions
  -> tanksim-6e5.2.2 (blocks) - Implement canonical concentration and compartment helper APIs
  -> tanksim-6e5.3 (parent-child) - Phase 1B — mass conservation and husbandry action semantics

Dependents:
  <- tanksim-6e5.3.4 (blocks) - Audit feeding, detritus, DOC, and mineralization bookkeeping end to end

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- The microfauna model can stay simple, but it cannot keep breaking the nutrient loop if the simulator is supposed to teach ecosystem behavior.
- Reuse conventions and helpers from the shrimp work (C2) instead of inventing a second bookkeeping language.
- The goal is ecological credibility, not species-by-species realism.
- Microfauna consume less total mass than shrimp but their consumption still matters for the budget because they process periphyton and fine detritus continuously.

Implementation note:
- The microfauna system uses a population_index (0-1 scale) rather than discrete counts. Routing fractions should be proportional to consumption amount, just like shrimp.
- Microfauna excretion products go to the same pools as shrimp excretion: TAN for N waste, DIC for respired C, fine detritus for feces.
