○ tanksim-6e5.4.6 · Add carbonate regression tests and scenario probes   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: carbonates, phase-2, scenarios, scientific-core, testing

Context:
- Carbonate work is numerically delicate enough that it needs strong regression coverage as soon as it lands.
- The tests should protect the intended qualitative behavior without freezing every intermediate value forever.

Deliverable:
- Scenario tests/probes for:
  1. Source-water pH differentiation: hard_shrimp vs ro_like should produce pH values at least 1.0 units apart.
  2. Aeration-driven CO2 stripping: high aeration should raise pH compared to no aeration.
  3. Day/night pH drift: light-on pH should be higher than light-off pH in a planted tank.
  4. Water change pH effect: changing from RO to hard water should shift pH upward.
- Debug output or probe utilities that help inspect carbonate state during failures.
- Explicit test notes about acceptable ranges/tolerances.

Likely touch points:
- chemistry/system tests
- scenario fixtures or helpers
- debug/snapshot output used for verification

## Acceptance Criteria


Acceptance Criteria:
At least the core carbonate stories are codified in tests: source-water differentiation, aeration effects, day/night swing, water-change pH shift, and nitrification-driven pH decline.
Each test scenario is a named test function with a doc comment explaining the expected chemical behavior and why.
Failures dump carbonate state (DIC, alkalinity, CO2(aq), HCO3-, pH) per tick via A2d tracing for diagnosis.
Tolerance ranges are documented and justified in test comments (e.g., "hard_shrimp pH in [7.3, 7.8] because KH 8 + equilibrium DIC → analytical pH ≈ 7.5").
Scenario probes include at minimum:
  1. Source-water pH differentiation: hard_shrimp vs ro_like should produce pH values at least 1.0 units apart.
  2. Aeration-driven CO2 stripping: high aeration should raise pH compared to no aeration.
  3. Day/night pH drift: light-on pH should be higher than light-off pH in a planted tank.
  4. Water change pH effect: changing from RO to hard water should shift pH upward.
  5. Nitrification-driven pH decline: a cycling tank with active nitrification in soft water (KH 2) shows pH decline > 0.5 units over 200 hours; same scenario in hard water (KH 8) shows pH decline < 0.2 units — demonstrating the buffering story.
An e2e carbonate test script runs all 5 probes, produces a summary report with observed pH ranges, and exits non-zero on envelope violation.
Future tuning can change numbers inside the envelope without rewriting the scientific story.

Dependencies:
  -> tanksim-6e5.1.2.5 (blocks) - Build deterministic scientific-regression harness, e2e runner, and failure artifact capture
  -> tanksim-6e5.4.3 (blocks) - Integrate CO2 gas exchange with aeration, surface exchange, and ambient coupling
  -> tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  -> tanksim-6e5.1.2.4 (blocks) - Add structured simulation tracing with per-system deltas and configurable verbosity
  -> tanksim-6e5.4 (parent-child) - Phase 2 — carbonate chemistry, CO2 exchange, and pH realism
  -> tanksim-6e5.4.7 (blocks) - Validate nitrification-driven alkalinity depletion with the carbonate solver, expose diagnostics, and prepare the denitrification return path
  -> tanksim-6e5.4.5 (blocks) - Upgrade source-water profiles to carry carbonate-relevant inputs and differentiated defaults
  -> tanksim-6e5.4.4 (blocks) - Connect photosynthesis and respiration to DIC / CO2 and day–night pH behavior

Dependents:
  <- tanksim-6e5.7.3 (blocks) - Build literature-backed scenario envelopes and expected qualitative outcomes
  <- tanksim-6e5.7.5 (blocks) - Update developer docs, TUI/API messaging, and scientific-scope narrative

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Future-self notes:
- Carbonate chemistry bugs can look like "just a weird pH drift." Make the tests narrative enough that failures point to a plausible cause.
- Keep tolerance choices visible and justified. Example: "pH should be in [7.3, 7.8] for hard_shrimp because KH 8 + equilibrium DIC → analytical pH ≈ 7.5."
- Revisit these tests after habitat and shrimp phases if new coupled dynamics widen the realistic envelope.
