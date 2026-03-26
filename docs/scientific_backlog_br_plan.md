# Scientific backlog plan created for `br`

This document mirrors the backlog encoded in `docs/create_scientific_backlog_br.sh`.

## Intent

The backlog is organized around one guiding idea: fix scientific invariants before adding more ecosystem detail. The sequence is designed to move from safety rails, to units and kinetics, to mass conservation, to carbonate chemistry, to habitatized ecology, to shrimp realism, and finally to provenance/calibration/documentation.

## Phase dependencies

- Phase 0 is the foundation for every later phase.
- Phase 1A and Phase 1B both depend on Phase 0 and can proceed partly in parallel.
  - Tasks B1/B2 and C1 are independent starting points once A1 completes.
  - C2/C3 depend on B2 (concentration helpers), so B2 should land early to unblock conservation work.
- Phase 2 depends on Phase 1A because carbonate work needs the unit/concentration semantics to be explicit.
- Phase 3 depends on Phase 1A because habitats and geometry-aware rates need the normalized helper layer.
  - Phase 2 and Phase 3 can overlap once Phase 1A completes. The only cross-link is E4 depending on D3 (gas exchange).
- Phase 4 depends on Phases 1B, 2, and 3 because shrimp realism should sit on top of correct bookkeeping, chemistry, and habitats.
- Phase 5 mostly depends on all earlier science work; the one intentional early-enabler exception is G1 (provenance schema), which depends only on A1 and can begin before the final calibration pass.

## Backlog tree

- **ROOT** — Scientific core overhaul roadmap (v0.2–v0.6)
  Type: `epic` | Priority: `P1` | Estimate: `n/a` | Depends on: `none`
  - **A** — Phase 0 — guardrails, baselines, and migration safety
    Type: `epic` | Priority: `P1` | Estimate: `n/a` | Depends on: `none`
    - **A1** — Inventory current scientific semantics, units, invariants, and shortcuts
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `none`
    - **A2** — Add conservation/debug instrumentation and save-schema migration scaffolding
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `A1`
      - **A2a** — Implement per-tick mass budget tracking for N, C, and O2
        Type: `task` | Priority: `P1` | Estimate: `90m` | Depends on: `none` (inherits A2)
      - **A2b** — Add debug hooks and test helpers for budget inspection
        Type: `task` | Priority: `P1` | Estimate: `60m` | Depends on: `A2a`
      - **A2c** — Add save-schema versioning and migration scaffolding
        Type: `task` | Priority: `P1` | Estimate: `90m` | Depends on: `A2a`
    - **A3** — Capture baseline scenario envelopes for current v0.1 behavior
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `A1, A2`
  - **B** — Phase 1A — units, concentration semantics, and kinetic normalization
    Type: `epic` | Priority: `P0` | Estimate: `n/a` | Depends on: `A`
    - **B1** — Decide internal unit taxonomy and display policy
      Type: `spike` | Priority: `P0` | Estimate: `180m` | Depends on: `A1`
    - **B2** — Implement canonical concentration and compartment helper APIs
      Type: `task` | Priority: `P0` | Estimate: `240m` | Depends on: `B1, A2`
    - **B3** — Normalize nitrogen-cycle kinetics to concentration-based terms
      Type: `task` | Priority: `P0` | Estimate: `300m` | Depends on: `B2, A3`
      - **B3a** — Normalize AOB TAN and DO limitation to concentration-based kinetics
        Type: `task` | Priority: `P0` | Estimate: `90m` | Depends on: `none` (inherits B3)
      - **B3b** — Normalize NOB nitrite and DO limitation to concentration-based kinetics
        Type: `task` | Priority: `P0` | Estimate: `60m` | Depends on: `B3a`
      - **B3c** — Normalize comammox TAN and DO limitation to concentration-based kinetics
        Type: `task` | Priority: `P0` | Estimate: `60m` | Depends on: `B3a`
      - **B3d** — Normalize decomposer DOC and DO limitation to concentration-based kinetics
        Type: `task` | Priority: `P0` | Estimate: `60m` | Depends on: `B3a`
      - **B3e** — Add tank-size-independence regression test for all nitrogen kinetics
        Type: `task` | Priority: `P0` | Estimate: `60m` | Depends on: `B3b, B3c, B3d`
    - **B4** — Normalize plant nutrient uptake and growth limitation semantics
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `B2`
    - **B5** — Normalize algae kinetics around concentration, light, and temperature interactions
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `B2`
    - **B6** — Retune process parameters and scenario defaults after normalization
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `B3, B4, B5`
    - **B7** — Update snapshot/API field names and display conversions
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `B1, B2`
    - **B8** — Expand TDS/conductivity accounting and label any remaining estimates honestly
      Type: `task` | Priority: `P2` | Estimate: `180m` | Depends on: `B7`
  - **C** — Phase 1B — mass conservation and husbandry action semantics
    Type: `epic` | Priority: `P0` | Estimate: `n/a` | Depends on: `A`
    - **C1** — Define closed-loop matter-routing conventions for consumers and maintenance actions
      Type: `task` | Priority: `P0` | Estimate: `180m` | Depends on: `A1`
    - **C2** — Implement shrimp ingestion → assimilation → excretion → feces → respiration loop
      Type: `task` | Priority: `P0` | Estimate: `240m` | Depends on: `C1, B2`
      - **C2a** — Implement shrimp assimilation pathway with body reserve tracking
        Type: `task` | Priority: `P0` | Estimate: `60m` | Depends on: `none` (inherits C2)
      - **C2b** — Implement shrimp excretion pathway returning TAN to water
        Type: `task` | Priority: `P0` | Estimate: `60m` | Depends on: `C2a`
      - **C2c** — Implement shrimp fecal pathway returning undigested matter to detritus
        Type: `task` | Priority: `P0` | Estimate: `45m` | Depends on: `C2a`
      - **C2d** — Implement shrimp respiration O2/DIC pathway and add conservation regression test
        Type: `task` | Priority: `P0` | Estimate: `75m` | Depends on: `C2b, C2c`
    - **C3** — Implement microfauna matter routing and recycling
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `C1, B2`
    - **C4** — Audit feeding, detritus, DOC, and mineralization bookkeeping end to end
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `C2, C3, B3`
    - **C5** — Split plant trimming into export vs leave-cuttings actions
      Type: `task` | Priority: `P1` | Estimate: `120m` | Depends on: `C1, A2`
    - **C6** — Add conservation diagnostics and regression tests for grazing and maintenance loops
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `C4, C5, A2`
  - **D** — Phase 2 — carbonate chemistry, CO2 exchange, and pH realism
    Type: `epic` | Priority: `P1` | Estimate: `n/a` | Depends on: `B`
    - **D1** — Define carbonate-state contract and solver strategy
      Type: `spike` | Priority: `P1` | Estimate: `180m` | Depends on: `B1, A1`
    - **D2** — Implement explicit carbonate equilibrium and pH solver
      Type: `task` | Priority: `P0` | Estimate: `300m` | Depends on: `D1, B2, A2`
      - **D2a** — Add carbonate state variables or derived-value struct to WaterState
        Type: `task` | Priority: `P0` | Estimate: `90m` | Depends on: `none` (inherits D2)
      - **D2b** — Implement carbonate equilibrium pH solver with temperature-dependent constants
        Type: `task` | Priority: `P0` | Estimate: `120m` | Depends on: `D2a`
      - **D2c** — Replace old pH shortcut with new carbonate equilibrium solver
        Type: `task` | Priority: `P0` | Estimate: `45m` | Depends on: `D2b`
      - **D2d** — Update serialization and snapshot for carbonate state changes
        Type: `task` | Priority: `P1` | Estimate: `45m` | Depends on: `D2c`
    - **D3** — Integrate CO2 gas exchange with aeration, surface exchange, and ambient coupling
      Type: `task` | Priority: `P0` | Estimate: `240m` | Depends on: `D2`
    - **D4** — Connect photosynthesis and respiration to DIC / CO2 and day–night pH behavior
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `D2, B4, B5, C4`
    - **D5** — Upgrade source-water profiles to carry carbonate-relevant inputs and differentiated defaults
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `D2, B6`
    - **D6** — Add carbonate regression tests and scenario probes
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `D3, D4, D5, A3`
  - **E** — Phase 3 — habitatized ecology, substrate redox, and geometry-aware scaling
    Type: `epic` | Priority: `P1` | Estimate: `n/a` | Depends on: `B`
    - **E1** — Introduce habitat registry and colonizable-area model
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `B1, A1, A2`
      - **E1a** — Define habitat type enum and registry data structure
        Type: `task` | Priority: `P1` | Estimate: `90m` | Depends on: `none` (inherits E1)
      - **E1b** — Compute colonizable areas from geometry, hardware, and plant state
        Type: `task` | Priority: `P1` | Estimate: `90m` | Depends on: `E1a`
      - **E1c** — Set flow and oxygen exposure modifiers per habitat
        Type: `task` | Priority: `P1` | Estimate: `60m` | Depends on: `E1b`
    - **E2** — Scale biofilter carrying capacity with habitat, media, flow, and oxygen
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `E1, B3`
    - **E3** — Split periphyton and decomposer pools by habitat
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `E1, C4, B5`
    - **E4** — Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks
      Type: `task` | Priority: `P1` | Estimate: `300m` | Depends on: `E1, E3, D3, B4`
      - **E4a** — Add oxic/suboxic zone state to substrate model
        Type: `task` | Priority: `P1` | Estimate: `90m` | Depends on: `none` (inherits E4)
      - **E4b** — Implement simplified denitrification in suboxic substrate zones
        Type: `task` | Priority: `P1` | Estimate: `120m` | Depends on: `E4a`
      - **E4c** — Add root-zone oxygenation hooks for rooted plants
        Type: `task` | Priority: `P1` | Estimate: `90m` | Depends on: `E4a`
    - **E5** — Add depth/turbidity light attenuation and habitat-specific light exposure
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `E1, B4, B5`
    - **E6** — Scale equipment, plant mass, and stocking defaults with tank geometry
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `E1, A3, B6`
    - **E7** — Add habitat/geometry scenario tests and probes
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `E2, E4, E5, E6, A3`
  - **F** — Phase 4 — shrimp life history, toxicity, and reproduction realism
    Type: `epic` | Priority: `P2` | Estimate: `n/a` | Depends on: `C, D, E`
    - **F1** — Define shrimp state model for reserve, condition, stage, molt, and reproduction
      Type: `spike` | Priority: `P1` | Estimate: `180m` | Depends on: `C2, D5, E3`
    - **F2** — Implement stage- or size-structured shrimp population dynamics
      Type: `task` | Priority: `P1` | Estimate: `300m` | Depends on: `F1`
      - **F2a** — Add stage-structured state to shrimp population model
        Type: `task` | Priority: `P1` | Estimate: `90m` | Depends on: `none` (inherits F2)
      - **F2b** — Implement stage transitions (juvenile → sub-adult → adult → berried)
        Type: `task` | Priority: `P1` | Estimate: `90m` | Depends on: `F2a`
      - **F2c** — Integrate stage structure with feeding, mortality, and reproduction systems
        Type: `task` | Priority: `P1` | Estimate: `60m` | Depends on: `F2b`
      - **F2d** — Update snapshot, save, and TUI for stage-structured shrimp population
        Type: `task` | Priority: `P1` | Estimate: `60m` | Depends on: `F2c`
    - **F3** — Add mineral budget and molt success/failure mechanics
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `F2, D5, B8`
    - **F4** — Model reproduction success as a function of temperature, stability, condition, density, and osmotic stress
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `F2, F3`
    - **F5** — Model chloride protection against nitrite hazard and integrate toxic stress accounting
      Type: `task` | Priority: `P2` | Estimate: `180m` | Depends on: `F2, D5, B8`
    - **F6** — Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `F3, F4, F5, A3`
  - **G** — Phase 5 — provenance, calibration, validation, and release narrative
    Type: `epic` | Priority: `P2` | Estimate: `n/a` | Depends on: `B, C, D, E, F`
    - **G1** — Add parameter provenance schema and loader support
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `A1` (early start)
    - **G2** — Attach provenance and confidence metadata to high-value chemistry and ecology parameters
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `G1, B6, D5, E2, F4`
    - **G3** — Build literature-backed scenario envelopes and expected qualitative outcomes
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `A3, B6, C6, D6, E7, F6, G2`
    - **G4** — Create calibration-report workflow comparing simulated outputs to target envelopes
      Type: `task` | Priority: `P2` | Estimate: `240m` | Depends on: `G3`
    - **G5** — Update developer docs, TUI/API messaging, and scientific-scope narrative
      Type: `task` | Priority: `P2` | Estimate: `180m` | Depends on: `B7, D6, E7, F6, G4`
    - **G6** — Run end-to-end rebalance pass and capture release narrative for the scientific-core upgrade
      Type: `task` | Priority: `P2` | Estimate: `180m` | Depends on: `G4, G5`

## Cross-cutting notes

- **Do not add fish/graphics yet.** The backlog deliberately keeps scope on scientific-core credibility first.
- **Use regression envelopes, not brittle exact traces.** Many of these refactors will change numbers while improving mechanisms.
- **Prefer explicit model contracts over hidden tuning.** The backlog repeatedly asks for naming, comments, provenance, and debug output so future tuning remains explainable.
- **Calibration is a workflow, not a one-time cleanup.** The final phase is there to make future iteration cheaper.
- **Phosphorus cycling is explicitly out of scope.** The model stores phosphate_mg_p_total but does not close a P loop. This would be a separate future epic.
- **Q10 temperature coefficients are deferred.** Temperature effects are modeled via species-specific curves already; systematic Q10 review is post-v0.6.
- **Existing tests will need migration.** Many of the 19 integration tests use exact numeric assertions that will change after kinetic normalization. Phase 0 (A3) captures baselines; each phase should update affected tests.
- **Save compatibility matters.** Phase 0 (A2c) sets up migration scaffolding. Each phase that adds state fields (D2, E1, E4, F2) should register migrations.

## Estimate summary

| Phase | Tasks | Subtasks | Total estimate |
|-------|-------|----------|---------------|
| Phase 0 (A) | 3 | 3 | ~15h |
| Phase 1A (B) | 8 | 5 | ~37h |
| Phase 1B (C) | 6 | 4 | ~20h |
| Phase 2 (D) | 6 | 4 | ~22h |
| Phase 3 (E) | 7 | 6 | ~30h |
| Phase 4 (F) | 6 | 4 | ~24h |
| Phase 5 (G) | 6 | 0 | ~21h |
| **Total** | **42** | **26** | **~170h** |
