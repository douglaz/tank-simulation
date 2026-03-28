○ tanksim-6e5.7.1 · Add parameter provenance schema and loader support   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: architecture, data, phase-5, provenance, scientific-core

Context:
- The project goal is explicitly scientific, which means important parameters should eventually carry source and confidence metadata.
- The engine/data pipeline needs a minimal schema for provenance before parameter files can be enriched consistently.

Deliverable:
- Add a provenance-friendly parameter representation or adjacent metadata path that supports source, confidence, notes, and valid-range information.
- Ensure data loading remains ergonomic enough for everyday development and backward-compatible with existing TOML files that lack provenance.
- Decide how provenance is stored for code-defined defaults versus data-file parameters.
- Provide a developer-facing formatter or display helper so provenance can be surfaced in docs, reports, or debug output instead of remaining write-only metadata.

Possible schema for TOML parameter provenance:
  [parameter_name]
  value = 0.5
  unit = "mg N/L"
  source = "EPA 2013 ammonia criteria"
  confidence = "literature"    # literature | expert | heuristic | placeholder
  valid_range = [0.1, 5.0]
  notes = "K_s for AOB in biofilter context; may differ for free-living AOB"

Likely touch points:
- crates/tank_core/src/types/process.rs
- crates/tank_data/src/lib.rs
- data file layout and supporting docs

## Acceptance Criteria


Acceptance Criteria:
The project has a concrete place to store provenance/confidence metadata, and existing parameter files without provenance still load successfully.
Unit tests cover full roundtrip with all provenance fields, optional-field loading, confidence-level validation, and backward-compatible loading of legacy files.
Valid_range metadata can be checked without turning heuristic tuning into a hard load failure; out-of-range values produce a warning or similarly explicit diagnostic path.
A developer-facing formatter or display helper can render a provenance entry clearly enough for docs/debug output.
The schema is flexible enough for both chemistry and biology parameters plus code-defined defaults.

Dependencies:
  -> tanksim-6e5.1.1 (blocks) - Inventory current scientific semantics, units, invariants, and shortcuts
  -> tanksim-6e5.7 (parent-child) - Phase 5 — provenance, calibration, validation, and release narrative

Dependents:
  <- tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters

Comments:
  [2026-03-26 13:47 UTC] backlog-planner: Rationale:
- Provenance is how the project grows from "plausible sim" into "scientifically grounded sim."
- Keep the first schema simple and useful; it does not need to solve every citation-management problem on day one.
- Think about developer ergonomics so provenance becomes a habit, not a burden.

Early-start note:
- G1 depends only on A1 (semantics inventory). It can start as soon as A1 completes, well before the later science phases finish.
- This is intentional: having the provenance schema in place early means later phases (B6 retuning, D5 source water, F3 minerals) can attach provenance metadata as they go, rather than backfilling it all in Phase 5.
  [2026-03-26 17:10 UTC] reviewer: Unit test requirements:

1. test_provenance_schema_roundtrip: A parameter with full provenance metadata (value, unit, source, confidence, valid_range, notes) serializes and deserializes correctly via TOML.
2. test_provenance_optional_fields: Provenance fields (source, confidence, valid_range, notes) are all optional. A parameter with just value + unit loads fine.
3. test_confidence_levels_enum: Confidence levels (literature, expert, heuristic, placeholder) are validated on load. Unknown confidence level → clear error.
4. test_valid_range_enforcement: If valid_range is specified and value is outside it → warning (not error, since heuristic values may be intentionally outside literature range during tuning).
5. test_backward_compatible_loading: Existing TOML data files WITHOUT provenance metadata still load correctly (serde default or migration).
6. test_provenance_display: Provenance metadata can be formatted for developer display (e.g., "K_s_aob: 0.5 mg N/L [literature, EPA 2013]").
