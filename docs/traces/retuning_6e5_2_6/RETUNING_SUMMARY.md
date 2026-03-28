# Retuning Summary: tanksim-6e5.2.6

Date: 2026-03-28
Commit base: e102f62 (post-normalization from 6e5.2.3, 6e5.2.4, 6e5.2.5)

## Context

Phase 1A normalization (beads 6e5.2.3 through 6e5.2.5) changed all kinetic
half-saturation constants from whole-tank totals (divided by a 20 L reference
at runtime) to direct concentration-based values (mg/L). This fundamentally
changed the effective Ks values used in Monod limitation calculations.

## Parameter Changes (Old Effective → New)

### Nitrogen kinetics (6e5.2.3)

| Parameter | Old stored | Old effective (÷20 L) | New (direct mg/L) | Change |
|-----------|-----------|----------------------|-------------------|--------|
| decomposer_k_doc | 5.0 | 0.25 mg C/L | 3.0 mg C/L | 12× higher Ks |
| decomposer_k_do | 2.0 | 0.10 mg O₂/L | 0.5 mg O₂/L | 5× higher Ks |
| aob_k_tan | 0.5 | 0.025 mg N/L | 1.0 mg N/L | 40× higher Ks |
| aob_k_do | 1.0 | 0.05 mg O₂/L | 0.5 mg O₂/L | 10× higher Ks |
| nob_k_nitrite | 0.3 | 0.015 mg N/L | 0.5 mg N/L | 33× higher Ks |
| nob_k_do | 1.0 | 0.05 mg O₂/L | 0.8 mg O₂/L | 16× higher Ks |
| comammox_k_tan | 0.8 | 0.04 mg N/L | 0.2 mg N/L | 5× higher Ks |
| comammox_k_do | 1.5 | 0.075 mg O₂/L | 0.6 mg O₂/L | 8× higher Ks |

### Plant kinetics (6e5.2.4)

Field renames from `_mg_total` to concentration-based names. Substrate Ks
(mg/m²) was new — no old equivalent.

### Algae kinetics (6e5.2.5)

Field renames from `_mg_total` to concentration-based names. Values adjusted
to literature ranges.

## Classification

All changes are **unit corrections**, not new science. The old effective Ks
values were far below any published literature range because the 20 L
divisor was an implementation artifact, not a calibration choice. The new
values sit within published ranges for freshwater aquarium biofilter
communities.

## Scenario Verification

All 9 envelope tests pass with the retuned parameters:

- nano_cycle_baseline_envelope: pH crash, TAN accumulation, shrimp death ✓
- medium_planted_baseline_envelope: guild competition, biofilter maturation ✓
- warm_room_baseline_envelope: thermal equilibration, algae dominance ✓
- cross_scenario_relative_behaviors: warm > cool temp, medium > nano plants ✓
- thermal_inertia_scales_with_tank_size: nano equilibrates faster ✓
- do_dips_at_night_in_planted_tank: light/dark DO cycle ✓
- shrimp_husbandry_fixture_reaches_berried_window: reproduction pathway ✓
- controlled_ideal_reproduction_path_still_hatches: hatch pipeline ✓
- biofilter_reaches_scenario_specific_cycle_floors: maturity timelines ✓

## Key Observations from Retuning Runs

### nano_cycle (seed=42, 0.05 g/day feed, soft_acidic water)

- Day 30: TAN=16.6, NO2=4.1, pH=8.50, biofilter maturity=0.55
- Shrimp all dead by week 1 (TAN lethal)
- Day 114: TAN=52.7, pH=6.47, plants declining (3.2 g from 5.0 g start)

### medium_planted (seed=42, 0.1/0.2 g/day feed, moderate water)

- Day 30: TAN=6.9, NO2=0.04, pH=8.50, plants 10.3 g (stems 5.8, rosettes 4.5)
- Rosettes declining by week 4 (substrate nutrients depleted)
- Day 114: TAN=13.6, pH=6.86, fast stems 6.3 g, rosettes 1.2 g, algae nuisance 0.22

### warm_room (seed=42, 0.05/0.15 g/day feed, moderate water, 29°C ambient)

- Day 7: temperature already at 29°C, DO=7.71 (vs 8.42 for nano)
- Day 30: TAN=8.2, NO2=0.42, biofilter maturity=0.27 (faster than medium)
- Day 114: TAN=28.5, algae nuisance=0.57, plants declining (3.1 g)

## Files in This Archive

- `discover_baselines.txt`: Full checkpoint trace from all three scenarios
- `RETUNING_SUMMARY.md`: This file
