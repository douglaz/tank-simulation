○ tanksim-6e5.2.3 · Normalize nitrogen-cycle kinetics to concentration-based terms   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: chemistry, kinetics, nitrogen, phase-1, scientific-core

Context:
- nitrogen_cycle.rs currently uses several Monod or half-saturation terms expressed against absolute totals. All calls to monod_rate() (line 342-352) pass tank totals for substrate and total-based K_s values.
- This creates nonphysical tank-size behavior when two tanks share the same concentration but differ in total liters.

Concrete example from current defaults:
  With 1 mg/L N available and aob_k_tan_mg = 0.5:
  - In a 10L tank: S=10, K=0.5 -> factor = 10/(10+0.5) = 0.952
  - In a 100L tank: S=100, K=0.5 -> factor = 100/(100+0.5) = 0.995
  But the chemistry is identical. The 100L tank appears nearly fully saturated while the 10L tank shows meaningful limitation. This is backwards from reality.

Deliverable:
- Refactor AOB, NOB, comammox, and decomposer kinetics to use concentration-based inputs from the B2 helper layer.
- Update the shared Monod helper signature and all affected process/data parameter names so their units communicate concentration semantics clearly.
- Remove the hardcoded decomposer DO half-saturation constant and replace it with a named parameter.
- Set concentration-based starting values in code/data that preserve the intended ecological distinctions (for example comammox lower TAN K_s than AOB, NOB at least as DO-sensitive as AOB) while leaving broad retuning to B6.
- Keep the final cross-guild tank-size-independence proof in B3e as a separate guardrail bead.

Likely touch points:
- crates/tank_core/src/systems/nitrogen_cycle.rs (lines 76-93, 137-180, 213-219, 342-352)
- crates/tank_core/src/types/process.rs (lines 38,48,50,59,61,70,72)
- crates/tank_data/data/process/default.toml

## Acceptance Criteria


Acceptance Criteria:
- AOB, NOB, comammox, and decomposer limitation terms use concentration inputs from the helper layer rather than raw total tank masses.
- monod_rate() and the affected parameter names/docs/data files clearly use concentration semantics (for example *_per_l under the B1 policy).
- No hardcoded total-mass half-saturation constants remain in the normalized nitrogen/decomposer kinetics.
- Per-guild unit tests cover the concentration semantics and the key qualitative distinctions this bead is meant to preserve; B3e remains the final cross-guild volume-independence proof.

Dependencies:
  -> tanksim-6e5.2.2 (blocks) - Implement canonical concentration and compartment helper APIs
  -> tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  -> tanksim-6e5.2 (parent-child) - Phase 1A — units, concentration semantics, and kinetic normalization

Dependents:
  <- tanksim-6e5.2.3.5 (parent-child) - Add tank-size-independence regression test for all nitrogen kinetics
  <- tanksim-6e5.4.7 (blocks) - Validate nitrification-driven alkalinity depletion with the carbonate solver, expose diagnostics, and prepare the denitrification return path
  <- tanksim-6e5.5.4.2 (blocks) - Implement simplified denitrification in suboxic substrate zones
  <- tanksim-6e5.5.2 (blocks) - Scale biofilter carrying capacity with habitat, media, flow, and oxygen
  <- tanksim-6e5.3.4 (blocks) - Audit feeding, detritus, DOC, and mineralization bookkeeping end to end
  <- tanksim-6e5.2.6 (blocks) - Retune process parameters and scenario defaults after normalization

Comments:
  [2026-03-26 20:44 UTC] master: Canonicalization note: merged former B3a-B3d micro-beads into B3. Those child beads were all one codepath change in nitrogen_cycle.rs and split the implementation more than the planning. B3e remains separate as the proof and guardrail bead.
  [2026-03-26 13:46 UTC] backlog-planner: Scientific background:
- Monod (1949): μ = μ_max × [S]/(K_s + [S]) where [S] is substrate CONCENTRATION (mg/L), not total mass. K_s is the half-saturation constant — the concentration at which growth rate = half of maximum.
- For nitrifying bacteria in aquarium biofilters:
  - AOB K_s(TAN) ≈ 0.5–2.0 mg N/L, K_s(DO) ≈ 0.3–1.0 mg O2/L
  - NOB K_s(NO2) ≈ 0.2–1.0 mg N/L, K_s(DO) ≈ 0.5–1.5 mg O2/L (NOB are more sensitive to low O2)
  - Comammox K_s(TAN) is typically lower than AOB, ≈ 0.05–0.5 mg N/L (competitive advantage at low ammonia)
  - Decomposer K_s(DOC) ≈ 1–10 mg C/L
- These are CONCENTRATION values. The current code stores them as total-mass values, which only accidentally work for one specific tank size.

Rationale:
- This bead is the heart of the scientific refactor. Until it lands, larger tanks appear "more saturated" for nonphysical reasons.
- Do not simply divide by volume in a scattered way; route through the helper layer (B2) so the semantics stay centralized.
- When in doubt, choose the simpler scientifically credible formulation rather than a more elaborate but poorly constrained one.

Retuning expectations:
- After normalization, the old K_s values become meaningless. New values should be set to literature-reasonable concentration ranges and then tuned against the baseline envelopes (A3).
- This is expected. The point is to get the math right first, then tune for qualitative behavior.
  [2026-03-26 17:09 UTC] reviewer: Test strategy clarification: B3 normalization will change kinetic behavior. The key test strategy is:

1. B3e (tank-size-independence test) is the PRIMARY validation: identical concentrations in different volumes must produce identical rates.
2. Existing integration tests (cycling.rs, dissolved_oxygen.rs, etc.) WILL change their numeric outputs after normalization. This is expected.
3. Do NOT update existing test assertions during B3 — leave that for B6 (retuning). During B3, existing tests may need their exact assertions temporarily relaxed to envelope-style assertions, or marked with a // TODO: retune in B6 comment.
4. The A3 baseline envelopes are the stability anchor. If B3 breaks a qualitative behavior (e.g., cycling no longer completes), that's a real bug. If it changes exact timing, that's expected.

This ordering (B3 changes kinetics → B6 retunes parameters → tests re-tightened) is intentional and should not be shortcut.
