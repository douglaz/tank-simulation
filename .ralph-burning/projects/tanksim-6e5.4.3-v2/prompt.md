○ tanksim-6e5.4.3 · Integrate CO2 gas exchange with aeration, surface exchange, and ambient coupling   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: co2, gas-exchange, hardware, phase-2, scientific-core

Context:
- Aeration currently affects dissolved oxygen (dissolved_oxygen.rs:27-33) but not carbon dioxide stripping or pH.
- dissolved_oxygen.rs already has a gas exchange model: delta_do = k_la × (do_sat - do_current) × volume.
- A planted tank simulator needs surface exchange that moves both gases, even if the model remains lumped rather than spatially resolved.

Deliverable:
- Extend gas exchange so CO2 responds to aeration/surface exchange and interacts with the carbonate state.
- Use existing geometry and hardware inputs (K_LA, top_exchange_factor, aeration_intensity) where possible.
- Keep the first pass consistent with the chosen carbonate update order.

Parallel to existing O2 exchange:
  O2: delta_o2 = k_la × (do_sat_mg_l - do_mg_l) × volume_l
  CO2: delta_co2 = k_la_co2 × (co2_sat_mg_l - co2_current_mg_l) × volume_l
  where co2_sat depends on atmospheric pCO2 (~410 ppm) and Henry's law.

Key difference from O2:
- CO2 is much more soluble than O2 (Henry's law constant is ~30× higher).
- K_LA for CO2 is related to K_LA for O2 by the ratio of diffusion coefficients: K_LA(CO2) ≈ K_LA(O2) × (D_CO2/D_O2)^0.5 ≈ K_LA(O2) × 0.91.
- CO2 exchange affects DIC, which affects pH through the carbonate equilibrium.

Likely touch points:
- crates/tank_core/src/systems/chemistry.rs (or new co2_exchange module)
- crates/tank_core/src/systems/dissolved_oxygen.rs (share geometry/KLA terms)
- crates/tank_core/src/types/hardware.rs
- crates/tank_core/src/types/geometry.rs

## Acceptance Criteria


Acceptance Criteria:
- high aeration can strip CO2 and influence pH in the expected direction (pH rises as CO2 decreases)
- gas exchange logic for O2 and CO2 is conceptually aligned (shared K_LA basis)
- the model remains deterministic and testable
- unit test: a tank with high dissolved CO2 (10 mg/L) and strong aeration → CO2 decreases toward equilibrium (~0.6 mg/L at 25°C, 410 ppm) within 24 simulated hours
- unit test: same tank without aeration → CO2 decreases much more slowly (surface exchange only)
- unit test: CO2 exchange rate is proportional to K_LA from dissolved_oxygen.rs (shared basis verified by comparing the two rates under identical hardware settings)
- integration test: aeration on → pH rises by at least 0.2 units over 12 hours in a scenario with elevated CO2

Dependencies:
  -> tanksim-6e5.4.2 (blocks) - Implement explicit carbonate equilibrium and pH solver
  -> tanksim-6e5.4 (parent-child) - Phase 2 — carbonate chemistry, CO2 exchange, and pH realism

Dependents:
  <- tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters
  <- tanksim-6e5.4.6 (blocks) - Add carbonate regression tests and scenario probes

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- This bead is what makes aeration scientifically richer than "more oxygen = good." In real planted tanks, heavy aeration strips CO2, which is good for fish but bad for plants that need dissolved CO2 for photosynthesis.
- It also gives the simulation a path toward later CO2 hardware (injection) without requiring that feature immediately.
- Reuse existing geometry terms like exposed area (top_exchange_factor()) where they already exist; do not create parallel geometry semantics.

Scientific notes:
- Atmospheric CO2 ≈ 410 ppm → CO2 equilibrium in water at 25°C ≈ 0.6 mg/L CO2 ≈ 0.16 mg C/L.
- In a non-aerated tank with active biology, CO2 can accumulate well above equilibrium (5-30 mg/L), driving pH down.
- Strong aeration strips CO2 toward equilibrium, raising pH. This is the primary mechanism by which aeration affects pH in freshwater.
- Henry's law: [CO2(aq)] = K_H × pCO2. K_H ≈ 3.4 × 10^-2 mol/(L·atm) at 25°C.
  [2026-03-26 17:08 UTC] reviewer: Unit test requirements:

1. test_co2_reaeration_toward_equilibrium: Water with high dissolved CO2 → CO2 off-gasses toward atmospheric equilibrium. Water with low dissolved CO2 → CO2 dissolves from atmosphere. Both directions should work.
2. test_aeration_accelerates_co2_exchange: With aeration on → CO2 exchange rate is higher than surface exchange alone (aeration multiplier > 1.0).
3. test_surface_area_affects_exchange_rate: Tank with larger surface area → faster gas exchange (both O2 and CO2).
4. test_co2_equilibrium_concentration: At 25°C, atmospheric CO2 (~420 ppm) equilibrium in freshwater is approximately 0.5-0.7 mg/L. Verify equilibrium CO2 is in this range.
5. test_temperature_affects_equilibrium: Warmer water → lower CO2 equilibrium (Henry's law temperature dependence). Verify at 20°C vs 30°C.
6. test_co2_exchange_independent_of_tank_volume: Gas exchange rate per unit surface area should be volume-independent. Two tanks with same surface area but different depths → same CO2 flux at the surface.
7. test_high_co2_from_respiration_drives_offgassing: After dark period with high respiration → elevated dissolved CO2 → increased off-gassing rate next lit period.

Integration test:
8. test_aerated_vs_nonaerated_co2_levels: Two identical tanks, one with aeration, one without. After 100 hours, aerated tank has CO2 closer to atmospheric equilibrium. Non-aerated tank has higher CO2 from accumulated respiration. Assert difference with envelope bounds.
