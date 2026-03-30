○ tanksim-6e5.7.3 · Build literature-backed scenario envelopes and expected qualitative outcomes   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: phase-5, research, scenarios, scientific-core, validation

Context:
- By this point the simulator should have richer chemistry, habitats, and shrimp outcomes, but it still needs explicit target stories to validate against.
- Those targets should be framed as scenario envelopes and causal expectations rather than fantasy exact replicas of every real tank.
- The earlier baseline envelopes (A3) captured what the v0.1 model does. This bead captures what the upgraded model should do, backed by literature or strong expert reasoning.
- Each phase added regression tests for its own domain (B3e, C6, D6, E7, F6). This bead synthesizes them into a coherent validation suite that serves as the project's scientific accountability layer.

Deliverable:
- A curated set of validation scenarios (6-10) with literature-backed or otherwise justified expected qualitative outcomes. At minimum:
  1. Cycling timeline: fishless cycling to completion in 4-8 weeks (Hovanec & DeLong 1996, Timmons & Ebeling 2010)
  2. Aeration effects: aerated tank shows higher DO and higher pH than unaerated (Colt 2006)
  3. Planted tank day/night: pH swing of 0.2-1.0 pH units in heavily planted tanks (Brewer & Goldman 1976)
  4. Source-water differentiation: RO water vs hard tap gives pH difference > 1.0 unit (basic carbonate chemistry)
  5. Shrimp breeding: Neocaridina breed at 22-28°C, suppress above 30°C (Tropea et al. 2015)
  6. Algae-plant competition: high-light + low-nutrient favors algae over slow-growing plants (Tilman 1982)
  7. Nitrate removal: planted tank with mature substrate shows lower steady-state NO3 than bare-bottom (via denitrification)
  8. Stocking density: overcrowding + overfeeding leads to water quality crash within weeks
- For each scenario: a one-paragraph rationale, literature citation or expert justification, expected qualitative outcome (directional, not exact), confidence rating (high/medium/low), and acceptable envelope bounds.
- Clear separation between "validated directionally" (high confidence) and "still heuristic" (low confidence).
- A validation report template that can be auto-populated by the calibration workflow (G4).

Likely touch points:
- docs/validation_scenarios.md (new document)
- scenario data/tests from earlier phases (A3, D6, E7, F6)
- provenance metadata from G2
- calibration workflow from G4

## Acceptance Criteria


Acceptance Criteria:
- at least 6 validation scenarios are documented with literature citations or expert justification
- each scenario includes a rationale, expected qualitative outcome, confidence rating, and envelope bounds
- the validation language distinguishes directionally validated claims from heuristic claims
- the scenario set covers chemistry, ecology, and animal behavior
- the validation document can be cited by future beads and release notes as the scientific accountability reference
- future tuning can reuse the same envelope definitions without rewriting the validation rationale
- a developer can run the validation suite and get a pass/fail summary for each scenario

Dependencies:
  -> tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  -> tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters
  -> tanksim-6e5.6.6 (blocks) - Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes
  -> tanksim-6e5.5.7 (blocks) - Add habitat/geometry scenario tests and probes
  -> tanksim-6e5.4.6 (blocks) - Add carbonate regression tests and scenario probes
  -> tanksim-6e5.3.6 (blocks) - Add conservation diagnostics and regression tests for grazing and maintenance loops
  -> tanksim-6e5.2.6 (blocks) - Retune process parameters and scenario defaults after normalization
  -> tanksim-6e5.7 (parent-child) - Phase 5 — provenance, calibration, validation, and release narrative

Dependents:
  <- tanksim-6e5.7.4 (blocks) - Create calibration-report workflow comparing simulated outputs to target envelopes

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Rationale:
- This bead turns scattered scientific intent into an explicit validation suite.
- The success criterion is causal plausibility, not perfect prediction of every aquarium.
- Keep the validation set small enough to maintain and rich enough to matter.
