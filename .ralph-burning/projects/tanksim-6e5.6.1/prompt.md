○ tanksim-6e5.6.1 · Define shrimp state model for reserve, condition, stage, molt, and reproduction   [● P1 · OPEN]
Owner: master · Type: spike
Created: 2026-03-26 · Updated: 2026-03-27
Labels: biology, design, phase-4, scientific-core, shrimp

Context:
- The current shrimp model already has useful population and reproduction logic, but the next step needs a clearer internal state structure.
- Before adding stages, minerals, and toxicity modifiers, the project should decide what shrimp-level state is explicit versus derived.

Deliverable:
- A design note for the shrimp state model covering:
  - Reserve/condition: food reserve, body condition index
  - Life stage: juvenile, sub-adult, adult (minimum 3 stages)
  - Molt state: inter-molt interval, readiness, last molt success
  - Reproduction: egg carrying state, clutch size, incubation progress
- Clear boundaries between population-level state and individual-level abstraction.
- Notes on what remains intentionally omitted in the first pass (e.g., sex ratio, individual tracking).

Likely touch points:
- crates/tank_core/src/types/biology.rs (AnimalState redesign)
- crates/tank_core/src/systems/shrimp.rs
- shrimp data files

## Acceptance Criteria


Acceptance Criteria:
- later shrimp implementation beads can proceed without reworking the model contract
- the state model is detailed enough for the planned mechanics but still tractable for tuning
- comments/docs explain why a given level of aggregation was chosen

Dependencies:
  -> tanksim-6e5.3.2 (blocks) - Implement shrimp ingestion → assimilation → excretion → feces → respiration loop
  -> tanksim-6e5.6 (parent-child) - Phase 4 — shrimp life history, toxicity, and reproduction realism

Dependents:
  <- tanksim-6e5.6.2 (blocks) - Implement stage- or size-structured shrimp population dynamics

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Rationale:
- This bead protects the project from sliding into either over-simplified "population only" logic or an unnecessarily expensive individual-based model.
- Keep the state choices closely tied to the behaviors the sim wants to express: molt success, breeding, juvenile survival, and chemistry-related stress.
- The best design is the one that supports explanation and calibration, not maximum detail for its own sake.

Recommended approach: stage-structured cohort model
- Track counts per stage: juveniles, sub-adults, adults (optionally: berried females).
- Track aggregate metrics per stage: average condition, average reserve.
- Individuals are not tracked — this is a population model with stage structure.
- This is the same level of abstraction used in many fisheries models (Leslie matrix / stage-structured matrix model) and is well-suited to hourly discrete-time simulation.
  [2026-03-26 16:22 UTC] reviewer: Dependency fix: removed blocks-dependency on E3 (periphyton habitat split).

Rationale: F1 is a DESIGN spike — it decides the shrimp state model contract. It needs to KNOW that habitats will exist (from the E1 design), but it does NOT need the E3 implementation (periphyton/decomposer pool splitting) to be complete before the shrimp state model can be designed.

The implementation beads under F2 may need habitat-aware food sources, but that's an F2-level concern, not an F1-level one. F1 can design the grazing-by-habitat interface without E3 being done.

  [2026-03-26 16:40 UTC] reviewer: Clarification on habitat coupling:
- F1 is intentionally habitat-agnostic in the executable graph after removing the old E3 blocker. That is fine.
- The design note may mention future habitat-aware grazing or surface-selection hooks, but it should treat those as extension points, not as prerequisites for the state-model spike.
- In other words: F1 should produce a shrimp state contract that can later integrate with habitats, without blocking on any Phase 3 habitat bead.

  [2026-03-26 16:52 UTC] master: Dependency refinement: removed the D5 source-water-profile blocker. F1 is a design spike for the shrimp state contract; it should define chemistry/mineral extension points without waiting for differentiated source-water defaults to be implemented.
