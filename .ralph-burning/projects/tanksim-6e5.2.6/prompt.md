○ tanksim-6e5.2.6 · Retune process parameters and scenario defaults after normalization   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: data, phase-1, retuning, scenarios, scientific-core

Context:
- Once kinetics are normalized (B3, B4, B5), old parameter values are no longer guaranteed to mean the same thing.
- Data files and scenario presets need an explicit retuning pass so the simulator remains stable and interpretable.

Deliverable:
- Review and retune relevant values in process, plant, shrimp, source-water, and scenario data files after the semantic changes from B3-B5.
- Update comments in data files where unit meanings or intended ranges changed.
- Confirm that the main shipped scenarios still tell useful stories after retuning.

Likely touch points:
- crates/tank_data/data/process/default.toml
- crates/tank_data/data/plants/*.toml
- crates/tank_data/data/scenarios/*.toml
- any supporting docs created in A1/A3

## Acceptance Criteria


Acceptance Criteria:
- parameter files no longer silently carry pre-refactor assumptions
- all shipped scenarios (nano_cycle, medium_planted, warm_room) pass their A3 envelope tests with the new parameter values
- changed semantics are documented in the data files where future maintainers will actually see them
- trace output from the retuning runs is archived for comparison in later phases

Dependencies:
  -> tanksim-6e5.2.3 (blocks) - Normalize nitrogen-cycle kinetics to concentration-based terms
  -> tanksim-6e5.2 (parent-child) - Phase 1A — units, concentration semantics, and kinetic normalization
  -> tanksim-6e5.2.5 (blocks) - Normalize algae kinetics around concentration, light, and temperature interactions
  -> tanksim-6e5.2.4 (blocks) - Normalize plant nutrient uptake and growth limitation semantics

Dependents:
  <- tanksim-6e5.7.3 (blocks) - Build literature-backed scenario envelopes and expected qualitative outcomes
  <- tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters
  <- tanksim-6e5.5.6 (blocks) - Scale equipment, plant mass, and stocking defaults with tank geometry
  <- tanksim-6e5.4.5 (blocks) - Upgrade source-water profiles to carry carbonate-relevant inputs and differentiated defaults

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Future-self notes:
- Retuning is part of the scientific refactor, not an afterthought.
- Be explicit about whether a change is "new science" versus "compensating for old unit mistakes."
- Capture why tuned values moved, especially if they are temporary envelopes pending later habitat or carbonate work.
- Use the baseline envelopes from A3 as the tuning target: "nano_cycle should still show cycling completion within 500-800 hours" is more useful than "make the numbers look right."
- Do not smuggle geometry/equipment scaling fixes into Phase 1 retuning constants. Fair 1× vs 2× scaled comparisons belong in E6/E7, where the scaling rules themselves are explicit and testable.

Process:
1. Run each shipped scenario with the old parameters under new kinetics.
2. Compare against A3 envelopes.
3. Adjust K_s values to literature-reasonable concentration ranges.
4. Iterate until scenarios tell qualitatively correct stories.
5. Document the new values with rationale and confidence level.
