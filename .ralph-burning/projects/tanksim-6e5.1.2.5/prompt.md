○ tanksim-6e5.1.2.5 · Build deterministic scientific-regression harness, e2e runner, and failure artifact capture   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: e2e, phase-0, scientific-core, testing, tooling, tracing

Context:
- Several later beads need scenario/e2e coverage, checkpoint assertions, and failure artifacts, but those concerns should assemble the shared budget/tracing surfaces instead of re-implementing them.
- Without a shared harness, later phases will either duplicate runner code or drift into incompatible artifact conventions, making scientific regressions slower to diagnose.
- This bead sits above A2b and A2d: it owns scenario orchestration, checkpoint/assertion conventions, and artifact packaging, not a second trace or budget API.

Deliverable:
- A reusable scientific-regression harness for seeded scenario runs and long-horizon e2e checks.
- Shared helpers/macros for scenario execution, checkpoint capture, envelope assertions, and structured artifact naming.
- Failure-artifact capture that packages the existing budget ledgers, trace output, scenario metadata, and assertion summaries into a deterministic temp/artifact path.
- A documented convention for local and CI execution, including an opt-in verbose mode for full trace dumps on success.
- Non-goals: do not create a parallel trace format, budget ledger, or per-system inspection API when A2b/A2d already provide one.

Likely touch points:
- crates/tank_core/tests/
- crates/tank_scenarios/tests/
- any shared test-support module or dedicated e2e test crate/scripts introduced by this bead

## Acceptance Criteria


Acceptance Criteria:
- at least one shared helper or macro exists for seeded scenario execution and envelope assertions so later beads do not hand-roll their own runners.
- failures emit a concise assertion summary plus paths to structured artifacts containing seed, scenario id, simulated hour, and relevant trace/budget dumps.
- a verbose mode (for example `TANK_E2E_VERBOSE=1`) emits full trace output on successful runs without changing assertions.
- at least two exemplar e2e scripts/tests use the harness end to end and pass in CI-friendly, non-interactive mode.
- the harness is deterministic: same seed + same scenario + same overrides -> identical checkpoints and artifact naming.
- the harness reuses the shared budget/tracing surfaces from A2b/A2d rather than inventing a second inspection or artifact schema for downstream beads.


Dependencies:
  -> tanksim-6e5.1.2.4 (blocks) - Add structured simulation tracing with per-system deltas and configurable verbosity
  -> tanksim-6e5.1.2.2 (blocks) - Add debug hooks and test helpers for budget inspection
  -> tanksim-6e5.1.2 (parent-child) - Add conservation/debug instrumentation and save-schema migration scaffolding

Dependents:
  <- tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  <- tanksim-6e5.6.6 (blocks) - Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes
  <- tanksim-6e5.5.7 (blocks) - Add habitat/geometry scenario tests and probes
  <- tanksim-6e5.4.6 (blocks) - Add carbonate regression tests and scenario probes
  <- tanksim-6e5.3.6 (blocks) - Add conservation diagnostics and regression tests for grazing and maintenance loops
  <- tanksim-6e5.7.4 (blocks) - Create calibration-report workflow comparing simulated outputs to target envelopes
