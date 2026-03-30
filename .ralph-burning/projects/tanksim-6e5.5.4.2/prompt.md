○ tanksim-6e5.5.4.2 · Implement simplified denitrification in suboxic substrate zones   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: denitrification, nitrogen, phase-3, scientific-core, substrate

Context:
- Denitrification is the primary pathway for permanent nitrogen removal in planted aquaria. Without it, nitrate accumulates indefinitely unless removed by water changes — a unrealistic simplification.
- The oxic/suboxic zone state from E4a provides the spatial framework: denitrification occurs in the suboxic zone where O2 is depleted but NO3 and organic carbon are available.
- Real denitrification requires: (1) suboxic conditions (low O2), (2) nitrate supply (diffuses from the water column into suboxic pore water), (3) organic carbon as electron donor (DOC in pore water or solid organic matter in substrate).
- The alkalinity produced by denitrification is important for the carbonate system (D7 wires it into alkalinity accounting).
- This bead implements the simplified process; the alkalinity coupling is handled by D7.

Deliverable:
- Implement a denitrification rate in the suboxic substrate zone:
  rate_mg_n_per_hour = denitrifier_activity × monod(NO3_pore, K_no3) × monod(DOC_pore, K_doc) × suboxic_volume_factor
  where NO3_pore and DOC_pore are estimated from water-column concentrations plus a diffusion/mixing factor.
- NO3 consumed by denitrification is removed from the system as N2 gas (permanent export, not recycled).
- Track N removed by denitrification explicitly in the budget ledger (A2a) as an export pathway, not as unexplained loss.
- Use named parameters for all rate constants and half-saturation values.
- Optionally track a denitrifier activity index in the SubstrateDeep habitat (simplified: starts low, builds over weeks in mature substrate) or use a simplified fixed-activity rate.
- The first pass need not model explicit denitrifier biomass growth — a mature-substrate activity coefficient that ramps up with substrate age is sufficient.

Stoichiometry reference:
  Simplified: 5CH2O + 4NO3- + 4H+ → 2N2↑ + 5CO2 + 7H2O
  N removal: 1 mol NO3-N → 0.5 mol N2 (removed as gas)
  C consumed: 1.25 mol C per mol N (5/4 ratio)
  Alkalinity produced: ~0.0714 meq per mg N denitrified (handled by D7)

Likely touch points:
- crates/tank_core/src/systems/nitrogen_cycle.rs (new denitrification step)
- crates/tank_core/src/types/substrate.rs (denitrifier activity index)
- crates/tank_core/src/types/process.rs (denitrification parameters)
- crates/tank_data/data/process/default.toml (default parameter values)
- budget ledger (A2a) for export tracking

## Acceptance Criteria


Acceptance Criteria:
- suboxic substrate removes nitrate at a rate dependent on NO3 concentration, DOC availability, and suboxic zone volume
- denitrification rate responds monotonically to NO3 and DOC concentration using concentration-based Monod kinetics
- N removed by denitrification is tracked as explicit permanent export in budget diagnostics rather than unexplained loss
- all rate constants and half-saturation values are named parameters in process data, not hardcoded
- denitrification is negligible when substrate is fully oxic
- unit test: suboxic zone with high NO3 and high DOC yields measurable N removal per tick
- unit test: zero NO3 yields zero denitrification
- unit test: zero DOC yields zero denitrification
- unit test: larger suboxic zone volume yields proportionally higher total denitrification rate
- unit test: denitrification N removal appears as explicit export in the budget ledger
- integration test: mature planted substrate with active denitrification shows lower steady-state nitrate than an identical unplanted tank
- integration test: a freshly set up tank shows minimal denitrification initially, increasing over weeks if the activity ramp is implemented

Dependencies:
  -> tanksim-6e5.2.3 (blocks) - Normalize nitrogen-cycle kinetics to concentration-based terms
  -> tanksim-6e5.4.7 (blocks) - Validate nitrification-driven alkalinity depletion with the carbonate solver, expose diagnostics, and prepare the denitrification return path
  -> tanksim-6e5.5.4.1 (blocks) - Add oxic/suboxic zone state to substrate model
  -> tanksim-6e5.5.4 (parent-child) - Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks

Dependents:
  <- tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Scientific notes:
- Denitrification stoichiometry (simplified): 5 CH2O + 4 NO3- → 2 N2↑ + 4 HCO3- + CO2 + 3 H2O
- This means denitrification: (a) removes NO3, (b) consumes DOC, (c) produces alkalinity (HCO3-), (d) produces DIC.
- The alkalinity production is significant: denitrification partially compensates for alkalinity consumed by nitrification. This is a real and important self-regulating mechanism in mature planted tanks.
- Rate-limiting factors in order of importance: NO3 availability at the suboxic boundary, DOC diffusion, suboxic zone volume.
- Keep the first pass simple: rate = denitrification_vmax × [NO3] / ([NO3] + K_no3) × [DOC] / ([DOC] + K_doc) × suboxic_volume_fraction.
  [2026-03-26 14:47 UTC] reviewer: Alkalinity production from denitrification:
When implementing this bead, wire the denitrification alkalinity production into the same pathway established by D4.7 (tanksim-6e5.4.7). The stoichiometry is: denitrification produces ~0.0714 meq alkalinity per mg NO3-N reduced (equivalent to 3.57 mg CaCO3/mg N). Use the named constant pattern from D4.7 rather than introducing new magic numbers.

This alkalinity production partially offsets nitrification's alkalinity consumption — a real and important self-regulating mechanism in mature planted tanks with deep substrate. It is one of the reasons mature planted tanks maintain more stable pH than bare-bottom tanks.
  [2026-03-26 16:23 UTC] reviewer: Unit test requirements added:

1. test_denitrification_removes_no3: Suboxic zone with NO3 available → NO3 decreases over time.
2. test_denitrification_requires_doc: No DOC → no denitrification (carbon source required).
3. test_denitrification_rate_depends_on_suboxic_volume: Larger suboxic zone → higher denitrification rate.
4. test_denitrification_produces_alkalinity: After denitrification, alkalinity_meq_total increases by DENITRIFICATION_ALK_MEQ_PER_MG_N × mg_N_denitrified (within 1e-6 tolerance).
5. test_denitrification_tracked_as_export: N removed by denitrification appears as explicit N2 gas export in budget tracking, not as unexplained loss.
6. test_denitrification_monod_concentration_based: Rate depends on NO3 and DOC concentrations (mg/L), not totals — following the same pattern established in B3.
7. test_denitrification_stoichiometry: For every mg NO3-N reduced, verify approximately correct DOC consumed (5/4 × 12/14 ≈ 1.07 mg C per mg N) and DIC produced.

  [2026-03-26 17:07 UTC] reviewer: Unit test requirements:

1. test_denitrification_consumes_nitrate: In suboxic zone with available organic carbon, nitrate decreases over time. Assert NO3 consumption rate is proportional to available NO3 and organic C.
2. test_denitrification_produces_n2_loss: Denitrification converts NO3 to N2 (gas loss). This is an intentional system export — total N DECREASES. Budget instrumentation should classify this as explicit N export, not a conservation violation.
3. test_denitrification_returns_alkalinity: Denitrification produces ~3.57 mg CaCO3 alkalinity per mg NO3-N reduced. Verify alkalinity increase matches stoichiometry.
4. test_no_denitrification_in_oxic_zone: Oxic substrate zone → denitrification rate ≈ 0.
5. test_denitrification_rate_scales_with_organic_carbon: More organic C in suboxic zone → faster denitrification (up to saturation).
6. test_denitrification_rate_scales_with_suboxic_depth: Thicker suboxic zone → more denitrification capacity.

Integration test:
7. test_denitrification_reduces_nitrate_accumulation: Tank with active substrate (thick suboxic zone) vs inert substrate. After 500 hours of cycling, tank with denitrification has measurably lower NO3 accumulation. Assert with envelope bounds.
