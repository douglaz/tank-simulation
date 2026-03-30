○ tanksim-6e5.5.3 · Split periphyton and decomposer pools by habitat   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: decomposition, habitats, periphyton, phase-3, scientific-core

Context:
- Periphyton and decomposer biomass are currently too lumped to capture differences among glass film, plant-surface biofilm, filter-associated biofilm, and substrate-associated biofilm.
- A habitat split allows grazing, light exposure, and decomposition to become more ecologically legible.

Deliverable:
- Introduce habitat-specific pools or weights for periphyton and decomposer activity.
- Ensure the first pass is still computationally simple and does not explode the state space unnecessarily.
- Tie the new pools to habitats created in E1.

Likely touch points:
- crates/tank_core/src/systems/algae_growth.rs (periphyton section)
- crates/tank_core/src/systems/microfauna.rs (grazing targets)
- crates/tank_core/src/systems/shrimp.rs (grazing targets)
- habitat-bearing state types

## Acceptance Criteria


Acceptance Criteria:
At least the major habitats can differ in periphyton/decomposer abundance or productivity.
Shrimp and microfauna can graze in a habitat-aware way later.
Comments explain what is still lumped versus newly differentiated.
Unit test: glass habitat periphyton grows faster than filter-media periphyton under identical nutrient conditions (because glass gets light, filter media does not).
Unit test: decomposer activity in filter-media habitat exceeds glass-habitat decomposer activity (filter has higher flow/O2 exposure).
Integration test: a 200-hour scenario shows periphyton developing preferentially on lit surfaces (glass, substrate surface) while filter-internal biofilm is nitrifier/decomposer-dominated.
Conservation test: total periphyton + decomposer biomass across all habitats sums to the correct total; splitting by habitat does not create or destroy biomass.

Dependencies:
  -> tanksim-6e5.5.1 (blocks) - Introduce habitat registry and colonizable-area model
  -> tanksim-6e5.3.4 (blocks) - Audit feeding, detritus, DOC, and mineralization bookkeeping end to end
  -> tanksim-6e5.2.5 (blocks) - Normalize algae kinetics around concentration, light, and temperature interactions
  -> tanksim-6e5.5 (parent-child) - Phase 3 — habitatized ecology, substrate redox, and geometry-aware scaling

Dependents:
  <- tanksim-6e5.5.7 (blocks) - Add habitat/geometry scenario tests and probes
  <- tanksim-6e5.5.4 (blocks) - Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- This bead gives "microscope life" and biofilm maturity a better substrate without requiring species-level simulation.
- Keep the dimensionality low. Habitat differentiation should reveal mechanisms, not bury the sim under bookkeeping.
- The main question is explanatory power: can a player understand why one surface is fouling or maturing differently from another?

Ecological notes:
- Glass biofilm: primarily periphyton (algae) because glass gets direct light. This is what hobbyists scrape off during maintenance.
- Filter media biofilm: primarily nitrifiers and heterotrophic bacteria. Very little periphyton because it's dark inside the filter.
- Plant surface biofilm: mixed — some periphyton, some epiphytic bacteria. Healthy plants often resist heavy biofilm.
- Substrate surface: moderate periphyton + decomposers processing settled detritus.
- Substrate deep: no periphyton (no light), anaerobic/suboxic bacteria, denitrifiers.
  [2026-03-26 17:08 UTC] reviewer: Unit test requirements (complementing existing reviewer tests):

1. test_periphyton_split_by_habitat: After splitting, periphyton exists on GlassHardscape, SubstrateSurface, and PlantSurfaces habitats with independent biomass pools.
2. test_decomposer_split_by_habitat: Decomposers exist on FilterMedia, SubstrateSurface, and SubstrateDeep with independent pools.
3. test_habitat_growth_rate_differs: Periphyton on high-light habitat grows faster than periphyton on low-light habitat (same nutrients). FilterMedia periphyton → ≈ 0 growth (no light).
4. test_total_biomass_conserved_on_split: When splitting a single lumped pool into habitat pools, total biomass before = sum of habitat pools after.
5. test_grazing_targets_habitat_periphyton: Shrimp graze primarily accessible periphyton (glass, substrate surface) not filter media periphyton. Verify grazing pressure is habitat-weighted.
6. test_save_load_habitat_pools: Save and reload state with split pools. All per-habitat biomass values survive roundtrip.

Integration test:
7. test_habitat_diversity_supports_more_microbes: Tank with more diverse habitats (active substrate + filter media + plants) supports higher total microbial biomass than bare tank (glass only). Run 500 hours, assert total decomposer + nitrifier biomass is higher with more habitat diversity.
