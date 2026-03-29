○ tanksim-6e5.5.5 · Add depth/turbidity light attenuation and habitat-specific light exposure   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: algae, geometry, light, phase-3, plants, scientific-core

Context:
- Tank geometry already includes depth (fill_height_cm in geometry.rs), but the light model does not yet turn depth and turbidity into different average exposure conditions.
- Depth-sensitive light is a key mechanism for distinguishing shallow and deep planted tanks under the same nominal lamp setting.

Deliverable:
- Extend light handling so depth and turbidity influence light available to plants/algae/habitats.
- Integrate the result with the habitat model where useful.
- Keep the first pass simple enough that users and developers can reason about it.

Likely model:
  I(z) = I_0 × exp(-k × z)  (Beer-Lambert law)
  where k = extinction coefficient (depends on turbidity, dissolved color, algae density)
  z = depth below surface

Likely touch points:
- crates/tank_core/src/systems/light.rs
- crates/tank_core/src/systems/plant_growth.rs
- crates/tank_core/src/systems/algae_growth.rs
- crates/tank_core/src/types/geometry.rs

## Acceptance Criteria


Acceptance Criteria:
- deeper or more turbid tanks receive meaningfully different effective light
- plant/algae systems can consume that difference without ad hoc hacks
- unit test: Beer-Lambert computation correctness — I(z) = I_0 × exp(-k×z) for k=0.2, z=20cm → expected attenuation factor ≈ 0.018 (within 0.001)
- unit test: average light across water column matches analytical formula I_avg = I_0 × (1 - exp(-k×H)) / (k×H)
- integration test: shallow tank (20cm) vs deep tank (50cm) with same lamp → shallow tank shows measurably higher plant growth rate over 200 hours

Dependencies:
  -> tanksim-6e5.5.1 (blocks) - Introduce habitat registry and colonizable-area model
  -> tanksim-6e5.2.5 (blocks) - Normalize algae kinetics around concentration, light, and temperature interactions
  -> tanksim-6e5.2.4 (blocks) - Normalize plant nutrient uptake and growth limitation semantics
  -> tanksim-6e5.5 (parent-child) - Phase 3 — habitatized ecology, substrate redox, and geometry-aware scaling

Dependents:
  <- tanksim-6e5.5.7 (blocks) - Add habitat/geometry scenario tests and probes

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- This bead turns geometry into a scientifically relevant control on growth instead of only a thermal or volumetric one.
- Avoid overcomplication: one clear attenuation story (Beer-Lambert) is better than several overlapping fudge factors.
- Make sure the result remains explainable in the TUI later.

Scientific notes:
- Typical extinction coefficient for clear freshwater: k ≈ 0.1-0.3 per cm. For turbid or tannin-stained water: k ≈ 0.5-2.0 per cm.
- In a 20cm deep nano tank with k=0.2: light at bottom = I_0 × exp(-0.2×20) ≈ I_0 × 0.018 (1.8% of surface light). This is significant.
- In a 50cm deep tank: light at bottom ≈ I_0 × 0.00005 — essentially dark. This is why tall tanks need strong lighting.
- Average light across the water column (more relevant for planktonic algae): I_avg = I_0 × (1 - exp(-k×H)) / (k×H).
  [2026-03-26 17:06 UTC] reviewer: Unit test requirements:

1. test_light_attenuation_increases_with_depth: At depth 30cm vs 60cm, light reaching substrate surface is measurably lower (Beer-Lambert style attenuation).
2. test_light_attenuation_increases_with_turbidity: Same depth, higher turbidity (from suspended algae or DOC) → less light reaches substrate.
3. test_zero_turbidity_depth_only_attenuation: Clear water attenuates light by depth alone. Verify against known freshwater Kd values (pure water Kd ≈ 0.04/m at PAR wavelengths).
4. test_habitat_light_exposure_varies: FilterMedia habitat → light_exposure ≈ 0 (inside canister). SubstrateSurface → attenuated by depth. GlassHardscape at water surface → higher than substrate.
5. test_algae_growth_responds_to_depth: Deeper tank → less light → lower algae growth rate. Run same nutrients in 20cm vs 50cm tank, assert algae biomass difference after 100 hours.
6. test_plant_growth_responds_to_light_attenuation: High turbidity from algae bloom → reduced plant growth (light competition feedback).

Integration test:
7. test_depth_light_competition_scenario: 50cm deep tank with substrate plants + suspended algae. Algae bloom shades substrate → plant health declines → plant nutrient uptake drops → more nutrients for algae (positive feedback). Assert this feedback loop is directionally present.
