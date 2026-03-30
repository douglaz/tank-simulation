○ tanksim-6e5.1.2.4 · Add structured simulation tracing with per-system deltas and configurable verbosity   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: logging, phase-0, scientific-core, testing, tracing

Context:
- Budget tracking (A2a) answers whether a tick balanced, but later phases also need to know which system changed which pool and when across both short tests and long scenario runs.
- Structured tracing should be a reusable substrate for scenario probes, CI artifacts, and future domain-specific diagnostics rather than something each later bead reinvents.
- This bead is intentionally separate from A2b budget assertion helpers and A2e harness/artifact orchestration; keeping those ownership lines clean preserves parallel work after A2a.

Deliverable:
- A reusable SimTrace or TickLog facility that records, per tick and per system:
  - system name (for example nitrogen_cycle, shrimp, chemistry)
  - pools modified and their deltas
  - key intermediate values worth debugging
  - any events generated
- Configurable verbosity levels: off (default gameplay), summary (per-tick totals), detail (per-system deltas), trace (intermediates).
- Output modes: in-memory (for test assertions), file (JSON-lines for CI archival), stderr (for interactive debugging).
- A stable trace schema/API that downstream beads and the A2e harness can consume directly without inventing a second trace format.
- Non-goals: do not own budget-balance assertion helpers, budget-centric convenience APIs, or scenario artifact naming / runner conventions.

Likely touch points:
- crates/tank_core/src/engine.rs (wrap system calls with trace points)
- new module: crates/tank_core/src/tracing.rs (or similar)
- crates/tank_core/tests/ (test helpers that enable tracing and inspect output)

## Acceptance Criteria


Acceptance Criteria:
- a test can run N ticks at detail verbosity and inspect per-system deltas for any pool.
- file output mode produces valid JSON-lines that can be piped to jq or diffed across runs.
- tracing is fully opt-in: normal gameplay has zero overhead (no allocation, no I/O).
- at least one existing integration test is retrofitted to use tracing output for its assertions.
- later harness/probe beads can reuse the same trace schema/API directly instead of inventing ad hoc per-domain trace formats.


Dependencies:
  -> tanksim-6e5.1.2.1 (blocks) - Implement per-tick mass budget tracking for N, C, and O2
  -> tanksim-6e5.1.2 (parent-child) - Add conservation/debug instrumentation and save-schema migration scaffolding

Dependents:
  <- tanksim-6e5.1.2.5 (blocks) - Build deterministic scientific-regression harness, e2e runner, and failure artifact capture
  <- tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  <- tanksim-6e5.4.7 (blocks) - Validate nitrification-driven alkalinity depletion with the carbonate solver, expose diagnostics, and prepare the denitrification return path
  <- tanksim-6e5.6.6 (blocks) - Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes
  <- tanksim-6e5.5.7 (blocks) - Add habitat/geometry scenario tests and probes
  <- tanksim-6e5.4.6 (blocks) - Add carbonate regression tests and scenario probes
  <- tanksim-6e5.3.6 (blocks) - Add conservation diagnostics and regression tests for grazing and maintenance loops
  <- tanksim-6e5.3.4 (blocks) - Audit feeding, detritus, DOC, and mineralization bookkeeping end to end
  <- tanksim-6e5.7.4 (blocks) - Create calibration-report workflow comparing simulated outputs to target envelopes

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- This is the infrastructure that makes "great, detailed logging" possible for every later bead's tests.
- Without structured tracing, debugging a conservation violation means adding ad hoc printlns, which are removed after debugging and lost. Structured tracing makes diagnostic output permanent and reusable.
- The JSON-lines format enables automated regression comparison: run a scenario before and after a change, diff the traces, and see exactly which system's behavior changed.

Design guidance:
- Keep it simple: a Vec<TickEntry> where TickEntry = { tick: u32, system: &str, deltas: HashMap<&str, f64>, notes: Vec<String> }.
- For zero-cost when off: use a trait with a no-op implementation, or a compile-time feature flag. Prefer runtime flag for test flexibility.
- The tracing should NOT replace the budget tracking from A2a — they serve different purposes. Budget tracking checks invariants (pass/fail). Tracing explains what happened (diagnostic).
- Consider making the engine accept an optional &mut dyn Tracer so tests can inject a recording tracer while gameplay uses a no-op.
