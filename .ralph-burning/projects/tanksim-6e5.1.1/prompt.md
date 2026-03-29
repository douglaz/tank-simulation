○ tanksim-6e5.1.1 · Inventory current scientific semantics, units, invariants, and shortcuts   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: analysis, guardrails, phase-0, scientific-core

Context:
- The current simulator already has meaningful chemistry and ecology state, but the meaning of many fields is implicit and spread across code, data, and tests.
- The review identified ambiguous units, hidden assumptions, and shortcuts whose intent should be captured before refactors begin.
- Later beads need one repo-local reference they can cite when renaming fields, tightening invariants, or changing scenario expectations.

Deliverable:
- Create docs/scientific_inventory.md as the repo-local scientific Rosetta stone for the refactor.
- The document must include these sections:
  1. Pool catalog table: field name | location | unit | meaning | compartment | known shortcuts.
  2. Rate / parameter catalog table: parameter name | location | unit | implicit assumptions | volume- or area-dependent? | downstream beads affected.
  3. Invariant catalog: what crates/tank_core/src/invariants.rs enforces today, plus missing invariants the vNext roadmap needs.
  4. Known nonphysical behaviors / shortcuts: each item tagged with severity (cosmetic, design debt, scientifically wrong) and the bead or phase expected to address it.
  5. Test dependency map: existing tank_core / tank_scenarios tests, what they currently lock numerically, and which ones should become qualitative envelope checks.
- Call out storage-vs-display semantics explicitly where internal totals, snapshot/API names, or save fields could be misread.
- Make later-bead handoff easy: the note should name the sections most relevant to A2, B1/B2/B7, C1, D1/D2, E1/E4, and G1.

Likely touch points:
- crates/tank_core/src/types/*.rs
- crates/tank_core/src/systems/*.rs
- crates/tank_core/src/invariants.rs
- crates/tank_core/tests/*.rs
- crates/tank_scenarios/tests/*.rs
- crates/tank_data/data/**/*.toml

## Acceptance Criteria


Acceptance Criteria:
docs/scientific_inventory.md exists and is organized around the five required sections listed above.
Major pools across water, substrate, detritus, biology, hardware/snapshot state, and exported observables are cataloged with owner/unit/meaning/compartment.
The rate/parameter catalog covers the load-bearing kinetics and display-conversion parameters that later phases change, including whether each one currently smuggles volume, area, or compartment assumptions.
The invariant catalog clearly distinguishes what is enforced today versus what is missing but required for the roadmap.
Known nonphysical behaviors are listed with severity and an owning follow-on bead or phase.
The test dependency map covers the current tank_core and tank_scenarios suites, marking which tests are brittle numeric locks versus qualitative story checks.
The note is specific enough that later beads can cite sections instead of rediscovering model semantics from scratch.

Dependencies:
  -> tanksim-6e5.1 (parent-child) - Phase 0 — guardrails, baselines, and migration safety

Dependents:
  <- tanksim-6e5.4.1 (blocks) - Define carbonate-state contract and solver strategy
  <- tanksim-6e5.3.1 (blocks) - Define closed-loop matter-routing conventions for consumers and maintenance actions
  <- tanksim-6e5.2.1 (blocks) - Decide internal unit taxonomy and display policy
  <- tanksim-6e5.1.3 (blocks) - Capture baseline scenario envelopes for current v0.1 behavior
  <- tanksim-6e5.1.2.3 (blocks) - Add save-schema versioning and migration scaffolding
  <- tanksim-6e5.1.2 (blocks) - Add conservation/debug instrumentation and save-schema migration scaffolding
  <- tanksim-6e5.7.1 (blocks) - Add parameter provenance schema and loader support
  <- tanksim-6e5.5.1 (blocks) - Introduce habitat registry and colonizable-area model

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- Preserve intent, not bugs. The point is to know what the current model is trying to represent before changing it.
- This note should become the "Rosetta stone" for later field renames and test updates.
- Capture uncertainties explicitly instead of letting them remain tribal knowledge.

Specific things to document:
- Which pools are N-as-element vs N-as-ion (currently all N-as-element internally, but snapshot names suggest ion)
- Which rate parameters carry implicit volume assumptions (all Monod K_s values currently do)
- Which state variables are totals vs concentrations (currently all totals in WaterState)
- Which processes are intentionally simplified vs accidentally wrong
- Where the existing 19 integration tests depend on specific numeric values vs qualitative behavior
  [2026-03-26 17:10 UTC] reviewer: Deliverable format clarification:

The inventory should be a Markdown document at docs/scientific_inventory.md containing:

1. **Pool catalog** (table): field name | location | unit | meaning | compartment | known shortcuts
2. **Rate parameter catalog** (table): parameter name | location | unit | implicit assumptions | volume-dependent?
3. **Invariant catalog**: what invariants.rs enforces today + what invariants are MISSING but needed
4. **Known nonphysical behaviors**: list with severity (cosmetic vs scientifically wrong) and which phase addresses each
5. **Test dependency map**: which existing tests depend on specific numeric values vs qualitative behavior (critical for B3/B6)

This document becomes the "Rosetta stone" referenced in the backlog-planner comment. Later beads should cite it by section when they change semantics.
