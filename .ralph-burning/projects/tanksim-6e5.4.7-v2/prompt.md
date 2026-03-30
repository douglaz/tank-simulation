○ tanksim-6e5.4.7 · Validate nitrification-driven alkalinity depletion with the carbonate solver, expose diagnostics, and prepare the denitrification return path   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: alkalinity, carbonates, nitrogen, phase-2, scientific-core

Context:
- Existing code already deducts alkalinity during AOB + comammox TAN oxidation, so this bead is NOT about inventing a second nitrification alkalinity charge.
- The real gap is end-to-end scientific verification after B3 and D2: prove the new carbonate solver turns that alkalinity loss into believable pH decline, expose the deltas in A2 budget/tracing, and prepare the same accounting path for later denitrification return.
- Soft-vs-hard source-water buffering is one of the user-visible stories this phase must teach, so this bead owns the tests and diagnostics that make that story explicit.

Deliverable:
- Verify the existing nitrification alkalinity accounting still applies exactly once per mg N after nitrogen normalization and carbonate-solver integration; refactor constant naming if needed so the stoichiometry is explicit and documented.
- Surface alkalinity deltas in budget tracking (A2a) and structured tracing (A2d), including per-tick attribution to nitrification and the future denitrification return path.
- Add regression and scenario coverage for nitrification-driven pH decline, including a soft-water vs hard-water buffering contrast under the same ammonia load.
- Define the accounting interface that E4b will use to add alkalinity back during denitrification, even if the denitrification implementation lands later.
- Non-goal: do not re-implement a second standalone nitrification alkalinity deduction if the existing charge is already correct.

Key stoichiometry to preserve:
  NITRIFICATION_ALK_MEQ_PER_MG_N = 2.0 / 14.007 ≈ 0.1428 meq/mg N
  DENITRIFICATION_ALK_MEQ_PER_MG_N ≈ 0.0714 meq/mg N
  The full nitrification charge belongs to TAN oxidation overall and must not be charged a second time at the NOB step unless an explicit split is documented.

Likely touch points:
- crates/tank_core/src/systems/nitrogen_cycle.rs (named constants, accounting attribution)
- crates/tank_core/src/types/process.rs and/or crates/tank_core/src/types/water.rs (constant naming or surfacing)
- budget/tracing modules from A2a/A2d
- chemistry/system tests and scenario probes

## Acceptance Criteria


Acceptance Criteria:
- the existing nitrification alkalinity charge is represented by named constant(s) and applied once per mg N nitrified, not independently charged at both AOB and NOB steps.
- with D2 in place, a cycling scenario without water changes or denitrification shows pH decline as alkalinity is consumed.
- a soft-water (low KH) case declines faster than a hard-water (high KH) case under the same ammonia load, demonstrating the buffering story rather than just a generic pH drift.
- a closed-system test verifies: initial_alkalinity - final_alkalinity ≈ total_N_nitrified × NITRIFICATION_ALK_MEQ_PER_MG_N (within explicit tolerance).
- alkalinity deltas are visible in budget tracking and structured tracing output with enough detail to attribute the change to nitrification.
- the accounting pathway is ready for later denitrification alkalinity return without redesigning the bookkeeping surface.


Dependencies:
  -> tanksim-6e5.4.2 (blocks) - Implement explicit carbonate equilibrium and pH solver
  -> tanksim-6e5.2.3 (blocks) - Normalize nitrogen-cycle kinetics to concentration-based terms
  -> tanksim-6e5.2.2 (blocks) - Implement canonical concentration and compartment helper APIs
  -> tanksim-6e5.1.2.4 (blocks) - Add structured simulation tracing with per-system deltas and configurable verbosity
  -> tanksim-6e5.1.2.1 (blocks) - Implement per-tick mass budget tracking for N, C, and O2
  -> tanksim-6e5.4 (parent-child) - Phase 2 — carbonate chemistry, CO2 exchange, and pH realism

Dependents:
  <- tanksim-6e5.5.4.2 (blocks) - Implement simplified denitrification in suboxic substrate zones
  <- tanksim-6e5.4.6 (blocks) - Add carbonate regression tests and scenario probes

Comments:
  [2026-03-26 14:45 UTC] reviewer: Why this bead was added:
- The original backlog had a significant scientific gap: the carbonate solver (D2) and gas exchange (D3) were implemented, but nitrification's effect on alkalinity — the most common real-world driver of pH decline in aquaria — was missing.
- Without this coupling, a cycling tank would show the correct initial pH from the carbonate solver but would never show the gradual pH decline that every aquarist observes in practice.
- This is especially important for the source-water differentiation story (D5): soft water tanks crash in pH faster than hard water tanks BECAUSE of alkalinity consumption by nitrification. The simulator cannot demonstrate this without this bead.

Real-world significance:
- A typical cycling tank with 2 mg/L TAN being nitrified per day in 40L of water consumes about 14.3 mg CaCO3 of alkalinity per day.
- In a soft water tank (KH 2 = ~36 mg/L CaCO3 equivalent alkalinity), this means the entire alkalinity buffer is exhausted in about 2.5 days without water changes. pH would crash.
- In a hard water tank (KH 8 = ~143 mg/L CaCO3), the same nitrification load takes ~10 days to exhaust alkalinity.
- This is one of the most important practical lessons the simulator should teach.

Interaction with later beads:
- D6 (carbonate tests) should include a test for nitrification-driven pH decline.
- E4b (denitrification) will add alkalinity back, partially offsetting nitrification. When E4b lands, the stoichiometric constant should be wired symmetrically.
- G2 (provenance) should annotate the alkalinity stoichiometric constants with their source.

Ordering note:
- This bead depends on B3 (nitrogen normalization) because the nitrification rates must be concentration-based before the alkalinity coupling is trustworthy. Coupling wrong-unit nitrification rates to alkalinity would produce wrong alkalinity depletion.
- It also depends on D2 (carbonate solver) so that pH responds mechanistically to the alkalinity change.
  [2026-03-26 15:51 UTC] reviewer: Correction and implementation note:
- The earlier example value of 14.3 mg CaCO3/day was a bad total. A load of 2 mg/L/day across 40L is 80 mg N/day, which corresponds to about 571 mg CaCO3/day at 7.14 mg CaCO3 per mg N.
- That larger value is the one consistent with the later "~2.5 days to exhaust KH 2" note.
- Also, the full 0.1428 meq/mg N nitrification charge belongs to the NH4 oxidation leg overall; do not apply it a second time at the NOB step unless you explicitly split the constant and document the split.

  [2026-03-26 16:22 UTC] reviewer: IMPORTANT REVISION — existing implementation acknowledgment:

The codebase ALREADY implements alkalinity consumption during nitrification:
- process.rs defines alk_per_mg_n = 0.1428 (meq/mg N)
- nitrogen_cycle.rs lines 273-283 deduct alkalinity after AOB+comammox TAN oxidation
- NOB does NOT consume additional alkalinity (correct behavior)

What D7 actually needs to deliver (revised scope):
1. VERIFY the existing alkalinity consumption is correctly coupled to the NEW carbonate solver from D2. The old pH shortcut (6.3 + log10(alk) - log10(DIC)) didn't properly translate alkalinity depletion into mechanistic pH behavior. The new solver should do this naturally — D7 verifies it.
2. ADD denitrification alkalinity PRODUCTION when E4b lands (stoichiometric constant + accounting pathway).
3. ADD budget tracking visibility for alkalinity deltas (wire into A2a ledger and A2d tracing).
4. ADD the integration/regression tests specified in the acceptance criteria (cycling pH decline story, soft-vs-hard buffering contrast).
5. VERIFY that the existing alk_per_mg_n constant matches the named NITRIFICATION_ALK_MEQ_PER_MG_N pattern — refactor if needed for consistency.

The bead description's framing ("Wire nitrification alkalinity effects") is misleading since the wiring already exists. The real work is verification, test coverage, tracing visibility, and preparing the denitrification return path.

