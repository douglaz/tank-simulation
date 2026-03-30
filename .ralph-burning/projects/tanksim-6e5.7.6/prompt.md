○ tanksim-6e5.7.6 · Run end-to-end rebalance pass and capture release narrative for the scientific-core upgrade   [● P2 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: calibration, phase-5, release, retuning, scientific-core

Context:
- After all major scientific-core changes land, the simulator will need a final rebalance and a clear statement of what improved.
- This is the bead that turns a pile of refactors into a coherent "next version."

Deliverable:
- One final pass over shipped scenarios and high-value parameters after all earlier beads are merged.
- A concise release narrative summarizing what changed scientifically, what new behaviors should emerge, and what still remains out of scope.
- A list of follow-on backlog candidates discovered during the pass.

Likely touch points:
- scenario data
- parameter files
- calibration outputs
- release/docs notes

## Acceptance Criteria


Acceptance Criteria:
- the upgraded simulator has a documented post-refactor tuning state
- the release narrative ties changes back to the project's overarching scientific goals
- obvious follow-on work is captured cleanly instead of being lost in memory

Dependencies:
  -> tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters
  -> tanksim-6e5.7.5 (blocks) - Update developer docs, TUI/API messaging, and scientific-scope narrative
  -> tanksim-6e5.7.4 (blocks) - Create calibration-report workflow comparing simulated outputs to target envelopes
  -> tanksim-6e5.7 (parent-child) - Phase 5 — provenance, calibration, validation, and release narrative

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Future-self notes:
- This is the "make it legible" bead, not just a tuning sweep.
- Use the calibration workflow (G4) and validation scenarios (G3) rather than intuition-only tweaks.
- Capture what was intentionally postponed so future roadmaps can pick it up without re-opening old debates.
