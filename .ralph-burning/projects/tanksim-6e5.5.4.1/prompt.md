○ tanksim-6e5.5.4.1 · Add oxic/suboxic zone state to substrate model   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: phase-3, redox, scientific-core, state, substrate

Context:
- The current substrate model (substrate.rs) represents layers by kind (inert sand, active soil, etc.) with bulk properties like porosity and CEC, but does not distinguish oxygenated surface zones from deeper low-oxygen zones.
- Real aquarium substrates develop a vertical redox gradient: the top 1-3cm is aerobic (O2 diffusion from water column), while deeper layers become suboxic or anaerobic depending on biological oxygen demand and substrate type.
- The habitat registry (E1) defines SubstrateSurface and SubstrateDeep as separate habitats, but the substrate state model does not yet carry the zone boundary or per-zone chemistry needed to make those habitats meaningful.
- This bead adds the state representation; the actual denitrification process (E4b) and root-zone hooks (E4c) build on top of it.

Deliverable:
- Add o2_penetration_depth_cm as a dynamic field on SubstrateLayerState, computed each tick from biological O2 demand in the substrate and diffusion rate from the water column.
- Define the oxic zone as substrate above o2_penetration_depth and the suboxic zone as substrate below it.
- Map the oxic zone to the SubstrateSurface habitat and the suboxic zone to the SubstrateDeep habitat from E1, so colonizable area and volume for each zone are derived from the penetration depth and substrate geometry.
- Add per-zone accessors for volume, porosity-adjusted pore volume, and nutrient availability so downstream systems (denitrification, root oxygenation) have a canonical API.
- Keep the diffusion model simple: O2 penetration depends on water-column DO concentration, substrate porosity, and total biological O2 demand in the substrate (from decomposers, roots, microfauna). A steady-state diffusion approximation is sufficient for the first pass.

Penetration depth model (first pass):
  o2_penetration_cm = sqrt(2 × D_eff × [O2]_water / R_total)
  where D_eff = free-water O2 diffusivity × porosity^2 (tortuosity correction)
  R_total = sum of all biological O2 demand per cm³ in the substrate
  This is the classic Bouldin (1968) one-dimensional steady-state model.

Likely touch points:
- crates/tank_core/src/types/substrate.rs (SubstrateLayerState: new fields)
- crates/tank_core/src/types/state.rs (zone accessors)
- crates/tank_core/src/systems/substrate.rs or chemistry.rs (penetration depth calculation)
- habitat registry from E1 (link SubstrateSurface/SubstrateDeep to computed zones)

## Acceptance Criteria


Acceptance Criteria:
- SubstrateLayerState carries o2_penetration_depth_cm as a dynamic field updated each tick
- penetration depth is computed from water-column DO, substrate porosity, and biological O2 demand rather than a hardcoded depth
- the oxic zone volume and suboxic zone volume are derivable from penetration depth plus substrate geometry
- SubstrateSurface and SubstrateDeep habitats from E1 map to the oxic and suboxic zones respectively, with colonizable areas consistent with the computed boundary
- unit test: high water-column DO plus low biological demand yields deep penetration (> 3 cm in porous substrate)
- unit test: low water-column DO plus high biological demand yields shallow penetration (< 1 cm)
- unit test: zero biological demand yields full-depth penetration (entire substrate oxic)
- unit test: zone volumes sum to total substrate pore volume
- unit test: changing porosity changes penetration depth in the expected direction
- save/load round-trips the new field, and old saves without it load with a sensible computed default

Dependencies:
  -> tanksim-6e5.5.4 (parent-child) - Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks

Dependents:
  <- tanksim-6e5.5.4.3 (blocks) - Add root-zone oxygenation hooks for rooted plants
  <- tanksim-6e5.5.4.2 (blocks) - Implement simplified denitrification in suboxic substrate zones

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Simplified model for O2 penetration depth:
- O2 penetration ≈ sqrt(2 × D_O2 × [O2_surface] / R_consumption)
  where D_O2 is diffusion coefficient in sediment (~1e-5 cm²/s), [O2_surface] is O2 concentration at substrate surface, and R_consumption is volumetric O2 consumption rate by bacteria.
- For a first pass: estimate penetration depth as 2-5 mm, modified by biological load and substrate porosity.
- The key behavior: heavily loaded tanks → shallower O2 penetration → larger suboxic zone → more denitrification.
  [2026-03-26 16:23 UTC] reviewer: Unit test requirements added:

1. test_o2_penetration_depth_decreases_with_bioload: Higher biological O2 demand → shallower O2 penetration depth.
2. test_suboxic_zone_exists_when_substrate_deep: With substrate_depth > o2_penetration, suboxic zone volume > 0.
3. test_no_suboxic_zone_shallow_substrate: Very thin substrate (1mm) → no meaningful suboxic zone.
4. test_zone_state_integrates_with_habitat_registry: SubstrateSurface habitat corresponds to oxic zone; SubstrateDeep corresponds to suboxic zone.
5. test_zone_state_serialization: New zone state fields serialize/deserialize correctly (save migration if needed).

  [2026-03-26 17:06 UTC] reviewer: Unit test requirements:

1. test_oxic_zone_defaults: New substrate layer has an oxic zone with positive O2 tendency and a suboxic zone with near-zero O2.
2. test_zone_state_serialization_roundtrip: Save and reload a state with oxic/suboxic zones. Verify all zone fields survive roundtrip.
3. test_zone_depths_sum_to_substrate_depth: oxic_depth + suboxic_depth = total substrate depth (within tolerance).
4. test_zone_state_part_of_budget_helpers: total_nitrogen() and total_carbon() helpers from A2a include any N/C stored in substrate zones.
5. test_initial_zone_proportions: Default substrate types (inert_sand, active_planted) have reasonable initial oxic/suboxic depth ratios (e.g., fine sand: thinner oxic zone; coarse gravel: deeper oxic zone).
