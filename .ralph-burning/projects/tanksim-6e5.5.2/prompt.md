○ tanksim-6e5.5.2 · Scale biofilter carrying capacity with habitat, media, flow, and oxygen   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: biofilter, habitats, nitrogen, phase-3, scientific-core

Context:
- Nitrifier capacity is currently capped by a logistic carrying capacity (nitrogen_cycle.rs) that doesn't scale with media area, flow rate, or oxygen availability.
- This prevents nano and larger tanks from differing in mature filter behavior for the right mechanistic reasons.

Deliverable:
- Replace the fixed nitrifier capacity assumption with a habitat-aware formulation tied to media area/volume, flow, and oxygen exposure.
- Keep the first pass simple and inspectable: carrying_capacity = base_density × habitat_area × flow_modifier × o2_modifier.
- Update comments/data so the new carrying-capacity semantics are obvious.

Likely touch points:
- crates/tank_core/src/systems/nitrogen_cycle.rs (carrying capacity section)
- crates/tank_core/src/types/hardware.rs
- habitat state introduced in E1
- related process parameters/data

## Acceptance Criteria


Acceptance Criteria:
- biofilter capacity meaningfully responds to habitat/media/flow inputs
- no single hidden constant stands in for every filter setup
- unit test: two identical tanks with different filter media area → higher-area filter achieves higher nitrifier biomass at maturity
- unit test: same media area but different flow rates → carrying capacity scales with flow
- integration test: nano tank with sponge filter vs medium tank with canister filter → the medium tank reaches cycling maturity faster per the cycling.rs envelope pattern

Dependencies:
  -> tanksim-6e5.2.3 (blocks) - Normalize nitrogen-cycle kinetics to concentration-based terms
  -> tanksim-6e5.5.1 (blocks) - Introduce habitat registry and colonizable-area model
  -> tanksim-6e5.5 (parent-child) - Phase 3 — habitatized ecology, substrate redox, and geometry-aware scaling

Dependents:
  <- tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters
  <- tanksim-6e5.5.7 (blocks) - Add habitat/geometry scenario tests and probes

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Future-self notes:
- This is one of the most visible "small tank vs bigger tank" realism wins after concentration normalization.
- Prefer a simple capacity model with clear knobs over a black-box emergent one that is hard to tune.
- Document which parts represent colonizable area versus process-rate multipliers.
- The key insight: a nano tank with a small sponge filter and a medium tank with a large canister filter should have very different nitrifier capacity ceilings, and this should emerge from habitat area × flow exposure, not from a tank-size lookup table.
  [2026-03-26 17:06 UTC] reviewer: Unit test requirements:

1. test_biofilter_capacity_scales_with_media_area: Two tanks with identical water chemistry but 2× filter media surface area → carrying capacity scales proportionally (within 10%).
2. test_biofilter_capacity_scales_with_flow: Same media area but 2× flow rate → higher effective capacity (flow × area interaction, not just area alone).
3. test_biofilter_o2_limitation: At low DO (< 2 mg/L), biofilter nitrification rate drops proportionally to O2 availability (Monod-style).
4. test_biofilter_zero_media_zero_capacity: Tank with no filter media → biofilter carrying capacity ≈ 0 (only glass/hardscape colonization contributes).
5. test_habitat_modifiers_applied_correctly: Verify that flow_exposure and o2_exposure modifiers from habitat registry are correctly applied to each habitat's contribution to total biofilter capacity.

Integration test:
6. test_cycling_speed_varies_with_biofilter_capacity: Run nano_cycle scenario with small vs large filter media → cycling completion time differs proportionally. Assert with envelope bounds (e.g., 2× media → cycling 30-60% faster).

All tests should log intermediate values (media area, flow factor, O2 factor, computed capacity) via A2d tracing for debugging.
