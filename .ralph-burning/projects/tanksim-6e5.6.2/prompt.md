○ tanksim-6e5.6.2 · Implement stage- or size-structured shrimp population dynamics   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: life-history, phase-4, population, scientific-core, shrimp

Context:
- The current shrimp model already distinguishes adults, juveniles, berried females, egg cohorts, and maturation progress, but it still shares most condition/readiness state globally and lacks an explicit sub-adult or reserve-aware stage layer.
- The next version should extend that coarse cohort model enough that lifecycle dynamics are visible, tunable, and chemically grounded.

Deliverable:
- Extend AnimalState into a stage-aware cohort model consistent with F1, with at least juvenile, sub-adult, adult, and berried stages plus per-stage reserve/condition semantics.
- Implement deterministic forward transitions for growth, berried state, and hatching so the lifecycle is parameterized and testable.
- Integrate stage-aware feeding, mortality, density, and reproduction logic so every stage contributes to bioload and later stress systems can hook into the same structure.
- Update snapshot/API/TUI outputs plus save migration and compatibility accessors so richer stage structure remains readable to users and loadable from old saves.
- Keep the abstraction aggregated and inspectable rather than drifting into hidden individual-level simulation.

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs
- crates/tank_core/src/types/biology.rs (AnimalState)
- crates/tank_core/src/types/snapshot.rs
- save/state code as needed

## Acceptance Criteria


Acceptance Criteria:
- the stage model distinguishes at least juvenile, sub-adult, adult, and berried cohorts with per-stage reserve/condition semantics, and old saves migrate into that layout cleanly.
- lifecycle transitions are deterministic, parameterized, and one-way forward through hatch/growth/berried progression, with counts conserved except for explicit births and deaths.
- feeding, mortality, density, and reproduction logic are stage-aware, only adults can become berried, and all stages contribute to water-chemistry load/diagnostics.
- snapshot/API/TUI expose a readable stage breakdown alongside backward-compatible total-population summaries.

Dependencies:
  -> tanksim-6e5.3.2 (blocks) - Implement shrimp ingestion → assimilation → excretion → feces → respiration loop
  -> tanksim-6e5.1.2.3 (blocks) - Add save-schema versioning and migration scaffolding
  -> tanksim-6e5.6.1 (blocks) - Define shrimp state model for reserve, condition, stage, molt, and reproduction
  -> tanksim-6e5.6 (parent-child) - Phase 4 — shrimp life history, toxicity, and reproduction realism

Dependents:
  <- tanksim-6e5.6.5 (blocks) - Model chloride protection against nitrite hazard and integrate toxic stress accounting
  <- tanksim-6e5.6.4 (blocks) - Model reproduction success as a function of temperature, stability, condition, density, and osmotic stress
  <- tanksim-6e5.6.3 (blocks) - Add mineral budget and molt success/failure mechanics

Comments:
  [2026-03-26 20:44 UTC] master: Canonicalization note: merged former F2a-F2d into F2 so the stage-model contract, transitions, system integration, and save/UI surface stay in one canonical bead. This keeps Phase 4 follow-on work blocked on one readable lifecycle milestone instead of a micro-chain.
  [2026-03-26 13:47 UTC] backlog-planner: Future-self notes:
- Use the lightest structure that unlocks the target behaviors. Three stages (juvenile, sub-adult, adult) are likely enough.
- Make sure this integrates with the mass-routing loop from C2 rather than bypassing it. Each stage should have feeding, excretion, and respiration rates.
- Pay attention to snapshot/UI burden; more state is only useful if it remains interpretable.
