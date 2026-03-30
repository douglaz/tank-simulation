○ tanksim-6e5.1.3 · Capture baseline scenario envelopes for current v0.1 behavior   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: guardrails, phase-0, scenarios, scientific-core, testing

Context:
- The current code already has scenario presets and regression tests for cycling, oxygen, shrimp reproduction, water changes, ambient temperature, and tank-size effects.
- Those scenarios should become explicit envelopes that future refactors compare against, even if exact trajectories change.

Deliverable:
- Baseline fixtures or notes for current scenarios such as nano_cycle, medium_planted, and warm_room.
- Envelope-based tests or documentation describing the intended qualitative behavior, not just exact current numbers.
- Coverage of the behavioral stories that matter most: cycling timeline, stable-state nutrient ranges, DO behavior, shrimp reproduction windows, thermal response, and algae/plant competition.
- Runnable e2e test scripts that exercise each scenario for a full simulated cycle (500+ hours), capture structured trace output (A2d), and assert envelope compliance. These scripts become the regression harness for all later phases.

Likely touch points:
- crates/tank_data/data/scenarios/*.toml
- crates/tank_scenarios/src/lib.rs
- crates/tank_core/tests/ (existing test modules)

## Acceptance Criteria


Acceptance Criteria:
- each reference scenario has a short "what should happen and why" note
- later phases can compare against qualitative envelopes rather than brittle exact traces
- tank-size, oxygen, and reproduction behavior are explicitly preserved where still desired
- at least one runnable e2e test per shipped scenario that logs structured traces and asserts envelope bounds
- e2e tests support --verbose for full trace dumps and produce CI-friendly exit codes

Dependencies:
  -> tanksim-6e5.1.2.5 (blocks) - Build deterministic scientific-regression harness, e2e runner, and failure artifact capture
  -> tanksim-6e5.1.2.4 (blocks) - Add structured simulation tracing with per-system deltas and configurable verbosity
  -> tanksim-6e5.1.2 (blocks) - Add conservation/debug instrumentation and save-schema migration scaffolding
  -> tanksim-6e5.1.1 (blocks) - Inventory current scientific semantics, units, invariants, and shortcuts
  -> tanksim-6e5.1 (parent-child) - Phase 0 — guardrails, baselines, and migration safety

Dependents:
  <- tanksim-6e5.2.3 (blocks) - Normalize nitrogen-cycle kinetics to concentration-based terms
  <- tanksim-6e5.7.3 (blocks) - Build literature-backed scenario envelopes and expected qualitative outcomes
  <- tanksim-6e5.6.6 (blocks) - Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes
  <- tanksim-6e5.5.7 (blocks) - Add habitat/geometry scenario tests and probes
  <- tanksim-6e5.5.6 (blocks) - Scale equipment, plant mass, and stocking defaults with tank geometry
  <- tanksim-6e5.4.6 (blocks) - Add carbonate regression tests and scenario probes
  <- tanksim-6e5.2.5 (blocks) - Normalize algae kinetics around concentration, light, and temperature interactions
  <- tanksim-6e5.2.4 (blocks) - Normalize plant nutrient uptake and growth limitation semantics

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- The goal is not to freeze today's outputs; it is to preserve the useful behavioral intent while allowing scientifically motivated changes.
- Scenario envelopes should be broad enough to survive tuning but narrow enough to catch regressions.
- These baselines will be especially useful once parameter retuning begins after concentration normalization (B6).

Existing tests to review and potentially convert to envelope-style:
- cycling.rs — TAN peak timing and magnitude, NO2 intermediate, NO3 accumulation
- dissolved_oxygen.rs — DO response to light/dark, aeration, biomass
- shrimp_population.rs — reproduction onset, steady-state population
- tank_size_thermal_response.rs — thermal inertia scaling
- water_change_mass_balance.rs — dilution accuracy
- plant_algae_integration.rs — competition dynamics

Each test should capture: "after N hours of scenario X, quantity Y should be in range [a, b] because Z."
The "because Z" part is critical — it prevents cargo-cult tolerance widening when numbers change.
  [2026-03-26 16:24 UTC] reviewer: E2E test script convention (to be established by this bead):

Location: crates/tank_core/tests/e2e/ (or a dedicated test crate crates/tank_e2e/tests/)

Structure for each e2e script:
- Named test binary or module: e2e_baseline_scenarios.rs, e2e_conservation.rs, etc.
- Each scenario is a #[test] function that:
  1. Constructs a scenario from presets or fixtures
  2. Enables A2d tracing at "detail" verbosity to an in-memory buffer
  3. Runs the simulation for N hours (scenario-specific, typically 500-1000+)
  4. Asserts envelope compliance for key metrics at checkpoint hours
  5. On failure: dumps the full structured trace to a temp file and prints the path
- Supports --verbose via an environment variable (TANK_E2E_VERBOSE=1) for full trace dumps even on success
- Produces CI-friendly exit codes: 0 = all pass, non-zero = envelope violation
- Each test function has a doc comment explaining: what scenario, what behavior it protects, why the envelope bounds are what they are

Convention for envelope assertions:
  assert_in_envelope!(metric, min, max, "explanation of why this range");
  — macro or helper that on failure prints: metric=X, expected=[min, max], at tick=N

This convention applies to: A3 (baselines), C6 (conservation), D6 (carbonate), E7 (habitat/geometry), F6 (shrimp), and G3 (validation scenarios).

All later phase test beads should follow this pattern rather than inventing their own.

