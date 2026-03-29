○ tanksim-6e5.2.5 · Normalize algae kinetics around concentration, light, and temperature interactions   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: algae, ecology, kinetics, phase-1, scientific-core

Context:
- Algae growth is one of the main visible outcomes players care about, but it currently sits on top of simplified and partly size-coupled logic.
- algae_growth.rs:31-38 computes nutrient limitation that may use totals.
- The next version should make algae respond to light, nutrients, and temperature through clearer concentration-based rules.

Deliverable:
- Refactor algae limitation terms to use canonical concentration helpers.
- Preserve a simple model shape while making light/nutrient/temperature interactions explicit enough to tune.
- Prepare algae behavior to interact later with habitats, periphyton, and CO2 (Phases 2 and 3).

Likely touch points:
- crates/tank_core/src/systems/algae_growth.rs
- crates/tank_core/src/systems/light.rs
- crates/tank_core/src/types/process.rs (algae_half_saturation_n_mg_total)

## Acceptance Criteria


Acceptance Criteria:
- no nutrient limitation term depends directly on raw tank totals
- algae behavior remains playable/tunable after the refactor
- code comments explain what interactions are represented versus intentionally abstracted
- a tank-size-independence unit test (same pattern as B3e): two tanks with identical concentrations but different volumes produce identical algae limitation factors
- unit tests for nutrient × light × temperature interaction with known inputs and expected outputs

Dependencies:
  -> tanksim-6e5.2.2 (blocks) - Implement canonical concentration and compartment helper APIs
  -> tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  -> tanksim-6e5.2 (parent-child) - Phase 1A — units, concentration semantics, and kinetic normalization

Dependents:
  <- tanksim-6e5.5.5 (blocks) - Add depth/turbidity light attenuation and habitat-specific light exposure
  <- tanksim-6e5.5.3 (blocks) - Split periphyton and decomposer pools by habitat
  <- tanksim-6e5.4.4 (blocks) - Connect photosynthesis and respiration to DIC / CO2 and day–night pH behavior
  <- tanksim-6e5.2.6 (blocks) - Retune process parameters and scenario defaults after normalization

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- Algae blooms are a core emergent outcome for this project, so the growth model must at least be mechanistically sane.
- Avoid premature overfitting. A well-explained low-dimensional model is preferable to a more complex one with weak parameter grounding.
- This bead sets up later habitatized periphyton work (E3) without requiring it immediately.

Scientific notes:
- Algae growth in aquaria is primarily limited by: light availability, dissolved N (usually as NH4+ or NO3), dissolved P, and temperature.
- The current model likely over-indexes on N limitation because P cycling is not closed. This is acceptable for v0.2 but should be noted in the algae system comments.
- Periphyton (attached algae) and planktonic algae have different light and nutrient strategies. The current model lumps them. Phase 3 (E3) will split them.
