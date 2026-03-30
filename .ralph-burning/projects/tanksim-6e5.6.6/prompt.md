○ tanksim-6e5.6.6 · Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: phase-4, scenarios, scientific-core, shrimp, testing

Context:
- Once the shrimp model is richer, it needs scenario coverage that demonstrates both success and failure modes.
- These tests should support calibration and gameplay confidence, not just prevent panics.

Deliverable:
- Scenario probes for:
  1. Successful breeding: mature planted tank, stable conditions, adequate minerals → population grows
  2. Thermal suppression: temperature above 30°C → breeding stops, egg dropping
  3. Chemistry-induced stress: low GH + high NO2 → molt failures, increased mortality
  4. Chloride protection: same NO2 but high chloride → less mortality than low chloride
  5. Crash mode: overcrowding + overfeeding + no water changes → population collapse
- Expected qualitative outcomes and acceptable envelopes for each scenario.
- Snapshot/debug support needed to explain failures.

Likely touch points:
- scenario/test crates
- shrimp snapshot/event output
- calibration notes

## Acceptance Criteria


Acceptance Criteria:
- each of the 5 shrimp scenarios (breeding success, thermal suppression, chemistry stress, chloride protection, crash mode) is a named test function with a doc comment explaining the husbandry story being validated
- failures dump per-stage shrimp state (counts, condition, reserve, molt status) and water chemistry (GH, pH, NO2, Cl) per tick via A2d tracing
- an e2e shrimp test script runs all 5 scenarios, logs structured traces, produces a summary report with population outcomes, and exits non-zero on envelope violation
- later retuning can preserve intent without freezing every exact count: assertions use directional/envelope comparisons
- the successful-breeding scenario runs long enough (1000+ hours) to observe at least 2 complete reproductive cycles

Dependencies:
  -> tanksim-6e5.1.2.5 (blocks) - Build deterministic scientific-regression harness, e2e runner, and failure artifact capture
  -> tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  -> tanksim-6e5.1.2.4 (blocks) - Add structured simulation tracing with per-system deltas and configurable verbosity
  -> tanksim-6e5.6.5 (blocks) - Model chloride protection against nitrite hazard and integrate toxic stress accounting
  -> tanksim-6e5.6.4 (blocks) - Model reproduction success as a function of temperature, stability, condition, density, and osmotic stress
  -> tanksim-6e5.6.3 (blocks) - Add mineral budget and molt success/failure mechanics
  -> tanksim-6e5.6 (parent-child) - Phase 4 — shrimp life history, toxicity, and reproduction realism

Dependents:
  <- tanksim-6e5.7.3 (blocks) - Build literature-backed scenario envelopes and expected qualitative outcomes
  <- tanksim-6e5.7.5 (blocks) - Update developer docs, TUI/API messaging, and scientific-scope narrative

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Future-self notes:
- These scenarios should be educational. A player or developer reading them should understand what husbandry story they are meant to tell.
- Prefer a handful of sharp scenarios over a single giant "everything at once" regression.
- These will become inputs to the calibration phase (G3, G4), so keep them readable.
