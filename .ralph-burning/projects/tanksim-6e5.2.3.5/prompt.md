○ tanksim-6e5.2.3.5 · Add tank-size-independence regression test for all nitrogen kinetics   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: kinetics, nitrogen, phase-1, scientific-core, testing

Context:
- After B3 normalizes all four nitrogen/decomposer guilds to concentration semantics, the project needs a definitive regression proving the fix actually removed tank-size artifacts: same concentration in different volumes -> same Monod factor and same per-liter rate.

Deliverable:
- A regression test that:
  1. Creates two tank states with different volumes (for example 10L and 100L) but identical concentrations of TAN, NO2, DOC, and DO.
  2. Runs one tick of nitrogen_cycle on each.
  3. Asserts that the Monod limitation factors are equal within floating-point tolerance.
  4. Asserts that the concentration changes per hour are equal (not just the raw totals).
- The test should be named clearly (for example test_concentration_kinetics_are_volume_independent) and include comments explaining what it protects.

## Acceptance Criteria


Acceptance Criteria:
- the test passes after B3 concentration normalization is complete
- the test would have failed against the pre-refactor code (verify this before the refactor if possible)
- the test is easy to extend when later systems are also normalized

Dependencies:
  -> tanksim-6e5.2.3 (parent-child) - Normalize nitrogen-cycle kinetics to concentration-based terms

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Test design notes:
- Use a controlled setup: disable plant/algae/shrimp/microfauna systems to isolate nitrogen kinetics.
- Set identical concentrations but 10× volume difference: this maximizes the signal if totals leak through.
- Assert both the Monod factor AND the per-liter concentration delta. A test that only checks the factor could miss volume leaks in the rate-to-mass conversion step.
- Consider also testing an edge case: very small tank (1L) and very large tank (1000L) at the same concentration to stress the extremes.
- This test becomes a permanent guardrail: any future code that accidentally reintroduces total-mass kinetics will break it.
