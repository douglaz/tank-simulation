# Scientific backlog plan created for `br`

This document mirrors the backlog encoded in `scripts/create_scientific_backlog_br.sh`.

## Intent

The backlog is organized around one guiding idea: fix scientific invariants before adding more ecosystem detail. The sequence is designed to move from safety rails, to units and kinetics, to mass conservation, to carbonate chemistry, to habitatized ecology, to shrimp realism, and finally to provenance/calibration/documentation.

## Phase dependencies

- Phase 0 is the foundation for every later phase.
- Phase 1A and Phase 1B both depend on Phase 0 and can proceed partly in parallel.
- Phase 2 depends on Phase 1A because carbonate work needs the unit/concentration semantics to be explicit.
- Phase 3 depends on Phase 1A because habitats and geometry-aware rates need the normalized helper layer.
- Phase 4 depends on Phases 1B, 2, and 3 because shrimp realism should sit on top of correct bookkeeping, chemistry, and habitats.
- Phase 5 mostly depends on all earlier science work; the one intentional early-enabler exception is G1, which can begin sooner so provenance has a place to live before the final calibration pass.

## Backlog tree

- **ROOT** — Scientific core overhaul roadmap (v0.2–v0.6)  
  Type: `epic` | Priority: `P1` | Estimate: `n/a` | Depends on: `none`
  Summary: Purpose: - Turn the current simulator into a more scientifically reliable aquarium ecosystem model without discarding the solid engine-first architecture already in place. - Prioritize nonphysical issues called out in the review: total-mass kinetics, disappearing matter, oversimplified carbonate chemistry, ambiguous output units, fixed biofilter capacity, and incomplete geometry scaling. - Sequence work so later ecology and shrimp realism sit on top of correct invariants instead of compensating for bad math.
  - **A** — Phase 0 — guardrails, baselines, and migration safety  
    Type: `epic` | Priority: `P1` | Estimate: `n/a` | Depends on: `none`
    Summary: This phase creates the safety rails needed to refactor the scientific core without losing track of intended behavior. It captures what the current model means, adds instrumentation for conservation and unit debugging, and preserves reference scenario envelopes so future changes can be judged against qualitative expectations instead of memory.
    - **A1** — Inventory current scientific semantics, units, invariants, and shortcuts  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `none`
      Summary: Context: - The current simulator already has meaningful chemistry and ecology state, but the meaning of many fields is implicit and spread across code and data. - The review identified ambiguous units, hidden assumptions, and shortcuts whose intent should be captured before refactors begin.
    - **A2** — Add conservation/debug instrumentation and save-schema migration scaffolding  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `A1`
      Summary: Context: - The next phases will add new state fields, rename units, and re-route matter through the system. - Refactoring without instrumentation makes it too easy to silently create or destroy mass, or to break save compatibility accidentally.
    - **A3** — Capture baseline scenario envelopes for current v0.1 behavior  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `A1, A2`
      Summary: Context: - The current code already has scenario presets and regression tests for cycling, oxygen, shrimp reproduction, water changes, ambient temperature, and tank-size effects. - Those scenarios should become explicit envelopes that future refactors compare against, even if exact trajectories change.
  - **B** — Phase 1A — units, concentration semantics, and kinetic normalization  
    Type: `epic` | Priority: `P0` | Estimate: `n/a` | Depends on: `A`
    Summary: This phase repairs the most fundamental scientific issue in the current model: many rate laws are parameterized against absolute tank totals rather than concentrations. It also makes internal-vs-display chemistry semantics explicit so the engine, API, and TUI stop speaking slightly different chemical languages.
    - **B1** — Decide internal unit taxonomy and display policy  
      Type: `spike` | Priority: `P0` | Estimate: `180m` | Depends on: `A1`
      Summary: Context: - Internally, several nitrogen pools are stored as mass of nitrogen, while snapshots and UI labels currently read like full-ion concentrations. - TDS and conductivity also need a clearer story about what is estimated, what is tracked, and what remains out of scope.
    - **B2** — Implement canonical concentration and compartment helper APIs  
      Type: `task` | Priority: `P0` | Estimate: `240m` | Depends on: `B1, A2`
      Summary: Context: - Multiple systems currently compute limitations directly from tank totals. - The code needs one canonical place to ask for concentrations, areal densities, or compartment-specific views so later systems stop re-deriving them ad hoc.
    - **B3** — Normalize nitrogen-cycle kinetics to concentration-based terms  
      Type: `task` | Priority: `P0` | Estimate: `300m` | Depends on: `B2, A3`
      Summary: Context: - `nitrogen_cycle.rs` currently uses several Monod or half-saturation terms expressed against absolute totals. - This creates nonphysical tank-size behavior when two tanks share the same concentration but differ in total liters.
    - **B4** — Normalize plant nutrient uptake and growth limitation semantics  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `B2`
      Summary: Context: - Plant growth currently depends on nutrient and light logic that is directionally useful but not yet normalized through explicit concentration semantics. - Rooted plants will also need a cleaner separation between water-column and substrate access in later phases.
    - **B5** — Normalize algae kinetics around concentration, light, and temperature interactions  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `B2`
      Summary: Context: - Algae growth is one of the main visible outcomes players care about, but it currently sits on top of simplified and partly size-coupled logic. - The next version should make algae respond to light, nutrients, and temperature through clearer concentration-based rules.
    - **B6** — Retune process parameters and scenario defaults after normalization  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `B3, B4, B5`
      Summary: Context: - Once kinetics are normalized, old parameter values are no longer guaranteed to mean the same thing. - Data files and scenario presets need an explicit retuning pass so the simulator remains stable and interpretable.
    - **B7** — Update snapshot/API field names and display conversions  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `B1, B2`
      Summary: Context: - Snapshot and UI fields such as `nitrite_mg_l` and `nitrate_mg_l` currently risk misleading users about whether values are nitrogen-as-N or full-ion concentrations. - As the simulation gets more scientific, ambiguous output language becomes a product problem as well as an engineering problem.
    - **B8** — Expand TDS/conductivity accounting and label any remaining estimates honestly  
      Type: `task` | Priority: `P2` | Estimate: `180m` | Depends on: `B7`
      Summary: Context: - Current TDS/conductivity output undercounts relevant dissolved species and risks sounding more precise than it is. - The sim should either compute a meaningfully broader estimate or explicitly frame the metric as “tracked ions” or a similar approximation.
  - **C** — Phase 1B — mass conservation and husbandry action semantics  
    Type: `epic` | Priority: `P0` | Estimate: `n/a` | Depends on: `A`
    Summary: This phase closes the most important open loops in the food web and maintenance model. The goal is to stop matter from disappearing when animals graze or when the player performs husbandry actions, while keeping the simulation simple enough to remain testable and tunable.
    - **C1** — Define closed-loop matter-routing conventions for consumers and maintenance actions  
      Type: `task` | Priority: `P0` | Estimate: `180m` | Depends on: `A1`
      Summary: Context: - Shrimp, microfauna, feeding, decomposition, and plant trimming all move matter around the system, but the current code does not consistently say where consumed or removed material goes. - Before implementing fixes, the model needs one explicit routing policy for assimilation, excretion, feces, respiration, detritus, and exported biomass.
    - **C2** — Implement shrimp ingestion → assimilation → excretion → feces → respiration loop  
      Type: `task` | Priority: `P0` | Estimate: `240m` | Depends on: `C1, B2`
      Summary: Context: - Shrimp currently remove periphyton and fine detritus without returning enough of that mass to the system. - This makes grazing look cleaner and less bioload-generating than it should.
    - **C3** — Implement microfauna matter routing and recycling  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `C1, B2`
      Summary: Context: - Microfauna currently consume periphyton/detritus in a way that can also delete matter from the system. - Even if the microfauna model remains abstract, it still needs explicit routing to waste, biomass, and respiration-like demand.
    - **C4** — Audit feeding, detritus, DOC, and mineralization bookkeeping end to end  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `C2, C3, B3`
      Summary: Context: - Fixing consumer loops is not enough if the broader feed → waste → detritus → DOC/mineralization pathway still has silent sinks or double-counted steps. - This bead reconciles the full short-loop bookkeeping around feeding and decomposition.
    - **C5** — Split plant trimming into export vs leave-cuttings actions  
      Type: `task` | Priority: `P1` | Estimate: `120m` | Depends on: `C1, A2`
      Summary: Context: - Current trimming behavior turns all removed plant biomass into fine detritus, which only matches the specific case where clippings are left in the tank. - Real aquarium maintenance often exports most trimmed biomass.
    - **C6** — Add conservation diagnostics and regression tests for grazing and maintenance loops  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `C4, C5, A2`
      Summary: Context: - Once the routing changes land, the project needs repeatable proof that matter is no longer silently disappearing in normal closed-loop cases. - The same test harness should also distinguish explicit exports such as water changes and trim-and-remove.
  - **D** — Phase 2 — carbonate chemistry, CO2 exchange, and pH realism  
    Type: `epic` | Priority: `P1` | Estimate: `n/a` | Depends on: `B`
    Summary: This phase replaces the current pH shortcut with an explicit carbonate-system model and couples gas exchange to both oxygen and carbon dioxide. It aims to make source-water profiles, aeration, photosynthesis, and respiration visibly matter for pH and dissolved inorganic carbon behavior.
    - **D1** — Define carbonate-state contract and solver strategy  
      Type: `spike` | Priority: `P1` | Estimate: `180m` | Depends on: `B1, A1`
      Summary: Context: - The current pH shortcut is too compressed for the simulator’s long-term goals, but the project still needs a pragmatic rather than maximal carbonate model. - Before implementation, the team should agree on which carbonate variables are explicit state, which are derived, and what numerical strategy is acceptable.
    - **D2** — Implement explicit carbonate equilibrium and pH solver  
      Type: `task` | Priority: `P0` | Estimate: `300m` | Depends on: `D1, B2, A2`
      Summary: Context: - After D1 chooses the state contract, the engine needs a real equilibrium update instead of the current `6.3 + log10(alkalinity) - log10(DIC)` shortcut. - This is the core chemistry change that unlocks more believable pH differentiation and gas-exchange effects.
    - **D3** — Integrate CO2 gas exchange with aeration, surface exchange, and ambient coupling  
      Type: `task` | Priority: `P0` | Estimate: `240m` | Depends on: `D2`
      Summary: Context: - Aeration currently affects dissolved oxygen but not carbon dioxide stripping or pH. - A planted tank simulator needs surface exchange that moves both gases, even if the model remains lumped rather than spatially resolved.
    - **D4** — Connect photosynthesis and respiration to DIC / CO2 and day–night pH behavior  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `D2, B4, B5, C4`
      Summary: Context: - Current defaults leave important DIC-related rates at or near zero, which means day/night chemistry behavior cannot emerge meaningfully. - Planted tanks should show at least a simplified mechanistic relationship among photosynthesis, respiration, CO2 availability, and pH drift.
    - **D5** — Upgrade source-water profiles to carry carbonate-relevant inputs and differentiated defaults  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `D2, B6`
      Summary: Context: - The shipped source-water presets currently differ in GH/KH/TDS-like dimensions, but not enough in pH behavior to justify their names. - The new carbonate model needs source-water data that actually drives differentiated equilibrium outcomes.
    - **D6** — Add carbonate regression tests and scenario probes  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `D3, D4, D5, A3`
      Summary: Context: - Carbonate work is numerically delicate enough that it needs strong regression coverage as soon as it lands. - The tests should protect the intended qualitative behavior without freezing every intermediate value forever.
  - **E** — Phase 3 — habitatized ecology, substrate redox, and geometry-aware scaling  
    Type: `epic` | Priority: `P1` | Estimate: `n/a` | Depends on: `B`
    Summary: This phase makes “where things live” matter. Instead of treating periphyton, decomposers, and nitrifiers as if they inhabit a single undifferentiated tank, it introduces habitats, surface area, redox structure, and geometry-aware defaults so tank size and equipment differences act through real ecological mechanisms.
    - **E1** — Introduce habitat registry and colonizable-area model  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `B1, A1, A2`
      Summary: Context: - The current simulation tracks a tank, substrate, and filter, but not a general habitat model for surfaces and zones. - To make biofilms, periphyton, redox, and geometry matter for the right reasons, the engine needs a first-class habitat registry.
    - **E2** — Scale biofilter carrying capacity with habitat, media, flow, and oxygen  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `E1, B3`
      Summary: Context: - Nitrifier capacity is currently capped by a fixed hidden value rather than by habitat and hardware characteristics. - This prevents nano and larger tanks from differing in mature filter behavior for the right mechanistic reasons.
    - **E3** — Split periphyton and decomposer pools by habitat  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `E1, C4, B5`
      Summary: Context: - Periphyton and decomposer biomass are currently too lumped to capture differences among glass film, plant-surface biofilm, filter-associated biofilm, and substrate-associated biofilm. - A habitat split allows grazing, light exposure, and decomposition to become more ecologically legible.
    - **E4** — Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks  
      Type: `task` | Priority: `P1` | Estimate: `300m` | Depends on: `E1, E3, D3, B4`
      Summary: Context: - The current substrate model does not yet distinguish oxygenated and low-oxygen zones in a way that can support denitrification or root-zone effects. - A planted tank simulator benefits from at least a simple redox gradient story for deeper substrate and roots.
    - **E5** — Add depth/turbidity light attenuation and habitat-specific light exposure  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `E1, B4, B5`
      Summary: Context: - Tank geometry already includes depth, but the light model does not yet turn depth and turbidity into different average exposure conditions. - Depth-sensitive light is a key mechanism for distinguishing shallow and deep planted tanks under the same nominal lamp setting.
    - **E6** — Scale equipment, plant mass, and stocking defaults with tank geometry  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `E1, A3, B6`
      Summary: Context: - Geometry scaling already changes some properties, but startup hardware and biomass defaults still remain partly fixed in ways that blur intended size effects. - The simulator should distinguish purposeful size scaling from accidental mismatch.
    - **E7** — Add habitat/geometry scenario tests and probes  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `E2, E4, E5, E6, A3`
      Summary: Context: - Habitat and geometry changes will touch many systems at once, so they need scenario-level tests that protect the intended stories. - The tests should confirm that the new abstractions changed behavior for mechanistic reasons, not simply because parameters were retuned.
  - **F** — Phase 4 — shrimp life history, toxicity, and reproduction realism  
    Type: `epic` | Priority: `P2` | Estimate: `n/a` | Depends on: `C, D, E`
    Summary: This phase deepens shrimp realism once chemistry, conservation, and habitats are trustworthy enough to support it. It focuses on state structure, molting, mineral constraints, reproduction success/failure, and stress from temperature and water chemistry.
    - **F1** — Define shrimp state model for reserve, condition, stage, molt, and reproduction  
      Type: `spike` | Priority: `P1` | Estimate: `180m` | Depends on: `C2, D5, E3`
      Summary: Context: - The current shrimp model already has useful population and reproduction logic, but the next step needs a clearer internal state structure. - Before adding stages, minerals, and toxicity modifiers, the project should decide what shrimp-level state is explicit versus derived.
    - **F2** — Implement stage- or size-structured shrimp population dynamics  
      Type: `task` | Priority: `P1` | Estimate: `300m` | Depends on: `F1, C2`
      Summary: Context: - A single undifferentiated shrimp population cannot express juvenile vulnerability, adult reproduction, or growth-stage tradeoffs very well. - The next version should differentiate shrimp enough that lifecycle dynamics are visible and tunable.
    - **F3** — Add mineral budget and molt success/failure mechanics  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `F2, D5, B8`
      Summary: Context: - Shrimp husbandry is heavily shaped by minerals and molt success, but the current model does not yet express that linkage explicitly. - With richer water chemistry and clearer state structure in place, molt outcomes can now depend on something more meaningful than generic stress.
    - **F4** — Model reproduction success as a function of temperature, stability, condition, density, and osmotic stress  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `F2, F3, C2, D5`
      Summary: Context: - Current shrimp reproduction is already directionally temperature-sensitive, but the next step is to tie breeding success/failure to a richer set of explicit mechanisms. - This should enable better simulation of “tank looks okay but breeding still stalls” outcomes.
    - **F5** — Model chloride protection against nitrite hazard and integrate toxic stress accounting  
      Type: `task` | Priority: `P2` | Estimate: `180m` | Depends on: `F2, D5, B8`
      Summary: Context: - The simulator already tracks chemistry that can support better stress logic, but nitrite hazard is still missing an important modifier: chloride. - Adding this makes stress outcomes less naive and better tied to water composition.
    - **F6** — Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `F3, F4, F5, A3`
      Summary: Context: - Once the shrimp model is richer, it needs scenario coverage that demonstrates both success and failure modes. - These tests should support calibration and gameplay confidence, not just prevent panics.
  - **G** — Phase 5 — provenance, calibration, validation, and release narrative  
    Type: `epic` | Priority: `P2` | Estimate: `n/a` | Depends on: `B, C, D, E, F`
    Summary: This phase turns the upgraded scientific core into something that can be explained, calibrated, and defended. It captures parameter provenance, codifies literature-backed scenario envelopes, and leaves behind a developer-facing and player-facing narrative about what the model is and is not claiming to simulate.
    - **G1** — Add parameter provenance schema and loader support  
      Type: `task` | Priority: `P1` | Estimate: `180m` | Depends on: `A1`
      Summary: Context: - The project goal is explicitly scientific, which means important parameters should eventually carry source and confidence metadata. - The engine/data pipeline needs a minimal schema for provenance before parameter files can be enriched consistently.
    - **G2** — Attach provenance and confidence metadata to high-value chemistry and ecology parameters  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `G1, B6, D5, E2, F4`
      Summary: Context: - After the schema exists, the most consequential parameters should be annotated first rather than trying to label everything at once. - This bead focuses on the parameters most likely to matter for validation, tuning, and scientific claims.
    - **G3** — Build literature-backed scenario envelopes and expected qualitative outcomes  
      Type: `task` | Priority: `P1` | Estimate: `240m` | Depends on: `A3, B6, C6, D6, E7, F6, G2`
      Summary: Context: - By this point the simulator should have richer chemistry, habitats, and shrimp outcomes, but it still needs explicit target stories to validate against. - Those targets should be framed as scenario envelopes and causal expectations rather than fantasy exact replicas of every real tank.
    - **G4** — Create calibration-report workflow comparing simulated outputs to target envelopes  
      Type: `task` | Priority: `P2` | Estimate: `240m` | Depends on: `G3`
      Summary: Context: - Once validation scenarios exist, the project needs a repeatable way to compare current outputs to target envelopes and capture tuning changes. - Calibration should become a workflow, not a vague memory of past runs.
    - **G5** — Update developer docs, TUI/API messaging, and scientific-scope narrative  
      Type: `task` | Priority: `P2` | Estimate: `180m` | Depends on: `B7, D6, E7, F6, G4`
      Summary: Context: - A more advanced simulation will raise user expectations, so the project needs clear messaging about what the model measures, what it estimates, and where it remains deliberately approximate. - Internal docs should also explain how the new architecture is intended to be extended.
    - **G6** — Run end-to-end rebalance pass and capture release narrative for the scientific-core upgrade  
      Type: `task` | Priority: `P2` | Estimate: `180m` | Depends on: `G4, G5`
      Summary: Context: - After all major scientific-core changes land, the simulator will need a final rebalance and a clear statement of what improved. - This is the bead that turns a pile of refactors into a coherent “next version.”

## Cross-cutting notes

- **Do not add fish/graphics yet.** The backlog deliberately keeps scope on scientific-core credibility first.
- **Use regression envelopes, not brittle exact traces.** Many of these refactors will change numbers while improving mechanisms.
- **Prefer explicit model contracts over hidden tuning.** The backlog repeatedly asks for naming, comments, provenance, and debug output so future tuning remains explainable.
- **Calibration is a workflow, not a one-time cleanup.** The final phase is there to make future iteration cheaper.