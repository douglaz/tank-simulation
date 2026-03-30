○ tanksim-6e5.7.2 · Attach provenance and confidence metadata to high-value chemistry and ecology parameters   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: biology, chemistry, phase-5, provenance, scientific-core

Context:
- After the schema exists, the most consequential parameters should be annotated first rather than trying to label everything at once.
- This bead focuses on the parameters most likely to matter for validation, tuning, and scientific claims.

Deliverable:
- Add provenance/confidence metadata to high-value parameters across nitrogen kinetics, carbonate chemistry, habitat/biofilter scaling, and shrimp reproduction/toxicology.
- Leave clear notes where a value is provisional or expert-curated rather than strongly literature-backed.
- Ensure the metadata is visible where future tuning work will notice it, for example in data comments, docs, or developer-facing summaries.
- Cover at least one representative file or data source for each targeted parameter family so later validation/calibration beads can cite the metadata directly.

Priority parameters to annotate (at minimum):
  - AOB/NOB/comammox K_s values (nitrogen kinetics)
  - Carbonate equilibrium constants (pKa1, pKa2, K_H)
  - Shrimp reproduction temperature curve parameters
  - Shrimp molt mineral thresholds
  - Chloride-nitrite protection factor
  - Plant and algae growth rate maxima
  - Denitrification rate parameters

Likely touch points:
- process data files
- shrimp/source-water data files
- any new docs or reports generated in Phase 5

## Acceptance Criteria


Acceptance Criteria:
Each listed priority parameter family has provenance metadata attached or an explicit TODO note explaining why the parameter still lives in code and where its provenance will be recorded.
Confidence/uncertainty is explicit, not implied: literature-backed, expert-curated, heuristic, and placeholder values are distinguishable in the data.
Representative enriched files from the chemistry, ecology, and shrimp tracks load through the G1 schema without special-case hacks.
A developer-facing summary distinguishes strong anchors from provisional approximations so future tuning work can focus on the weakest assumptions first.
Future maintainers can tell which parameters are strong anchors versus temporary approximations, and later beads (G3/G4/G5) can cite these metadata entries directly.

Dependencies:
  -> tanksim-6e5.4.3 (blocks) - Integrate CO2 gas exchange with aeration, surface exchange, and ambient coupling
  -> tanksim-6e5.4.2 (blocks) - Implement explicit carbonate equilibrium and pH solver
  -> tanksim-6e5.7.1 (blocks) - Add parameter provenance schema and loader support
  -> tanksim-6e5.6.5 (blocks) - Model chloride protection against nitrite hazard and integrate toxic stress accounting
  -> tanksim-6e5.6.4 (blocks) - Model reproduction success as a function of temperature, stability, condition, density, and osmotic stress
  -> tanksim-6e5.6.3 (blocks) - Add mineral budget and molt success/failure mechanics
  -> tanksim-6e5.5.4.2 (blocks) - Implement simplified denitrification in suboxic substrate zones
  -> tanksim-6e5.5.2 (blocks) - Scale biofilter carrying capacity with habitat, media, flow, and oxygen
  -> tanksim-6e5.4.5 (blocks) - Upgrade source-water profiles to carry carbonate-relevant inputs and differentiated defaults
  -> tanksim-6e5.2.6 (blocks) - Retune process parameters and scenario defaults after normalization
  -> tanksim-6e5.7 (parent-child) - Phase 5 — provenance, calibration, validation, and release narrative

Dependents:
  <- tanksim-6e5.7.3 (blocks) - Build literature-backed scenario envelopes and expected qualitative outcomes
  <- tanksim-6e5.7.6 (blocks) - Run end-to-end rebalance pass and capture release narrative for the scientific-core upgrade
  <- tanksim-6e5.7.5 (blocks) - Update developer docs, TUI/API messaging, and scientific-scope narrative

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Future-self notes:
- Start with the load-bearing parameters, not every constant in the codebase.
- Be honest about uncertainty. A transparent provisional value is better than an unlabeled "scientific-looking" number.
- Use this bead to identify where more literature review is still needed.
