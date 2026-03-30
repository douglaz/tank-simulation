○ tanksim-6e5.1.2.2 · Add debug hooks and test helpers for budget inspection   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: api, phase-0, scientific-core, testing

Context:
- Budget tracking (A2a) records per-element sources and sinks during a tick, but that data is only useful if tests can inspect it and developers can enable it during debugging.
- The existing test suite uses direct state assertions (e.g., "ammonia went down") but has no budget-level inspection (e.g., "nitrogen_cycle consumed 0.003 mg N of ammonia this tick").
- Without ergonomic debug hooks, later beads will hand-roll their own inspection logic, leading to inconsistent and fragile conservation checks.
- This bead bridges A2a (raw budget tracking) and A2d (structured tracing) by making budget data accessible to both tests and interactive debugging.

Deliverable:
- Test helper functions that wrap engine stepping and return budget deltas alongside the final state, so a test can say step_and_inspect(state, 1) and get back both the new state and the per-element budget for that tick.
- A budget assertion API that lets tests write one-liners like assert_n_conserved(&budget, tolerance) or assert_budget_balanced(&budget, Element::Nitrogen, 1e-6) instead of manually summing pools.
- Optional debug output (configurable via feature flag, env var, or test configuration) that surfaces budget summaries per tick to stderr or a log target — useful for interactive debugging without modifying test code.
- Clear documentation of the API so future test authors naturally reach for budget helpers rather than reimplementing pool arithmetic.

Likely touch points:
- crates/tank_core/tests/ (new test utility module or extension of existing helpers)
- crates/tank_core/src/engine.rs (expose budget data from step functions)
- crates/tank_core/src/types/state.rs (budget ledger access)

## Acceptance Criteria


Acceptance Criteria:
- at least one existing test is retrofitted to also check budget invariants using the new helpers, demonstrating the API is usable
- debug output can be enabled via feature flag or env var (for example TANK_BUDGET_DEBUG=1) without changing test code or assertions
- the assertion API covers at minimum: N conservation, C conservation, O2 demand balance, and a generic per-element check
- a test can inspect which system produced which delta for a given element in a given tick
- the helpers compile without overhead when budget tracking is disabled
- the API is ergonomic enough that a budget assertion is a single function call, not a multi-line manual calculation

Dependencies:
  -> tanksim-6e5.1.2.1 (blocks) - Implement per-tick mass budget tracking for N, C, and O2
  -> tanksim-6e5.1.2 (parent-child) - Add conservation/debug instrumentation and save-schema migration scaffolding

Dependents:
  <- tanksim-6e5.1.2.5 (blocks) - Build deterministic scientific-regression harness, e2e runner, and failure artifact capture
  <- tanksim-6e5.3.6 (blocks) - Add conservation diagnostics and regression tests for grazing and maintenance loops

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Design note:
- Consider a test wrapper like: let (state, budget) = engine.step_hours_with_budget(n);
- Budget assertions should use a configurable tolerance (e.g., 1e-6 mg) to account for floating-point arithmetic.
- The debug output should identify the system responsible for the largest budget imbalance, making it easy to locate conservation bugs.
