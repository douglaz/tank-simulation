○ tanksim-6e5.6.3 · Add mineral budget and molt success/failure mechanics   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: minerals, molting, phase-4, scientific-core, shrimp, water-chemistry

Context:
- Shrimp husbandry is heavily shaped by minerals and molt success, but the current model does not yet express that linkage explicitly.
- With richer water chemistry (D5 source water profiles) and clearer state structure (F2) in place, molt outcomes can now depend on something more meaningful than generic stress.

Deliverable:
- Introduce a simplified mineral-budget or availability check that affects molt success/failure.
- Tie the mechanic to the water-profile story established earlier without demanding perfect ionic physiology.
- Surface the outcome in events, snapshots, or tracing so players and developers can tell whether failures came from low GH, missing Ca/Mg, or poor condition.
- Keep thresholds and modifiers inspectable and tunable through named parameters rather than hardcoded constants.

Key mechanism:
  molt_success_probability = base_rate × mineral_modifier(GH, Ca, Mg) × condition_modifier(reserve)
  - Low GH (< 4°) -> reduced molt success (insufficient Ca/Mg for exoskeleton)
  - Very low GH (< 2°) -> high molt failure risk
  - High condition/reserve -> normal molt success
  - Low condition (recent starvation or stress) -> reduced molt success

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs
- crates/tank_core/src/types/water.rs (GH/Ca/Mg access)
- source-water/shrimp data files
- snapshots/events/UI text

## Acceptance Criteria


Acceptance Criteria:
Molt success is influenced by explicit chemistry-related state and reserve/condition rather than a hidden generic penalty.
All molt-related thresholds and modifiers (GH thresholds, Ca/Mg minimums, stage cadence if applicable) are named parameters in species/process data, not hardcoded constants.
Unit tests verify GH monotonicity into the healthy range, that both calcium and magnesium contribute to the mineral modifier, and that low reserve/condition reduces molt success.
Unit tests verify repeated failed molts increase mortality and that juveniles molt more frequently than adults once stage structure exists.
An integration test contrasts soft-water vs hard-water tanks over 1000 simulated hours and shows detectable molt-failure deaths plus worse survival in the mineral-poor case.
Events, tracing, or snapshots expose enough detail to explain why molt failure occurred in a failing scenario.

Dependencies:
  -> tanksim-6e5.6.2 (blocks) - Implement stage- or size-structured shrimp population dynamics
  -> tanksim-6e5.4.5 (blocks) - Upgrade source-water profiles to carry carbonate-relevant inputs and differentiated defaults
  -> tanksim-6e5.6 (parent-child) - Phase 4 — shrimp life history, toxicity, and reproduction realism

Dependents:
  <- tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters
  <- tanksim-6e5.6.6 (blocks) - Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes
  <- tanksim-6e5.6.4 (blocks) - Model reproduction success as a function of temperature, stability, condition, density, and osmotic stress

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Rationale:
- This bead gives GH/TDS/mineral-related husbandry a real gameplay and scientific role.
- Avoid pretending to model full crustacean physiology; the goal is a credible simplified dependency that explains common aquarium outcomes.
- Keep the player-facing feedback clear so the mechanic teaches rather than mystifies.

Scientific notes:
- Neocaridina davidi molt approximately every 4-6 weeks as adults, more frequently as juveniles.
- Exoskeleton formation requires Ca2+ and Mg2+ ions. In very soft water (GH < 2°), shrimp often have visibly thin shells and higher molt mortality.
- The "white ring of death" (failed molt visible as a white band) is a commonly observed phenomenon in soft-water shrimp tanks.
- Recommended GH for Neocaridina: 6-8° (≈ 100-140 mg/L as CaCO3).
  [2026-03-26 16:22 UTC] reviewer: Dependency fix: removed blocks-dependency on B8 (TDS/conductivity expansion, P2).

Rationale: F3 (molt/minerals) needs access to GH, Ca2+, and Mg2+ — all of which ALREADY EXIST in the codebase:
- water.rs: calcium_mg_total, magnesium_mg_total
- shrimp.rs line 89: GH is computed as ((2.497 × Ca_mg_l) + (4.118 × Mg_mg_l)) / 17.848
- The existing GH mineral factor in condition_index (lines 150-203) already penalizes low GH.

B8 is about expanding TDS/conductivity DISPLAY accounting and honesty — it adds no new mineral state that F3 needs. The old dependency created a priority inversion (P1 blocked by P2).

F3 still correctly depends on: F2 (stage structure), D5 (source water profiles with richer mineral differentiation).

  [2026-03-26 17:07 UTC] reviewer: Unit test requirements:

1. test_molt_success_increases_with_gh: GH 2° → low molt success. GH 6° → normal molt success. GH 10° → normal (plateau). Verify monotonic increase up to optimal range.
2. test_molt_success_formula_uses_ca_and_mg: Verify that both calcium_mg_total and magnesium_mg_total contribute to the mineral modifier (not just GH shortcut).
3. test_molt_success_affected_by_condition: Well-fed shrimp (high reserve) → higher molt success than starved shrimp (low reserve).
4. test_failed_molt_increases_mortality: When molt fails, affected shrimp have increased mortality probability. Assert population decreases faster with repeated molt failures.
5. test_very_low_gh_causes_molt_deaths: In very soft water (GH < 2°), run 500 hours. Assert deaths from molt failure are detectable in event log.
6. test_mineral_modifier_named_parameters: All molt-related thresholds (GH thresholds, Ca/Mg minimums) are named parameters that can be overridden in species data, not hardcoded constants.
7. test_molt_frequency_varies_by_stage: If stage-structured population (F2) is in place, juveniles molt more frequently than adults. Verify per-stage molt intervals differ.

Integration test:
8. test_soft_vs_hard_water_shrimp_survival: Two identical tanks, one with GH 2° (RO water), one with GH 8° (hard_shrimp source water). Run 1000 hours. Hard water tank maintains stable population; soft water tank shows declining population from molt failures. Assert population difference with envelope bounds.
