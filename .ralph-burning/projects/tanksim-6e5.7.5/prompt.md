○ tanksim-6e5.7.5 · Update developer docs, TUI/API messaging, and scientific-scope narrative   [● P2 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: api, communication, docs, phase-5, scientific-core, tui

Context:
- A more advanced simulation will raise user expectations, so the project needs clear messaging about what the model measures, what it estimates, and where it remains deliberately approximate.
- Internal docs should also explain how the new architecture is intended to be extended.

Deliverable:
- Developer docs covering the new scientific core concepts, especially units, habitats, carbonate state, and provenance.
- Updated TUI/API wording where labels or interpretations changed.
- A concise scope/limitations narrative suitable for future README or release-note use.

Likely touch points:
- spec.md
- TUI/API rendering/help text
- any new architecture or calibration docs from earlier beads

## Acceptance Criteria


Acceptance Criteria:
- a new contributor can understand the upgraded model without reverse-engineering the code
- user-facing wording does not overclaim scientific precision
- the project narrative aligns with the actual implemented fidelity

Dependencies:
  -> tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters
  -> tanksim-6e5.6.6 (blocks) - Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes
  -> tanksim-6e5.5.7 (blocks) - Add habitat/geometry scenario tests and probes
  -> tanksim-6e5.4.6 (blocks) - Add carbonate regression tests and scenario probes
  -> tanksim-6e5.2.7 (blocks) - Update snapshot/API chemistry semantics, display conversions, and TDS/conductivity labeling
  -> tanksim-6e5.7.4 (blocks) - Create calibration-report workflow comparing simulated outputs to target envelopes
  -> tanksim-6e5.7 (parent-child) - Phase 5 — provenance, calibration, validation, and release narrative

Dependents:
  <- tanksim-6e5.7.6 (blocks) - Run end-to-end rebalance pass and capture release narrative for the scientific-core upgrade

Comments:
  [2026-03-26 20:44 UTC] master: Correction: former B8 was merged into canonical bead B7 (tanksim-6e5.2.7). The user-facing chemistry semantics, display conversions, and TDS/conductivity honesty dependency still exists, but it now lands through B7 rather than a separate bead.
  [2026-03-26 13:47 UTC] backlog-planner: Rationale:
- Good docs are part of scientific honesty.
- This bead also reduces future rework by making extension points and naming conventions explicit.
- Keep the tone educational: the simulator should teach players what it knows and what it is still approximating.
