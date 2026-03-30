○ tanksim-6e5.3.6 · Add conservation diagnostics and regression tests for grazing and maintenance loops   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: diagnostics, mass-balance, phase-1, scientific-core, testing

Context:
- Once the routing changes (C2-C5 and C7) land, the project needs repeatable proof that matter is no longer silently disappearing in normal closed-loop cases.
- The same test harness should also distinguish explicit exports such as water changes and trim-and-remove from in-tank routing such as death/senescence -> detritus.

Deliverable:
- Regression tests for shrimp grazing, microfauna consumption, feeding, death/senescence routing, decomposition, and trimming semantics.
- Diagnostics that report conserved-vs-exported deltas clearly enough to debug failures.
- Tolerances appropriate for deterministic floating-point simulation rather than exact arithmetic fantasy.

Test scenarios to cover:
  1. Closed-system grazing: shrimp eat periphyton, verify total N conserved (TAN increases, periphyton decreases, feces appear).
  2. Closed-system feeding: add feed, verify total N/C conserved through the whole decomposition chain.
  3. Closed-system mortality/senescence: shrimp death or plant/algae senescence routes biomass to detritus, and total N/C stay conserved unless a named export action removes the biomass.
  4. Trim-and-remove: verify biomass exported, total N decreases by exactly the exported amount.
  5. Trim-and-leave: verify biomass -> detritus transfer, total N conserved.
  6. Water change: verify dilution + replacement chemistry is mass-balanced.

Likely touch points:
- crates/tank_core/src/invariants.rs
- test modules across tank_core and scenario crates
- snapshot/debug helpers from A2

## Acceptance Criteria


Acceptance Criteria:
Closed-loop grazing, feeding, and mortality/senescence cases stay within explicit N and C tolerances (< 1e-6 mg per tick for deterministic runs).
Export actions are tracked as exports, not as unexplained mass loss.
The tests are easy to extend when later shrimp, habitat, or chemistry work lands.
Each of the 6 test scenarios (grazing, feeding, mortality/senescence, trim-remove, trim-leave, water change) is a separate named test function with a descriptive doc comment explaining the conservation story being tested.
All conservation tests log per-system deltas via A2d tracing at detail verbosity; failures dump the full trace to a file for debugging.
An e2e conservation script runs all 6 scenarios in sequence, produces a summary report (pass/fail per scenario with delta magnitudes), and exits non-zero on any failure.

Dependencies:
  -> tanksim-6e5.3.7 (blocks) - Implement organism death → detritus routing for shrimp, plants, and algae
  -> tanksim-6e5.1.2.5 (blocks) - Build deterministic scientific-regression harness, e2e runner, and failure artifact capture
  -> tanksim-6e5.1.2.4 (blocks) - Add structured simulation tracing with per-system deltas and configurable verbosity
  -> tanksim-6e5.1.2.2 (blocks) - Add debug hooks and test helpers for budget inspection
  -> tanksim-6e5.3 (parent-child) - Phase 1B — mass conservation and husbandry action semantics
  -> tanksim-6e5.3.5 (blocks) - Split plant trimming into export vs leave-cuttings actions
  -> tanksim-6e5.3.4 (blocks) - Audit feeding, detritus, DOC, and mineralization bookkeeping end to end

Dependents:
  <- tanksim-6e5.7.3 (blocks) - Build literature-backed scenario envelopes and expected qualitative outcomes

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Future-self notes:
- This bead is the contract that keeps future feature work honest. Any change that breaks conservation should break these tests.
- Prefer a few crisp invariant tests plus readable debug output over a giant opaque golden-file.
- The real success condition is fast diagnosis when a future change breaks conservation.
- Use the budget helpers from A2a-b to make assertions concise: assert!(budget.net_n_delta().abs() < 1e-6, "N budget violated: {}", budget.explain());
