○ tanksim-6e5.5.4.3 · Add root-zone oxygenation hooks for rooted plants   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: phase-3, plants, roots, scientific-core, substrate

Context:
- Rooted aquatic plants transport O2 from photosynthesis down through their roots into the substrate (radial oxygen loss, ROL). This creates micro-oxic zones around root tips in otherwise suboxic substrate.
- In real planted tanks, ROL is the primary mechanism that prevents hydrogen sulfide buildup in deep substrate, enables aerobic nitrification in root zones, and supports beneficial rhizosphere microbial communities.
- Without this hook, the simulator cannot distinguish "mature planted substrate" from "unplanted substrate" for mechanistic reasons — the redox gradient would be identical regardless of plant presence.
- The habitat registry (E1) already tracks plant biomass and substrate zones. The oxic/suboxic model (E4a) provides the penetration depth. This bead connects them.

Deliverable:
- Add a root-zone oxygenation modifier that increases o2_penetration_depth_cm proportionally to rooted plant biomass.
- The modifier should represent radial oxygen loss: more root biomass → more O2 injected into substrate → deeper effective oxic zone.
- Implement as a modifier to the penetration depth calculation from E4a:
  effective_penetration = base_penetration + root_oxygenation_bonus
  root_oxygenation_bonus = root_biomass_g × rol_rate_cm_per_g
  where rol_rate_cm_per_g is a named parameter representing the marginal O2 penetration per gram of root biomass.
- Only rooted plant guilds (root_rosette, etc.) contribute; floating or epiphytic plants do not.
- The effect should saturate at high root biomass (diminishing returns) — a simple sqrt or log modifier is acceptable.
- Expose the root oxygenation contribution in tracing/debug output so the mechanism is inspectable.

Key ecological consequence:
  Planted substrate with healthy root systems →
    deeper oxic zone →
    more aerobic nitrification in substrate surface →
    larger suboxic zone pushed deeper →
    more denitrification capacity in deep substrate →
    better overall N cycling

Likely touch points:
- crates/tank_core/src/systems/substrate.rs or wherever E4a computes penetration depth
- crates/tank_core/src/types/process.rs (rol_rate parameter)
- crates/tank_core/src/systems/plant_growth.rs (root biomass accessor)
- crates/tank_data/data/plant_guilds/*.toml (per-guild root contribution)

## Acceptance Criteria


Acceptance Criteria:
- rooted plant presence visibly increases substrate O2 penetration depth compared to an unplanted baseline
- the effect is proportional to root biomass rather than a binary on/off switch
- the effect saturates at high biomass so extreme root mass does not produce absurdly deep penetration
- only rooted plant guilds contribute; floating or epiphytic plants do not
- root oxygenation parameters are named parameters rather than hardcoded constants
- unit test: adding rooted plants increases penetration depth above the same unplanted baseline
- unit test: more root biomass increases penetration bonus, but with diminishing returns
- unit test: floating plant biomass does not increase penetration depth
- unit test: zero root biomass yields zero oxygenation bonus beyond base penetration
- integration test: otherwise identical tanks with and without rooted plants diverge in penetration depth, substrate-surface nitrification, and, if E4b is active, denitrification capacity over 500 hours
- tracing/debug output exposes the root oxygenation contribution separately from base diffusive penetration

Dependencies:
  -> tanksim-6e5.5.4.1 (blocks) - Add oxic/suboxic zone state to substrate model
  -> tanksim-6e5.5.4 (parent-child) - Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks
  -> tanksim-6e5.5.1 (blocks) - Introduce habitat registry and colonizable-area model

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Scientific background:
- Radial oxygen loss (ROL) from plant roots is well-documented in aquatic macrophytes. Roots release O2 into the surrounding sediment, creating an oxic rhizosphere within otherwise suboxic substrate.
- This has dual effects: (a) prevents toxic reduced compounds (H2S, Fe2+) from accumulating near roots, (b) enables nitrification in the rhizosphere (a micro-nitrogen-cycle around each root).
- For the simulator: parameterize as o2_release_per_g_root_per_hour and increase O2 penetration depth proportionally to root biomass.
- Keep it simple: root_oxidation_effect = plant.root_biomass_g × o2_release_rate. This widens the oxic zone, reducing the effective suboxic volume.
  [2026-03-26 16:23 UTC] reviewer: Unit test requirements added:

1. test_rooted_plants_increase_o2_penetration: Tank with rooted plants has deeper O2 penetration than same tank without plants.
2. test_root_oxygenation_proportional_to_biomass: More root biomass → deeper O2 penetration (or wider oxic rhizosphere).
3. test_root_oxygenation_reduces_suboxic_volume: Rooted plants reduce the effective suboxic zone, reducing denitrification capacity.
4. test_no_root_effect_for_water_column_plants: FastStem (water-column feeders) should NOT affect substrate O2 penetration — only RootRosette and similar rooted guilds.
5. test_root_oxygenation_dynamic: As rooted plant biomass grows, O2 penetration increases; as it declines (senescence/trimming), O2 penetration decreases.

  [2026-03-26 17:06 UTC] reviewer: Dependency added: E4.3 now depends on E1 (Introduce habitat registry and colonizable-area model).

Rationale: Root-zone oxygenation hooks need to query habitat types to know which habitats have rooted plants and what their substrate access properties are. Without the habitat registry, root-zone hooks would need to hardcode habitat knowledge.
  [2026-03-26 17:07 UTC] reviewer: Unit test requirements:

1. test_rooted_plants_oxygenate_substrate: Tank with RootFeedingRosette plants → oxic zone around roots is deeper than tank without rooted plants.
2. test_oxygenation_proportional_to_root_biomass: More root biomass → larger/deeper oxygenated zone (up to saturation).
3. test_no_rooted_plants_no_root_oxygenation: Tank with only FastStem (water column) plants or no plants → no root-zone oxygenation effect on substrate.
4. test_root_oxygenation_suppresses_denitrification_locally: In the root zone, oxygenation pushes the oxic/suboxic boundary deeper, reducing the suboxic zone available for denitrification. Verify denitrification rate decreases with more root oxygenation.
5. test_root_oxygenation_queries_habitat_registry: Verify that the root-zone system uses HabitatKind::SubstrateSurface/SubstrateDeep from the habitat registry to determine where roots are active.

Integration test:
6. test_planted_substrate_ecosystem: Tank with active planted substrate, rooted plants, and shrimp. Root oxygenation creates aerobic zones that support nitrification near roots while deeper suboxic zones allow denitrification. Run 500 hours, assert both processes are active. This is the classic planted tank nitrogen cycling story.
