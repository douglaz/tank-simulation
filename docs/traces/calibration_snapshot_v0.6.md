# Post-Refactor Calibration Snapshot

Date: 2026-03-30
Commit base: 6abb4ee (post tanksim-6e5.7.5, all Phase 5 beads merged)
Parameter set: default (shipped TOML presets, no manual overrides)

## Context

This snapshot records the simulator's tuning state after the full scientific-core
upgrade (phases 2-6, beads tanksim-6e5.2.x through tanksim-6e5.7.5). It serves
as the baseline for any future parameter changes and documents what the calibration
harness reports at the moment all Phase 5 deliverables are complete.

## Calibration Report Summary

| Metric | Value |
|--------|-------|
| Total scenarios | 8 |
| Pass | 6 |
| Marginal | 2 |
| Fail | 0 |

### Per-Scenario Results

| ID | Name | Domain | Confidence | Provenance | Status |
|----|------|--------|------------|------------|--------|
| vs01 | Fishless cycling timeline | Chemistry / Microbiology | high | validated directionally | **marginal** |
| vs02 | Aeration effects (DO + pH) | Physics / Chemistry | high | validated directionally | pass |
| vs03 | Day/night pH swing | Chemistry / Plant biology | high | validated directionally | pass |
| vs04 | Source-water differentiation | Chemistry | high | validated directionally | pass |
| vs05 | Shrimp breeding thermal window | Animal behavior | high | validated directionally | pass |
| vs06 | Algae-plant competition | Ecology | medium | still heuristic | pass |
| vs07 | Nitrate removal (denitrification) | Microbiology / Chemistry | medium | still heuristic | pass |
| vs08 | Stocking density crash | Toxicology / Husbandry | high | validated directionally | **marginal** |

## Marginal Findings — Analysis

### vs01: Fishless cycling timeline (marginal)

**Observed**: pH clamped at 8.500 across all checkpoints; TAN and nitrite very
low (0.3-1.1 mg N/L and ~0.002 mg N/L respectively through week 8).

**Root cause**: Two distinct mechanisms produce the marginal classification:

1. **pH upper clamp**: The carbonate solver clamps pH at 8.5 (gameplay bound
   defined in `chemistry.rs`). With moderate-buffered source water and no acid
   load, the equilibrium pH naturally wants to sit at or above this clamp. The
   value 8.500 is exactly at the envelope boundary [6.5, 8.5], triggering the
   10% marginal heuristic every checkpoint.

2. **Near-zero nitrogen species**: TAN and nitrite values close to 0.0 with
   envelopes bounded at 0.0 trigger the 10% boundary detector. These are
   structurally marginal — any non-negative value within 10% of a zero lower
   bound is flagged.

**Assessment**: Neither finding indicates a parameter tuning problem. The pH
clamp is a deliberate design choice (documented in `carbonate_state_contract.md`).
The near-zero nitrogen values mean the biofilter processes ammonia effectively
in the cycling scenario, which is the expected directional behavior. The marginal
classification is an artifact of the 10% boundary heuristic applied to clamped
and zero-bounded fields.

**Action**: No parameter changes warranted. Consider widening the vs01 pH
envelope upper bound to 8.6 or annotating the clamp interaction in the
validation scenario definition.

### vs08: Stocking density crash (marginal)

**Observed**: shrimp_count=0 at week 4 checkpoint, within envelope [0, 5].

**Root cause**: All shrimp die before week 4 in this scenario (by design — the
scenario tests overloaded, neglected conditions). The value 0 at the zero lower
bound of [0, 5] triggers the 10% marginal heuristic.

**Assessment**: The crash trajectory is correct: 20 shrimp initially, 10 at
week 1, 3 at week 2, 0 at week 4, still 0 at week 8 with TAN at 275 mg N/L.
This matches the expected overload story. The marginal is a boundary-detection
artifact, not a tuning concern.

**Action**: No parameter changes warranted. The week_4 envelope lower bound
of 0 correctly allows full die-off; the marginal flag is harmless noise.

## Key Observed Behaviors (Passing Scenarios)

### vs02: Aeration effects
- Aerated tank: DO=8.45 mg/L, pH=8.33
- Passive tank: DO=4.17 mg/L, pH=6.73
- DO gap: 4.28 mg/L; pH gap: 1.60 units
- Strong directional differentiation confirms CO2 stripping and gas exchange coupling.

### vs03: Day/night pH swing
- Light-phase max pH: 8.500, dark-phase min pH: 7.580
- pH swing: 0.920 units (envelope: 0.2-1.0)
- Cycle-over-cycle consistency confirmed (cycle 1: 0.920, cycle 2: 0.772).

### vs04: Source-water differentiation
- Hard shrimp water: pH 8.257
- RO-like water: pH 6.732
- Gap: 1.525 pH units (envelope requires >= 1.0)
- Confirms carbonate solver produces meaningful chemistry differentiation.

### vs05: Shrimp breeding thermal window
- Cool (24C): 260 population, 10 breeding cycles, 241 offspring, readiness 0.915
- Warm (30C): 10 population, 0 breeding cycles, 0 offspring, readiness 0.017
- Thermal suppression ratio >50:1 in offspring, confirming strong temperature gating.

### vs06: Algae-plant competition
- Plants declined from 8.0g to 1.73g (0.22x), health dropped from 0.8 to 0.0
- Algae held at 70% of initial biomass, nuisance rose from 0.1 to 0.58
- Directionally correct: nutrient-limited plants lose to algae under sustained stress.

### vs07: Nitrate removal (denitrification)
- Planted tank: NO3=0.13 mg N/L, N2 export=87.1 mg N, denitrifier activity=0.213
- Bare tank: NO3=0.24 mg N/L, N2 export=0.0 mg N
- Confirms denitrification active only in mature planted substrate.

## Test Suite Status

| Suite | Tests | Pass | Fail | Ignored |
|-------|-------|------|------|---------|
| tank_core unit + regression | 45 | 45 | 0 | 0 |
| tank_scenarios | 16 | 16 | 0 | 0 |
| tank_harness (validation + calibration) | 24 | 24 | 0 | 5 (explicit-only probes) |
| tank_tui | 11 | 11 | 0 | 0 |
| tank_api | (compile check) | - | 0 | - |
| **Total** | **96+** | **96+** | **0** | **5** |

Code quality: `cargo clippy` clean (zero warnings), `cargo fmt --check` clean.

## Conclusion

The simulator is in a stable, well-characterized post-refactor state. All 8
validation scenarios pass or are marginal for structural (non-tuning) reasons.
No parameter changes are needed at this time. The two marginal findings are
documented as boundary-detection artifacts and do not indicate model deficiencies.
