○ tanksim-6e5.6.4 · Model reproduction success as a function of temperature, stability, condition, density, and osmotic stress   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: phase-4, reproduction, scientific-core, shrimp, stress

Context:
- Current shrimp reproduction is already directionally temperature-sensitive, but the next step is to tie breeding success/failure to a richer set of explicit mechanisms.
- This should enable better simulation of "tank looks okay but breeding still stalls" outcomes.

Deliverable:
- Extend reproduction logic so success depends on temperature envelope, environmental stability (rate of change), shrimp condition/reserve, density, and relevant water-chemistry stress proxies.
- Keep the first pass aggregated and interpretable.
- Update reproduction-related species parameters and expose the dominant suppression factors in events, debug output, or snapshots.

Factors to include:
  1. Temperature: existing curve (22-28°C optimal), sharply reduced above 30°C
  2. Stability: rapid temperature or chemistry changes suppress reproduction
  3. Condition: well-fed adults with high reserve breed more successfully
  4. Density: overcrowding reduces per-capita breeding rate
  5. Water chemistry: very low GH, high TAN, or high NO2 suppress breeding

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs
- crates/tank_core/src/types/process.rs
- crates/tank_data/data/shrimp/neocaridina_davidi.toml
- events/snapshot output

## Acceptance Criteria


Acceptance Criteria:
Breeding success/failure can diverge across scenarios for explicit, inspectable reasons rather than hidden fertility rolls.
Temperature, density, instability, and chemistry suppression factors are all named parameters in species/process data.
Unit tests verify suppression above 30°C, suppression after a destabilizing temperature swing, improved breeding with high condition, reduced per-capita breeding at high density, and chemistry-driven suppression from TAN/NO2.
Unit tests verify multiple moderate stressors compound, and that sudden instability can trigger egg-dropping or reduced hatch success.
An integration test for a stable, well-maintained tank over 2000 simulated hours shows sustained breeding with at least three reproductive cycles observed.
A contrasting neglected-tank integration test shows chemistry/stability degradation that stalls breeding and flattens or reverses population growth.
Debug output, events, or snapshots identify which factors suppressed reproduction in failing scenarios so tuning remains explainable.

Dependencies:
  -> tanksim-6e5.6.3 (blocks) - Add mineral budget and molt success/failure mechanics
  -> tanksim-6e5.6.2 (blocks) - Implement stage- or size-structured shrimp population dynamics
  -> tanksim-6e5.6 (parent-child) - Phase 4 — shrimp life history, toxicity, and reproduction realism

Dependents:
  <- tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters
  <- tanksim-6e5.6.6 (blocks) - Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Future-self notes:
- The point is not to create hidden fertility dice rolls. Tie outcomes to visible state that the player can monitor and influence.
- Keep the literature-backed temperature signal strong, but do not let temperature become the only story.
- Density and stability should matter because mature tanks often succeed for systemic reasons, not just because they are "warm enough."

Scientific notes:
- Neocaridina davidi breeding behavior: females carry eggs for 21-30 days. Unfavorable conditions can cause egg dropping (loss of clutch).
- Egg dropping triggers: sudden temperature change (>2°C in 24h), very high or very low pH, molting stress, physical disturbance.
- Density-dependent effects: at high densities (>10/L), per-capita reproduction decreases due to competition and stress.
- A "stable, mature tank" breeds well because: temperature is consistent, chemistry is stable, food supply is reliable, and bioload is managed. The simulator should reward this through the interplay of all the above factors.
  [2026-03-26 17:07 UTC] reviewer: Unit test requirements:

1. test_reproduction_suppressed_above_30c: At 31°C, breeding rate drops to near zero. At 25°C (optimal), breeding is at baseline.
2. test_reproduction_suppressed_by_instability: Simulate a 3°C temperature swing in 24 hours. Assert breeding rate drops for 2-3 days after the disturbance.
3. test_reproduction_increases_with_condition: Well-fed shrimp (condition_index > 0.8) → normal breeding. Starved shrimp (condition_index < 0.3) → severely reduced breeding.
4. test_density_reduces_per_capita_breeding: At 2 shrimp/L → normal per-capita breeding rate. At 15 shrimp/L → reduced per-capita breeding rate. Assert monotonic decrease.
5. test_chemistry_stress_suppresses_breeding: High TAN (> 1 mg/L as N) or high NO2 (> 0.5 mg/L as N) → breeding suppression. Verify each stressor independently.
6. test_egg_dropping_from_sudden_change: Berried female experiences >2°C temp change in 24h → probability of egg dropping increases. Verify via reduced hatch success or clutch loss.
7. test_multiple_stressors_compound: Two moderate stressors (slightly elevated temp + moderate instability) produce greater breeding suppression than either alone.
8. test_all_reproduction_factors_are_named_parameters: Temperature curve points, density threshold, instability sensitivity, chemistry thresholds — all tunable via species/process data, not hardcoded.

Integration test:
9. test_mature_stable_tank_breeds_well: Well-maintained tank (regular water changes, stable temp, good GH) → population grows steadily over 2000 hours with at least 3 reproductive cycles observed.
10. test_neglected_tank_breeding_stalls: Same initial conditions but no water changes, overfeeding → chemistry degrades → breeding stalls → population plateaus or declines.
