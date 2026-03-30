○ tanksim-6e5.6.5 · Model chloride protection against nitrite hazard and integrate toxic stress accounting   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: chloride, nitrite, phase-4, scientific-core, shrimp, toxicology

Context:
- The simulator already tracks chemistry that can support better stress logic (chloride_mg_total in water.rs:26, nitrite_mg_n_total in water.rs:13), but nitrite hazard is still missing an important modifier: chloride.
- Adding this makes stress outcomes less naive and better tied to water composition.

Deliverable:
- Introduce a chloride-aware nitrite hazard modifier suitable for the project's fidelity level.
- Integrate the result into the existing shrimp stress/mortality accounting rather than creating a parallel toxicology path.
- Document assumptions and parameter confidence clearly, and expose the effective hazard in debug/tracing output so salt-treatment scenarios are explainable.

Mechanism:
  effective_nitrite_hazard = [NO2-] / (1 + chloride_protection_factor × [Cl-] / [NO2-])
  where chloride_protection_factor is a species-specific parameter.
  High chloride relative to nitrite -> reduced effective hazard.

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs
- crates/tank_core/src/types/water.rs
- source-water and process parameter files
- validation/provenance notes

## Acceptance Criteria


Acceptance Criteria:
Equal nitrite does not imply equal hazard when chloride differs, and chloride protection feeds into the existing nitrite-stress accounting path.
Chloride_protection_factor is a named species parameter rather than a hardcoded constant.
Unit tests verify monotonic hazard reduction with increasing chloride, zero hazard when nitrite is zero, and a strongly protective high Cl:NO2 ratio.
Unit tests verify low-chloride/high-nitrite scenarios increase mortality more than otherwise identical high-chloride scenarios.
An integration test models a salt-treatment emergency: mortality is high before chloride is raised and measurably lower after the treatment even while NO2 remains elevated.
Tracing, events, or debug output expose NO2, chloride, effective hazard, and resulting stress contribution so failures are diagnosable.

Dependencies:
  -> tanksim-6e5.6.2 (blocks) - Implement stage- or size-structured shrimp population dynamics
  -> tanksim-6e5.4.5 (blocks) - Upgrade source-water profiles to carry carbonate-relevant inputs and differentiated defaults
  -> tanksim-6e5.6 (parent-child) - Phase 4 — shrimp life history, toxicity, and reproduction realism

Dependents:
  <- tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters
  <- tanksim-6e5.6.6 (blocks) - Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Rationale:
- This bead is a good example of "small addition, big explanatory value."
- Keep the formulation modest and honest about uncertainty; the win is mechanistic directionality, not toxicology exhaustiveness.
- Treat this as a bridge between chemistry richness and animal outcomes.

Scientific background:
- Nitrite is toxic to freshwater crustaceans because NO2- competes with Cl- for uptake at the gills.
- In high-chloride water, nitrite uptake is inhibited, reducing toxicity. This is why salt (NaCl) is a common emergency treatment for nitrite poisoning in aquaria.
- The Cl-:NO2- ratio is the key modifier. A ratio > 10:1 is generally considered protective.
- Source: chloride reduces nitrite accumulation and associated stress in freshwater organisms (ResearchGate: Chloride uptake in freshwater teleosts).
  [2026-03-26 16:52 UTC] master: Dependency refinement: removed the B8 TDS/conductivity blocker. Chloride-aware nitrite hazard depends on chloride state and source-water differentiation, not on how TDS/conductivity are labeled for users.
  [2026-03-26 17:07 UTC] reviewer: Unit test requirements:

1. test_chloride_reduces_nitrite_hazard: At [NO2] = 1.0 mg/L, [Cl] = 0 → full nitrite hazard. [Cl] = 30 mg/L → reduced hazard. Verify hazard decreases monotonically with increasing chloride.
2. test_chloride_protection_ratio: At Cl:NO2 ratio > 10:1, effective hazard is < 20% of unprotected hazard.
3. test_zero_nitrite_zero_hazard: Regardless of chloride level, [NO2] = 0 → effective hazard = 0.
4. test_high_nitrite_low_chloride_lethal: [NO2] = 5.0 mg/L, [Cl] = 0 → severe stress/mortality. Assert shrimp mortality rate increases.
5. test_high_nitrite_high_chloride_survivable: [NO2] = 5.0 mg/L, [Cl] = 100 mg/L → stress present but significantly reduced mortality compared to test 4.
6. test_protection_factor_is_species_parameter: chloride_protection_factor is a named parameter in shrimp species data, not hardcoded.
7. test_stress_accounting_integrates_with_existing: Chloride-modified nitrite stress feeds into the existing hourly_nitrite_stress_accum pathway, not a parallel stress system.

Integration test:
8. test_salt_treatment_emergency_scenario: Tank with cycling crash (high NO2). Apply NaCl treatment (increase chloride via water change with salty source water). Assert: before salt → high mortality, after salt → mortality rate decreases despite NO2 still being elevated.
