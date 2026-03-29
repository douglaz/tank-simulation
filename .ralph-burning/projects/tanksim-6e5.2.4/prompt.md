○ tanksim-6e5.2.4 · Normalize plant nutrient uptake and growth limitation semantics   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: ecology, kinetics, phase-1, plants, scientific-core

Context:
- Plant growth currently depends on nutrient and light logic that is directionally useful but not yet normalized through explicit concentration semantics.
- plant_growth.rs computes nutrient limitation using available nitrogen pools but the formulation may use totals in some paths.
- Rooted plants will also need a cleaner separation between water-column and substrate access in later phases (E4).

Deliverable:
- Refactor plant nutrient limitation terms to use concentration-based helpers and explicit semantics for nutrient source (water column vs substrate).
- Keep the model simple enough for tuning now, while leaving hooks for later root-zone work.
- Update parameter names where they currently imply total-mass behavior (e.g., plant_half_saturation_n_mg_total in process.rs).

Likely touch points:
- crates/tank_core/src/systems/plant_growth.rs
- crates/tank_core/src/systems/light.rs
- crates/tank_core/src/types/process.rs (plant_half_saturation_n_mg_total and related)
- crates/tank_data/data/plants/*.toml

## Acceptance Criteria


Acceptance Criteria:
- plant limitation terms are concentration-based and no longer implicitly depend on tank size
- rooted and water-column uptake assumptions are explicit in code comments or docs
- tests or scenario notes cover at least one plant-heavy case after normalization
- a tank-size-independence unit test (same pattern as B3e): two tanks with identical nutrient concentrations but 10× different volumes produce identical plant limitation factors within floating-point tolerance
- unit tests for each refactored limitation term with known inputs and expected outputs

Dependencies:
  -> tanksim-6e5.2.2 (blocks) - Implement canonical concentration and compartment helper APIs
  -> tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  -> tanksim-6e5.2 (parent-child) - Phase 1A — units, concentration semantics, and kinetic normalization

Dependents:
  <- tanksim-6e5.5.5 (blocks) - Add depth/turbidity light attenuation and habitat-specific light exposure
  <- tanksim-6e5.5.4 (blocks) - Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks
  <- tanksim-6e5.4.4 (blocks) - Connect photosynthesis and respiration to DIC / CO2 and day–night pH behavior
  <- tanksim-6e5.2.6 (blocks) - Retune process parameters and scenario defaults after normalization

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Future-self notes:
- Keep the first pass modest. The immediate goal is correct semantics, not a full plant physiology simulator.
- Leave room for later habitat/redox work (E4) by making nutrient-source assumptions explicit now. For example: "rooted plants access N from both water column and substrate porewater" should be a commented assumption that can later be replaced with actual habitat-specific queries.
- Retuning may matter more than model complexity at this stage.
- The plant_half_saturation_n_mg_total parameter name tells you it's a total — rename to plant_half_saturation_n_mg_n_per_l or similar.

Scientific notes:
- Rooted aquarium plants (e.g., Echinodorus, Cryptocoryne) obtain nutrients from both root uptake (substrate) and foliar uptake (water column). The relative importance depends on species and substrate nutrient availability.
- Water-column feeders (e.g., Rotala, floating plants) depend entirely on dissolved nutrients.
- The current model treats both plant types similarly. Phase 3 (E4) will add substrate-zone differentiation.
