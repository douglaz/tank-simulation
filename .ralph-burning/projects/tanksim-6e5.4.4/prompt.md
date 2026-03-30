○ tanksim-6e5.4.4 · Connect photosynthesis and respiration to DIC / CO2 and day–night pH behavior   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: carbonates, day-night, phase-2, plants, scientific-core

Context:
- Current defaults leave important DIC-related rates at or near zero (process.rs:135-136: respiration_dic_rate_mg_c_per_g_per_hour = 0.0, photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0).
- This means day/night chemistry behavior cannot emerge meaningfully.
- Planted tanks should show at least a simplified mechanistic relationship among photosynthesis, respiration, CO2 availability, and pH drift.

Deliverable:
- Wire plant/algae photosynthesis and biological respiration into the new DIC / CO2 state.
- Revisit defaults so the shipped scenarios actually exercise the new pathway.
- Make the day/night chemistry story visible in tests and/or snapshots.

Expected behavior:
  During light: plants consume CO2 (↓DIC) → pH rises
  During dark: respiration produces CO2 (↑DIC) → pH drops
  Net effect: diurnal pH swing of 0.2-1.0 pH units in heavily planted tanks

Likely touch points:
- crates/tank_core/src/systems/plant_growth.rs (add DIC consumption)
- crates/tank_core/src/systems/algae_growth.rs (add DIC consumption)
- crates/tank_core/src/systems/chemistry.rs (respiration DIC production)
- crates/tank_core/src/types/process.rs (set non-zero defaults)

## Acceptance Criteria


Acceptance Criteria:
- light cycle changes can affect DIC / CO2 / pH in a believable direction
- defaults are no longer effectively disabling the pathway
- interactions remain simple enough to tune
- unit test: photosynthesis DIC consumption is stoichiometrically consistent with O2 production (12g C consumed per 32g O2 produced, within 1% tolerance)
- integration test (day/night probe): run a heavily planted scenario for 48 hours with 12h/12h photoperiod → assert pH at end-of-light > pH at end-of-dark by at least 0.1 units
- integration test: same scenario with lights always off → no diurnal pH swing (DIC only increases from respiration)

Dependencies:
  -> tanksim-6e5.4.2 (blocks) - Implement explicit carbonate equilibrium and pH solver
  -> tanksim-6e5.4 (parent-child) - Phase 2 — carbonate chemistry, CO2 exchange, and pH realism
  -> tanksim-6e5.3.4 (blocks) - Audit feeding, detritus, DOC, and mineralization bookkeeping end to end
  -> tanksim-6e5.2.5 (blocks) - Normalize algae kinetics around concentration, light, and temperature interactions
  -> tanksim-6e5.2.4 (blocks) - Normalize plant nutrient uptake and growth limitation semantics

Dependents:
  <- tanksim-6e5.4.6 (blocks) - Add carbonate regression tests and scenario probes

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Future-self notes:
- This bead is about coupling, not about turning the simulator into a detailed photosynthesis model.
- Prefer a transparent day/night effect that can be explained to players and developers alike.
- Recheck scenario envelopes after this lands because it will likely change algae/plant behavior noticeably.
- The dissolved_oxygen.rs system already has a photosynthesis O2 production rate (plant_photosynthesis_o2_mg_per_g_per_hour). The DIC rate should be stoichiometrically consistent: for every mg O2 produced by photosynthesis, ~0.375 mg C is consumed from DIC (from CH2O stoichiometry: CO2 + H2O → CH2O + O2, so 12g C consumed per 32g O2 produced).
  [2026-03-26 17:08 UTC] reviewer: Dependency note: This bead should be aware that C2d (shrimp respiration O2/DIC pathway) also contributes DIC to the water. D4 is primarily about plant/algae photosynthesis and respiration coupling to DIC, but the implementation should integrate with (not duplicate or conflict with) the shrimp DIC contribution established in C2d.

Unit test requirements:

1. test_photosynthesis_consumes_dic: During lit hours with active plants/algae, DIC decreases proportional to photosynthetic rate.
2. test_respiration_produces_dic: During dark hours, respiration from all biomass (microbes, plants, algae, animals) produces DIC. Verify DIC increases.
3. test_day_night_ph_swing: Run 48 hours with light cycle. pH should increase during lit hours (CO2 consumed → DIC decreases → pH rises) and decrease during dark hours (CO2 produced → DIC increases → pH drops). Assert swing magnitude > 0.1 pH units for a planted tank.
4. test_heavy_plant_load_larger_ph_swing: Tank with 2× plant biomass → larger day-night pH swing than tank with 1× plants.
5. test_no_plants_minimal_ph_swing: Tank with no plants/algae → minimal day-night pH variation (only microbial respiration).
6. test_photosynthesis_o2_dic_stoichiometry: For every mole of CO2 consumed by photosynthesis, one mole of O2 is produced. Verify the DIC decrease and DO increase are stoichiometrically consistent (within 5%).
7. test_respiration_o2_dic_stoichiometry: Same for respiration: O2 consumed and DIC produced should be stoichiometrically consistent.

Integration test:
8. test_planted_tank_day_night_cycle: Run a medium_planted scenario for 168 hours (1 week). Capture pH trajectory. Assert: clear diurnal pattern visible, pH range within realistic bounds (6.0-8.0), plants healthy, no crashes.
