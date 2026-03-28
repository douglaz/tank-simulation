○ tanksim-6e5.3.1 · Define closed-loop matter-routing conventions for consumers and maintenance actions   [● P0 · OPEN]
Owner: master · Type: spike
Created: 2026-03-26 · Updated: 2026-03-27
Labels: actions, design, mass-balance, phase-1, scientific-core

Context:
- Shrimp, microfauna, feeding, decomposition, and plant trimming all move matter around the system, but the current code does not consistently say where consumed or removed material goes.
- Before implementing fixes, the model needs one explicit routing policy for assimilation, excretion, feces, respiration, detritus, and exported biomass.

Deliverable:
- A short design note or code comment standard describing how C/N/energy-like quantities move through consumer and maintenance actions.
- Clear rules for what stays in-tank, what becomes waste, and what counts as export.
- A documented stance on where the model remains intentionally lumped/abstract.

Routing policy to define (at minimum):
  Ingestion → assimilation efficiency (fraction retained as body mass/reserve)
  Ingestion → fecal fraction (→ fine detritus)
  Assimilation → respiration fraction (→ O2 demand + DIC production)
  Assimilation → excretion fraction (→ TAN, dissolved N)
  Plant trimming → export (removed from system) vs leave-cuttings (→ fine detritus)
  Feed input → uneaten fraction (→ DOC/DON leaching → fine detritus)

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs
- crates/tank_core/src/systems/microfauna.rs
- crates/tank_core/src/engine.rs
- crates/tank_core/src/systems/nitrogen_cycle.rs

## Acceptance Criteria


Acceptance Criteria:
- future implementation beads can point to one agreed routing convention
- trim, feed, graze, and decay all have explicit intended outcomes
- the design is simple enough to test and tune

Dependencies:
  -> tanksim-6e5.1.1 (blocks) - Inventory current scientific semantics, units, invariants, and shortcuts
  -> tanksim-6e5.3 (parent-child) - Phase 1B — mass conservation and husbandry action semantics

Dependents:
  <- tanksim-6e5.3.7 (blocks) - Implement organism death → detritus routing for shrimp, plants, and algae
  <- tanksim-6e5.3.2 (blocks) - Implement shrimp ingestion → assimilation → excretion → feces → respiration loop
  <- tanksim-6e5.3.5 (blocks) - Split plant trimming into export vs leave-cuttings actions
  <- tanksim-6e5.3.3 (blocks) - Implement microfauna matter routing and recycling

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- This bead prevents later implementation from becoming a patchwork of one-off waste terms.
- Make the routing policy explicit enough that later shrimp and carbonate work can depend on it.
- It is okay to use lumped pools as long as the bookkeeping is intentional and documented.

Scientific reference points:
- For freshwater invertebrate grazers, typical assimilation efficiency ≈ 40-70% of ingested organic matter.
- Fecal production ≈ 30-60% of ingestion (inverse of assimilation efficiency).
- Of assimilated material: ~60-80% goes to respiration (energy), ~10-20% to growth/reserve, ~10-20% excreted as dissolved waste (TAN, urea, DON).
- These are order-of-magnitude guidelines, not precise values. The routing conventions should use named parameters so they can be tuned later.
  [2026-03-26 16:24 UTC] reviewer: Missing routing path identified: organism death → detritus.

The current shrimp.rs mortality logic (lines 440-480) decrements population counts when shrimp die, but does NOT return the dead organism's body mass to the detritus pool. This is a conservation gap — dead shrimp biomass currently vanishes.

The routing conventions defined in C1 should explicitly cover:
  Death pathway:
    Shrimp death → body mass enters fine_detritus_g_total (with appropriate C:N composition)
    Plant senescence → senescent biomass enters fine_detritus_g_total (already partially handled in plant_growth.rs but should be verified)
    Algae death/senescence → same routing

This should be a named routing fraction: death_biomass_to_detritus_fraction = 1.0 (all dead biomass becomes detritus, unless explicitly exported like "remove dead shrimp" action).

C2d (shrimp respiration + conservation test) and C4 (bookkeeping audit) should both verify this pathway. The C2 implementation should add the death → detritus routing alongside the feeding → waste routing.

