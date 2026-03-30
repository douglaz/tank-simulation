○ tanksim-6e5.5.7 · Add habitat/geometry scenario tests and probes   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: geometry, habitats, phase-3, scientific-core, testing

Context:
- Habitat and geometry changes will touch many systems at once, so they need scenario-level tests that protect the intended stories.
- The tests should confirm that the new abstractions changed behavior for mechanistic reasons, not simply because parameters were retuned.

Deliverable:
- Scenario probes for:
  1. Biofilter scaling: bigger filter media → higher nitrifier capacity → faster cycling
  2. Light-depth: deep tank → less bottom light → different plant/algae behavior vs shallow tank
  3. Habitat-specific fouling: glass periphyton vs filter biofilm developing at different rates
  4. Substrate redox: mature planted substrate shows NO3 removal (denitrification) that unplanted substrate does not
  5. Equipment scaling: scaled-up tank with appropriately scaled equipment should show similar per-liter behavior
- Readable debug output for habitat-specific state during failures.
- Test notes that explain what each scenario is intended to demonstrate.

Likely touch points:
- scenario/test crates
- habitat/light/nitrogen debug helpers
- any snapshot extensions needed for verification

## Acceptance Criteria


Acceptance Criteria:
- at least one scenario demonstrates each major habitat/geometry story (5 scenarios listed in deliverables)
- each test scenario is a named test function with a doc comment explaining the ecological mechanism being validated
- failures dump habitat-specific state (per-habitat biomass, area, modifiers) via A2d tracing for diagnosis
- an e2e habitat/geometry test script runs all 5 probes, logs structured traces, produces a summary report, and exits non-zero on envelope violation
- the equipment-scaling probe includes a paired 1× vs 2× geometry run with proportionally scaled hardware, plants, and stocking; per-liter concentrations and qualitative outcomes stay within envelope across both sizes
- the tests remain resilient to reasonable retuning: assertions use directional comparisons (A > B) and envelope ranges, not exact values

Dependencies:
  -> tanksim-6e5.1.2.5 (blocks) - Build deterministic scientific-regression harness, e2e runner, and failure artifact capture
  -> tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  -> tanksim-6e5.1.2.4 (blocks) - Add structured simulation tracing with per-system deltas and configurable verbosity
  -> tanksim-6e5.5.6 (blocks) - Scale equipment, plant mass, and stocking defaults with tank geometry
  -> tanksim-6e5.5.5 (blocks) - Add depth/turbidity light attenuation and habitat-specific light exposure
  -> tanksim-6e5.5.4 (blocks) - Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks
  -> tanksim-6e5.5.3 (blocks) - Split periphyton and decomposer pools by habitat
  -> tanksim-6e5.5.2 (blocks) - Scale biofilter carrying capacity with habitat, media, flow, and oxygen
  -> tanksim-6e5.5 (parent-child) - Phase 3 — habitatized ecology, substrate redox, and geometry-aware scaling

Dependents:
  <- tanksim-6e5.7.3 (blocks) - Build literature-backed scenario envelopes and expected qualitative outcomes
  <- tanksim-6e5.7.5 (blocks) - Update developer docs, TUI/API messaging, and scientific-scope narrative

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Rationale:
- Habitat work adds hidden structure; tests are what keep that structure understandable.
- Focus on explanatory scenarios rather than giant kitchen-sink simulations.
- This bead is also the proof that geometry now matters through ecology instead of accidental totals.
