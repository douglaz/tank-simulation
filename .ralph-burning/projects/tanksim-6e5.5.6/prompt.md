○ tanksim-6e5.5.6 · Scale equipment, plant mass, and stocking defaults with tank geometry   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: geometry, hardware, phase-3, scenarios, scientific-core

Context:
- Geometry scaling (tank_scenarios/src/lib.rs) already changes some properties, but startup hardware and biomass defaults still remain partly fixed.
- Filter flow stays at ~200 lph if enabled (lib.rs:581-589), plant biomass stays fixed at ~5g per guild (lib.rs:687-699), heater power is not geometry-aware.

Deliverable:
- Revisit scenario/preset logic so filter flow, heater power, initial plant biomass, and stocking defaults scale coherently with geometry or are explicitly overridden.
- Separate conservative startup defaults from maximum sustainable colony density so geometry scaling does not silently turn normal scenarios into overstock stress tests.
- Document which defaults scale automatically versus which remain scenario-authored choices.
- Recheck ambient-temperature behavior after the changes.

Scaling rules to implement:
  filter_flow_lph: scale with volume (e.g., 10× volume per hour turnover)
  heater_power_w: scale with volume and exposed surface area (W/L guideline)
  initial_plant_biomass_g: scale with substrate footprint (g/cm²)
  initial_shrimp_count: preserve explicit scenario-authored startup counts by default; optional auto-stocking mode may scale a conservative startup density with volume, but should not silently jump to colony-capacity stocking

Likely touch points:
- crates/tank_scenarios/src/lib.rs
- crates/tank_core/src/types/hardware.rs
- scenario data files and related builders

## Acceptance Criteria


Acceptance Criteria:
Geometry-scaled scenarios no longer inherit obviously mismatched default hardware/biomass.
Scaling rules are written down as named constants or documented formulas.
Tests can distinguish deliberate overrides from automatic scaling behavior.
Unit test: a 100L tank auto-scales to filter_flow ~1000 lph, heater ~75W, and plant_biomass ~15g (proportional to footprint); explicit scenario overrides still win when present.
Unit test: a 10L nano tank auto-scales to proportionally smaller hardware and plant biomass.
Unit test: explicit scenario-authored shrimp startup counts are preserved when present; geometry scaling does not silently rewrite authored stocking.
Unit test: if an auto-stocking mode is enabled, it uses a conservative startup density calibrated to current happy-path scenarios (roughly 0.15-0.25 adults/L, so 100L yields ~15-25 adults and 10L yields ~1-3 adults), not colony-capacity densities or overstock envelopes.
Integration test: a 1× vs 2× geometry comparison where hardware, plants, and a non-overstocked shrimp baseline all scale proportionally shows per-liter concentrations stay within 20% of each other over 500 hours (cycling timeline, TAN peak, DO range).
Integration test: an intentionally mismatched scenario (large tank, tiny filter) shows worse cycling performance than the auto-scaled version — proving that scaling rules prevent this mismatch.


Dependencies:
  -> tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  -> tanksim-6e5.5.1 (blocks) - Introduce habitat registry and colonizable-area model
  -> tanksim-6e5.2.6 (blocks) - Retune process parameters and scenario defaults after normalization
  -> tanksim-6e5.5 (parent-child) - Phase 3 — habitatized ecology, substrate redox, and geometry-aware scaling

Dependents:
  <- tanksim-6e5.5.7 (blocks) - Add habitat/geometry scenario tests and probes

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Future-self notes:
- This bead is about making comparisons fair. When a bigger tank behaves differently, it should be because the model says it should, not because startup defaults quietly stayed nano-sized.
- Keep scenario authorship flexible: auto-scaling should help, not remove the ability to create intentionally unusual setups (e.g., overstocked nano, understocked large tank).
- Revisit ambient-temperature edge cases because thermal behavior depends on both geometry (volume, surface area) and hardware (heater power).

Common aquarium sizing guidelines:
- Filter: 5-10× tank volume per hour flow rate
- Heater: 0.5-1.0 W per liter (depends on ambient-to-target temperature delta)
- Plants: 5-15g/1000cm² of substrate footprint for a "moderately planted" setup
- Shrimp: 2-5 shrimp per liter for Neocaridina (varies with filtration and plant density)
  [2026-03-26 15:51 UTC] reviewer: Clarification on stocking semantics:
- The 2-5 shrimp/L guideline in the earlier note is closer to a mature colony-density or upper husbandry range under strong filtration and plant cover, not a sensible startup default for geometry-scaled scenarios.
- Keep startup stocking conservative or explicitly scenario-authored. If the project wants to explore high-density shrimp colonies, model those as intentional stress or husbandry scenarios rather than hidden defaults.

  [2026-03-26 17:09 UTC] reviewer: Unit test requirements (complementing existing reviewer tests):

1. test_stocking_scales_with_volume: Default shrimp count for a 20L tank vs 60L tank → larger tank gets proportionally more shrimp (not identical counts).
2. test_plant_mass_scales_with_footprint: Default plant biomass scales with tank footprint (length × width) not volume. Verify 2× footprint → ~2× initial plant biomass.
3. test_filter_capacity_scales_with_volume: Default filter media amount scales with tank volume. Larger tank → larger filter.
4. test_light_intensity_independent_of_tank_size: Light intensity (PAR at water surface) is a property of the light fixture, not the tank. Verify it doesn't scale with volume.
5. test_heater_wattage_scales_with_volume: Heater wattage needed scales with water volume (thermal mass). Verify heater output recommendation scales.
6. test_substrate_depth_independent_of_tank_size: Default substrate depth (cm) is the same regardless of tank footprint. Total substrate volume scales with footprint.
7. test_aeration_scales_with_surface_area: Aeration effect scales with surface area and volume, not just a fixed rate.

Integration test (additional to existing reviewer 1x-vs-2x test):
8. test_nano_vs_standard_vs_large_scenarios: Run 3 tank sizes (20L, 60L, 200L) with proportionally scaled equipment/stocking. All three should cycle in similar timeframes and reach similar steady-state concentrations. Assert cycling_completion_hours are within 20% of each other. This is the key scaling validation.
