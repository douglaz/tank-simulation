#!/usr/bin/env bash
set -euo pipefail

# This script creates a comprehensive beads_rust backlog for the scientific-core
# overhaul described in the aquarium simulation review and spec.
#
# It intentionally uses only `br` commands for backlog mutations:
# - br init
# - br create
# - br update
# - br comments add
# - br dep add
# - br dep cycles
# - br sync --flush-only

if ! command -v br >/dev/null 2>&1; then
  echo 'Error: `br` is not installed or not on PATH.' >&2
  echo 'Install beads_rust first, then rerun this script from the repository root.' >&2
  exit 1
fi

if [[ -f .beads/issues.jsonl ]] && [[ -s .beads/issues.jsonl ]]; then
  echo 'Error: existing .beads/issues.jsonl detected. This script assumes a clean backlog and aborts to avoid duplicate beads.' >&2
  exit 1
fi

BR=(br --actor backlog-planner)

if [[ ! -d .beads ]]; then
  "${BR[@]}" init --prefix tanksim
fi

create_bead() {
  local __outvar="$1"; shift
  local title="$1"; shift
  local type="$1"; shift
  local priority="$1"; shift
  local parent="$1"; shift
  local labels="$1"; shift
  local estimate="$1"; shift
  local description="$1"; shift
  local comment="$1"; shift
  local id
  if [[ -n "$parent" ]]; then
    id=$("${BR[@]}" create "$title" --type "$type" --priority "$priority" --parent "$parent" --silent)
  else
    id=$("${BR[@]}" create "$title" --type "$type" --priority "$priority" --silent)
  fi
  printf -v "$__outvar" '%s' "$id"
  if [[ -n "$estimate" ]]; then
    "${BR[@]}" update "$id" --estimate "$estimate" --set-labels "$labels" --description "$description"
  else
    "${BR[@]}" update "$id" --set-labels "$labels" --description "$description"
  fi
  "${BR[@]}" comments add "$id" "$comment"
  echo "Created $id :: $title"
}

add_dep() {
  local issue="$1"
  local depends_on="$2"
  "${BR[@]}" dep add "$issue" "$depends_on"
}

# ---------------------------------------------------------------------------
# Root epic, phase epics, and detailed task beads
# ---------------------------------------------------------------------------

read -r -d '' ROOT_DESC <<'EOF' || true
Purpose:
- Turn the current simulator into a more scientifically reliable aquarium ecosystem model without discarding the solid engine-first architecture already in place.
- Prioritize nonphysical issues called out in the review: total-mass kinetics, disappearing matter, oversimplified carbonate chemistry, ambiguous output units, fixed biofilter capacity, and incomplete geometry scaling.
- Sequence work so later ecology and shrimp realism sit on top of correct invariants instead of compensating for bad math.

Execution strategy:
1. add guardrails and baselines so refactors are safe
2. normalize units and concentration-based kinetics
3. close major matter loops and maintenance semantics
4. deepen carbonate / CO2 / pH behavior
5. add habitat-aware ecology and geometry-aware scaling
6. deepen shrimp life history and toxicology
7. calibrate, document, and explain the model

Success criteria:
- same concentration yields the same kinetic limitation behavior across tank sizes
- major feeding/grazing loops conserve mass within explicit tolerances
- source waters and aeration settings produce meaningfully different CO2 / pH behavior
- habitat and geometry matter for mechanistic reasons rather than hidden constants
- parameter provenance and validation scenarios explain why the sim behaves the way it does
EOF

read -r -d '' ROOT_COMMENT <<'EOF' || true
Future-self notes:
- This roadmap is intentionally biased toward scientific-core work rather than content expansion.
- Resist the temptation to add fish, diseases, or graphics until the invariants below are in place.
- Prefer explicit assumptions, helper APIs, and regression envelopes over “magic stability” tuning.
- The goal is not perfect limnology; the goal is a credible, inspectable, and teachable aquarium simulator whose failure modes emerge for the right reasons.
EOF

create_bead ROOT 'Scientific core overhaul roadmap (v0.2–v0.6)' epic 1 "" scientific-core,roadmap,planning '' "${ROOT_DESC}" "${ROOT_COMMENT}"

read -r -d '' A_DESC <<'EOF' || true
This phase creates the safety rails needed to refactor the scientific core without losing track of intended behavior.
It captures what the current model means, adds instrumentation for conservation and unit debugging, and preserves reference scenario envelopes so future changes can be judged against qualitative expectations instead of memory.
EOF

read -r -d '' A_COMMENT <<'EOF' || true
Why this phase comes first:
- large scientific refactors are hard to tune if there is no shared language for current units, invariants, and expected scenario behavior
- migration pain is cheaper to think about before new state variables and save fields are added
- later phases should be able to prove improvement rather than merely “feel better”
EOF

create_bead A 'Phase 0 — guardrails, baselines, and migration safety' epic 1 "$ROOT" scientific-core,phase-0,guardrails,testing '' "${A_DESC}" "${A_COMMENT}"

read -r -d '' B_DESC <<'EOF' || true
This phase repairs the most fundamental scientific issue in the current model: many rate laws are parameterized against absolute tank totals rather than concentrations.
It also makes internal-vs-display chemistry semantics explicit so the engine, API, and TUI stop speaking slightly different chemical languages.
EOF

read -r -d '' B_COMMENT <<'EOF' || true
Why this matters:
- if the same concentration behaves differently only because a tank has more liters, the model is not physically credible
- once kinetics are concentration-based, later geometry and habitat effects can be added for real reasons instead of hidden size artifacts
- explicit naming prevents quiet semantic drift in data files and UI outputs
EOF

create_bead B 'Phase 1A — units, concentration semantics, and kinetic normalization' epic 0 "$ROOT" scientific-core,phase-1,units,kinetics,chemistry '' "${B_DESC}" "${B_COMMENT}"

read -r -d '' C_DESC <<'EOF' || true
This phase closes the most important open loops in the food web and maintenance model.
The goal is to stop matter from disappearing when animals graze or when the player performs husbandry actions, while keeping the simulation simple enough to remain testable and tunable.
EOF

read -r -d '' C_COMMENT <<'EOF' || true
Why this matters:
- disappearing biomass makes tanks look cleaner and more stable than they should
- closed loops are required before DIC, oxygen demand, waste loading, and long-term nutrient turnover can be trusted
- maintenance actions should reflect real export-vs-in-tank-decay choices rather than silently choosing one behavior
EOF

create_bead C 'Phase 1B — mass conservation and husbandry action semantics' epic 0 "$ROOT" scientific-core,phase-1,mass-balance,ecology,actions '' "${C_DESC}" "${C_COMMENT}"

read -r -d '' D_DESC <<'EOF' || true
This phase replaces the current pH shortcut with an explicit carbonate-system model and couples gas exchange to both oxygen and carbon dioxide.
It aims to make source-water profiles, aeration, photosynthesis, and respiration visibly matter for pH and dissolved inorganic carbon behavior.
EOF

read -r -d '' D_COMMENT <<'EOF' || true
Why this matters:
- the current source-water presets differ much less in pH than their names imply
- planted tanks are strongly shaped by the interaction among CO2 availability, alkalinity, gas exchange, and photosynthesis
- later habitat and shrimp stress work will inherit better chemistry if DIC / pH behavior is mechanistic now
EOF

create_bead D 'Phase 2 — carbonate chemistry, CO2 exchange, and pH realism' epic 1 "$ROOT" scientific-core,phase-2,carbonates,chemistry,gas-exchange '' "${D_DESC}" "${D_COMMENT}"

read -r -d '' E_DESC <<'EOF' || true
This phase makes “where things live” matter.
Instead of treating periphyton, decomposers, and nitrifiers as if they inhabit a single undifferentiated tank, it introduces habitats, surface area, redox structure, and geometry-aware defaults so tank size and equipment differences act through real ecological mechanisms.
EOF

read -r -d '' E_COMMENT <<'EOF' || true
Why this matters:
- biofilter maturity should depend on media area, flow, and oxygen, not a fixed hidden cap
- light, periphyton, denitrification, and root-zone effects all depend on habitat and depth
- geometry should influence stability through surface area, depth, and carrying surfaces instead of accidental parameter mismatches
EOF

create_bead E 'Phase 3 — habitatized ecology, substrate redox, and geometry-aware scaling' epic 1 "$ROOT" scientific-core,phase-3,habitats,geometry,ecology '' "${E_DESC}" "${E_COMMENT}"

read -r -d '' F_DESC <<'EOF' || true
This phase deepens shrimp realism once chemistry, conservation, and habitats are trustworthy enough to support it.
It focuses on state structure, molting, mineral constraints, reproduction success/failure, and stress from temperature and water chemistry.
EOF

read -r -d '' F_COMMENT <<'EOF' || true
Why this is later:
- shrimp realism built on top of broken mass balance or weak chemistry would force ad hoc tuning
- by this point the model can attach shrimp outcomes to explicit mechanisms like NH3, chloride, minerals, and habitat quality
- this phase should improve both gameplay and scientific explanatory power
EOF

create_bead F 'Phase 4 — shrimp life history, toxicity, and reproduction realism' epic 2 "$ROOT" scientific-core,phase-4,shrimp,biology,toxicology '' "${F_DESC}" "${F_COMMENT}"

read -r -d '' G_DESC <<'EOF' || true
This phase turns the upgraded scientific core into something that can be explained, calibrated, and defended.
It captures parameter provenance, codifies literature-backed scenario envelopes, and leaves behind a developer-facing and player-facing narrative about what the model is and is not claiming to simulate.
EOF

read -r -d '' G_COMMENT <<'EOF' || true
Why this phase is not optional:
- “scientifically grounded” requires traceability and calibration, not only plausible-looking outputs
- future tuning is cheaper when confidence, source, and intended behavior are attached to parameters and scenarios
- release notes and docs should communicate model scope honestly, especially where the sim is still heuristic
- one exception is G1: the provenance schema can start earlier as enabling infrastructure, even though most of the calibration/reporting work lands after the science refactors
EOF

create_bead G 'Phase 5 — provenance, calibration, validation, and release narrative' epic 2 "$ROOT" scientific-core,phase-5,validation,calibration,research,docs '' "${G_DESC}" "${G_COMMENT}"

read -r -d '' A1_DESC <<'EOF' || true
Context:
- The current simulator already has meaningful chemistry and ecology state, but the meaning of many fields is implicit and spread across code and data.
- The review identified ambiguous units, hidden assumptions, and shortcuts whose intent should be captured before refactors begin.

Deliverable:
- A repo-local design note that maps every major pool and rate to its scientific meaning, unit, compartment, and display semantics.
- Explicit notes for known shortcuts, known nonphysical behavior, and assumptions that are acceptable for now.

Likely touch points:
- `crates/tank_core/src/types/*.rs`
- `crates/tank_core/src/systems/*.rs`
- `crates/tank_data/data/**/*.toml`

Acceptance:
- major pools are cataloged with owner/unit/meaning
- current invariants and missing invariants are listed
- the note is good enough that a future refactor does not need to rediscover model intent from scratch
EOF

read -r -d '' A1_COMMENT <<'EOF' || true
Rationale:
- Preserve intent, not bugs. The point is to know what the current model is trying to represent before changing it.
- This note should become the “Rosetta stone” for later field renames and test updates.
- Capture uncertainties explicitly instead of letting them remain tribal knowledge.
EOF

create_bead A1 'Inventory current scientific semantics, units, invariants, and shortcuts' task 1 "$A" scientific-core,phase-0,guardrails,analysis 180 "${A1_DESC}" "${A1_COMMENT}"

read -r -d '' A2_DESC <<'EOF' || true
Context:
- The next phases will add new state fields, rename units, and re-route matter through the system.
- Refactoring without instrumentation makes it too easy to silently create or destroy mass, or to break save compatibility accidentally.

Deliverable:
- Lightweight per-tick budget instrumentation for major tracked quantities (at minimum N, C, O2 demand, and tracked ions where feasible).
- Debug hooks or test helpers that expose budget deltas without forcing them into normal gameplay output.
- Save/version scaffolding so later state changes can be migrated intentionally instead of ad hoc.

Likely touch points:
- `crates/tank_core/src/invariants.rs`
- `crates/tank_core/src/engine.rs`
- `crates/tank_core/src/save.rs`
- `crates/tank_core/src/types/state.rs`
- `crates/tank_core/src/types/snapshot.rs`

Acceptance:
- test code can inspect budget deltas
- save/version path exists for upcoming field additions and renames
- instrumentation can be enabled in dev/test builds without cluttering the player experience
EOF

read -r -d '' A2_COMMENT <<'EOF' || true
Future-self notes:
- Not every action should conserve mass inside the tank. Water changes and “trim-and-remove” intentionally export matter, so instrumentation should distinguish internal conservation from explicit exports.
- Keep diagnostics developer-facing first; surface only the most useful summaries in the TUI later.
- Versioning early is cheaper than backfilling migrations after multiple incompatible changes land.
EOF

create_bead A2 'Add conservation/debug instrumentation and save-schema migration scaffolding' task 1 "$A" scientific-core,phase-0,guardrails,testing,migration 240 "${A2_DESC}" "${A2_COMMENT}"

read -r -d '' A3_DESC <<'EOF' || true
Context:
- The current code already has scenario presets and regression tests for cycling, oxygen, shrimp reproduction, water changes, ambient temperature, and tank-size effects.
- Those scenarios should become explicit envelopes that future refactors compare against, even if exact trajectories change.

Deliverable:
- Baseline fixtures or notes for current scenarios such as `nano_cycle`, `medium_planted`, and `warm_room`.
- Envelope-based tests or documentation describing the intended qualitative behavior, not just exact current numbers.

Likely touch points:
- `crates/tank_data/data/scenarios/*.toml`
- `crates/tank_scenarios/src/lib.rs`
- existing test modules across the workspace

Acceptance:
- each reference scenario has a short “what should happen and why” note
- later phases can compare against qualitative envelopes rather than brittle exact traces
- tank-size, oxygen, and reproduction behavior are explicitly preserved where still desired
EOF

read -r -d '' A3_COMMENT <<'EOF' || true
Rationale:
- The goal is not to freeze today’s outputs; it is to preserve the useful behavioral intent while allowing scientifically motivated changes.
- Scenario envelopes should be broad enough to survive tuning but narrow enough to catch regressions.
- These baselines will be especially useful once parameter retuning begins after concentration normalization.
EOF

create_bead A3 'Capture baseline scenario envelopes for current v0.1 behavior' task 1 "$A" scientific-core,phase-0,guardrails,testing,scenarios 240 "${A3_DESC}" "${A3_COMMENT}"

read -r -d '' B1_DESC <<'EOF' || true
Context:
- Internally, several nitrogen pools are stored as mass of nitrogen, while snapshots and UI labels currently read like full-ion concentrations.
- TDS and conductivity also need a clearer story about what is estimated, what is tracked, and what remains out of scope.

Deliverable:
- A written unit policy covering internal storage, helper method naming, and display naming.
- Clear decisions on when to use `mg N/L`, when to convert to ion-style `mg/L` for display, and how to label estimated TDS/conductivity.
- A pragmatic strategy for strong types/newtypes versus lighter naming conventions.

Likely touch points:
- `crates/tank_core/src/types/water.rs`
- `crates/tank_core/src/types/snapshot.rs`
- `crates/tank_core/src/types/process.rs`
- `crates/tank_api`
- `crates/tank_tui`

Acceptance:
- field names and helper names have a consistent convention
- display policy is written down before refactors start
- the chosen strategy is explicit enough to prevent semantic drift in future data files
EOF

read -r -d '' B1_COMMENT <<'EOF' || true
Rationale:
- This is a design decision bead, not a “bike shed” bead. Make the semantics obvious enough that future contributors do not have to guess what a field means.
- Prefer clarity over type-system maximalism. A smaller number of well-named helpers can beat a forest of wrapper types if the API is disciplined.
- Decide once, then apply consistently across engine, snapshots, data files, docs, and tests.
EOF

create_bead B1 'Decide internal unit taxonomy and display policy' spike 0 "$B" scientific-core,phase-1,units,api,tui 180 "${B1_DESC}" "${B1_COMMENT}"

read -r -d '' B2_DESC <<'EOF' || true
Context:
- Multiple systems currently compute limitations directly from tank totals.
- The code needs one canonical place to ask for concentrations, areal densities, or compartment-specific views so later systems stop re-deriving them ad hoc.

Deliverable:
- Helper methods for canonical concentration queries such as TAN, nitrite, nitrate, phosphate, DOC, DIC, and dissolved oxygen.
- Where needed, helpers for habitat/compartment area- or volume-normalized quantities.
- A small API that later systems can use instead of touching raw totals directly.

Likely touch points:
- `crates/tank_core/src/types/water.rs`
- `crates/tank_core/src/types/state.rs`
- `crates/tank_core/src/types/geometry.rs`
- `crates/tank_core/src/types/snapshot.rs`

Acceptance:
- major process systems can call helpers instead of reimplementing concentration math
- helper naming matches the unit policy from B1
- new helper coverage is enough to support nitrogen, plant, algae, and later carbonate refactors
EOF

read -r -d '' B2_COMMENT <<'EOF' || true
Future-self notes:
- Treat these helpers as the anti-regression layer that prevents total-mass kinetics from creeping back in later.
- Keep the API narrow and opinionated; it is better to have a small number of clearly named helpers than many nearly-duplicate accessors.
- Where a quantity is only an estimate, encode that in the name or docs rather than implying false precision.
EOF

create_bead B2 'Implement canonical concentration and compartment helper APIs' task 0 "$B" scientific-core,phase-1,units,helpers,chemistry 240 "${B2_DESC}" "${B2_COMMENT}"

read -r -d '' B3_DESC <<'EOF' || true
Context:
- `nitrogen_cycle.rs` currently uses several Monod or half-saturation terms expressed against absolute totals.
- This creates nonphysical tank-size behavior when two tanks share the same concentration but differ in total liters.

Deliverable:
- Refactor AOB/NOB/comammox and decomposer-related kinetics to use concentration-based or compartment-based inputs.
- Rename parameters in code and data so their units communicate the new semantics.
- Update tests to confirm same concentration gives the same limitation behavior regardless of tank size.

Likely touch points:
- `crates/tank_core/src/systems/nitrogen_cycle.rs`
- `crates/tank_core/src/types/process.rs`
- `crates/tank_data/data/process/default.toml`

Acceptance:
- no Monod-style nitrogen/oxygen limitation term depends on raw total tank mass when it should depend on concentration
- parameter names and docs reflect the new units
- a regression test explicitly checks the tank-size-independence case
EOF

read -r -d '' B3_COMMENT <<'EOF' || true
Rationale:
- This bead is the heart of the scientific refactor. Until it lands, larger tanks can look “more saturated” for the wrong reason.
- Do not simply divide by volume in a scattered way; route through the helper layer so the semantics stay centralized.
- When in doubt, choose the simpler scientifically credible formulation rather than a more elaborate but poorly constrained one.
EOF

create_bead B3 'Normalize nitrogen-cycle kinetics to concentration-based terms' task 0 "$B" scientific-core,phase-1,kinetics,nitrogen,chemistry 300 "${B3_DESC}" "${B3_COMMENT}"

read -r -d '' B4_DESC <<'EOF' || true
Context:
- Plant growth currently depends on nutrient and light logic that is directionally useful but not yet normalized through explicit concentration semantics.
- Rooted plants will also need a cleaner separation between water-column and substrate access in later phases.

Deliverable:
- Refactor plant nutrient limitation terms to use concentration-based helpers and explicit semantics for nutrient source.
- Keep the model simple enough for tuning now, while leaving hooks for later root-zone work.
- Update parameter names where they currently imply total-mass behavior.

Likely touch points:
- `crates/tank_core/src/systems/plant_growth.rs`
- `crates/tank_core/src/systems/light.rs`
- `crates/tank_core/src/types/process.rs`
- `crates/tank_data/data/plants/*.toml`

Acceptance:
- plant limitation terms are concentration-based and no longer implicitly depend on tank size
- rooted and water-column uptake assumptions are explicit in code comments or docs
- tests or scenario notes cover at least one plant-heavy case after normalization
EOF

read -r -d '' B4_COMMENT <<'EOF' || true
Future-self notes:
- Keep the first pass modest. The immediate goal is correct semantics, not a full plant physiology simulator.
- Leave room for later habitat/redox work by making nutrient-source assumptions explicit now.
- Retuning may matter more than model complexity at this stage.
EOF

create_bead B4 'Normalize plant nutrient uptake and growth limitation semantics' task 1 "$B" scientific-core,phase-1,plants,kinetics,ecology 240 "${B4_DESC}" "${B4_COMMENT}"

read -r -d '' B5_DESC <<'EOF' || true
Context:
- Algae growth is one of the main visible outcomes players care about, but it currently sits on top of simplified and partly size-coupled logic.
- The next version should make algae respond to light, nutrients, and temperature through clearer concentration-based rules.

Deliverable:
- Refactor algae limitation terms to use canonical concentration helpers.
- Preserve a simple model shape while making light/nutrient/temperature interactions explicit enough to tune.
- Prepare algae behavior to interact later with habitats, periphyton, and CO2.

Likely touch points:
- `crates/tank_core/src/systems/algae_growth.rs`
- `crates/tank_core/src/systems/light.rs`
- `crates/tank_core/src/types/process.rs`

Acceptance:
- no nutrient limitation term depends directly on raw tank totals
- algae behavior remains playable/tunable after the refactor
- code comments explain what interactions are represented versus intentionally abstracted
EOF

read -r -d '' B5_COMMENT <<'EOF' || true
Rationale:
- Algae blooms are a core emergent outcome for this project, so the growth model must at least be mechanistically sane.
- Avoid premature overfitting. A well-explained low-dimensional model is preferable to a more complex one with weak parameter grounding.
- This bead sets up later habitatized periphyton work without requiring it immediately.
EOF

create_bead B5 'Normalize algae kinetics around concentration, light, and temperature interactions' task 1 "$B" scientific-core,phase-1,algae,kinetics,ecology 240 "${B5_DESC}" "${B5_COMMENT}"

read -r -d '' B6_DESC <<'EOF' || true
Context:
- Once kinetics are normalized, old parameter values are no longer guaranteed to mean the same thing.
- Data files and scenario presets need an explicit retuning pass so the simulator remains stable and interpretable.

Deliverable:
- Review and retune relevant values in process, plant, shrimp, source-water, and scenario data files after the semantic changes from B3-B5.
- Update comments in data files where unit meanings or intended ranges changed.
- Confirm that the main shipped scenarios still tell useful stories after retuning.

Likely touch points:
- `crates/tank_data/data/process/default.toml`
- `crates/tank_data/data/plants/*.toml`
- `crates/tank_data/data/scenarios/*.toml`
- any supporting docs created in A1/A3

Acceptance:
- parameter files no longer silently carry pre-refactor assumptions
- at least the main scenarios are rerun and checked after retuning
- changed semantics are documented where future maintainers will actually see them
EOF

read -r -d '' B6_COMMENT <<'EOF' || true
Future-self notes:
- Retuning is part of the scientific refactor, not an afterthought.
- Be explicit about whether a change is “new science” versus “compensating for old unit mistakes.”
- Capture why tuned values moved, especially if they are temporary envelopes pending later habitat or carbonate work.
EOF

create_bead B6 'Retune process parameters and scenario defaults after normalization' task 1 "$B" scientific-core,phase-1,retuning,data,scenarios 240 "${B6_DESC}" "${B6_COMMENT}"

read -r -d '' B7_DESC <<'EOF' || true
Context:
- Snapshot and UI fields such as `nitrite_mg_l` and `nitrate_mg_l` currently risk misleading users about whether values are nitrogen-as-N or full-ion concentrations.
- As the simulation gets more scientific, ambiguous output language becomes a product problem as well as an engineering problem.

Deliverable:
- Rename snapshot/API fields where needed or add explicit conversion fields for display.
- Update TUI/API text so the displayed chemistry matches the internal semantics chosen in B1.
- Keep backward compatibility considerations visible if save/load or API consumers are affected.

Likely touch points:
- `crates/tank_core/src/types/snapshot.rs`
- `crates/tank_api`
- `crates/tank_tui`
- any save/schema code affected by renamed fields

Acceptance:
- a user reading the UI can tell what chemistry unit is being shown
- internal and external naming no longer contradict each other
- conversions are centralized rather than being sprinkled through rendering code
EOF

read -r -d '' B7_COMMENT <<'EOF' || true
Rationale:
- The point of scientific fidelity is undermined if the outputs are mislabeled.
- This bead should make the model easier to learn from, not just more correct internally.
- Keep old names only where compatibility truly demands it, and document any transitional shims clearly.
EOF

create_bead B7 'Update snapshot/API field names and display conversions' task 1 "$B" scientific-core,phase-1,api,tui,units 180 "${B7_DESC}" "${B7_COMMENT}"

read -r -d '' B8_DESC <<'EOF' || true
Context:
- Current TDS/conductivity output undercounts relevant dissolved species and risks sounding more precise than it is.
- The sim should either compute a meaningfully broader estimate or explicitly frame the metric as “tracked ions” or a similar approximation.

Deliverable:
- Review which dissolved species should contribute to displayed TDS/conductivity in the current model scope.
- Add missing tracked contributors where the state already exists, and rename/relabel the metric where estimation remains coarse.
- Ensure the TUI/API explain what the number means.

Likely touch points:
- `crates/tank_core/src/types/water.rs`
- `crates/tank_core/src/types/snapshot.rs`
- `crates/tank_tui`
- any docs or scenario notes that mention TDS

Acceptance:
- displayed TDS/conductivity is either materially improved or explicitly framed as an estimate
- tracked ions and omitted contributors are documented
- future fertilizers/ions can be added without redefining the metric from scratch
EOF

read -r -d '' B8_COMMENT <<'EOF' || true
Future-self notes:
- Honest approximation beats false precision.
- The player-facing goal is actionable understanding: what changed, why it changed, and whether the number should be trusted for fine-grained husbandry decisions.
- Keep the computation extensible because later fertilizer, chloride, and carbonate work will change what “tracked dissolved solids” means.
EOF

create_bead B8 'Expand TDS/conductivity accounting and label any remaining estimates honestly' task 2 "$B" scientific-core,phase-1,tds,conductivity,ui 180 "${B8_DESC}" "${B8_COMMENT}"

read -r -d '' C1_DESC <<'EOF' || true
Context:
- Shrimp, microfauna, feeding, decomposition, and plant trimming all move matter around the system, but the current code does not consistently say where consumed or removed material goes.
- Before implementing fixes, the model needs one explicit routing policy for assimilation, excretion, feces, respiration, detritus, and exported biomass.

Deliverable:
- A short design note or code comment standard describing how C/N/energy-like quantities move through consumer and maintenance actions.
- Clear rules for what stays in-tank, what becomes waste, and what counts as export.
- A documented stance on where the model remains intentionally lumped/abstract.

Likely touch points:
- `crates/tank_core/src/systems/shrimp.rs`
- `crates/tank_core/src/systems/microfauna.rs`
- `crates/tank_core/src/engine.rs`
- `crates/tank_core/src/systems/nitrogen_cycle.rs`

Acceptance:
- future implementation beads can point to one agreed routing convention
- trim, feed, graze, and decay all have explicit intended outcomes
- the design is simple enough to test and tune
EOF

read -r -d '' C1_COMMENT <<'EOF' || true
Rationale:
- This bead prevents later implementation from becoming a patchwork of one-off waste terms.
- Make the routing policy explicit enough that later shrimp and carbonate work can depend on it.
- It is okay to use lumped pools as long as the bookkeeping is intentional and documented.
EOF

create_bead C1 'Define closed-loop matter-routing conventions for consumers and maintenance actions' task 0 "$C" scientific-core,phase-1,mass-balance,design,actions 180 "${C1_DESC}" "${C1_COMMENT}"

read -r -d '' C2_DESC <<'EOF' || true
Context:
- Shrimp currently remove periphyton and fine detritus without returning enough of that mass to the system.
- This makes grazing look cleaner and less bioload-generating than it should.

Deliverable:
- Refactor shrimp feeding so consumed matter is partitioned into at least assimilation/body reserve, dissolved waste, feces/detritus, and respiration-related demand.
- Keep the first pass simple but mass-aware.
- Surface any new parameters clearly in process or species data.

Likely touch points:
- `crates/tank_core/src/systems/shrimp.rs`
- `crates/tank_core/src/types/biology.rs`
- `crates/tank_core/src/types/process.rs`
- `crates/tank_data/data/shrimp/neocaridina_davidi.toml`

Acceptance:
- shrimp grazing no longer deletes matter by default
- at least one regression test shows conservation behavior through the shrimp loop
- the new routing integrates cleanly with nitrogen/DIC/DO systems rather than bypassing them
EOF

read -r -d '' C2_COMMENT <<'EOF' || true
Future-self notes:
- Keep the first implementation transparent. A few explicit fractions with clear names are better than opaque “efficiency” magic.
- This bead is a prerequisite for later molt/reproduction realism because condition and stress should emerge from the same intake/waste loop.
- Avoid double-counting with decomposition or generic waste systems; pick one owner for each transfer.
EOF

create_bead C2 'Implement shrimp ingestion → assimilation → excretion → feces → respiration loop' task 0 "$C" scientific-core,phase-1,mass-balance,shrimp,ecology 240 "${C2_DESC}" "${C2_COMMENT}"

read -r -d '' C3_DESC <<'EOF' || true
Context:
- Microfauna currently consume periphyton/detritus in a way that can also delete matter from the system.
- Even if the microfauna model remains abstract, it still needs explicit routing to waste, biomass, and respiration-like demand.

Deliverable:
- Refactor microfauna feeding to follow the same closed-loop conventions defined in C1.
- Keep the abstraction lightweight while ensuring it does not silently erase load from the tank.
- Ensure the new loop interacts correctly with detritus and nutrient recycling.

Likely touch points:
- `crates/tank_core/src/systems/microfauna.rs`
- any shared helper code created for C2
- `crates/tank_core/src/types/process.rs`

Acceptance:
- microfauna consumption no longer acts as a hidden sink
- routing is consistent with the shrimp conventions where appropriate
- tests or budget diagnostics confirm the change
EOF

read -r -d '' C3_COMMENT <<'EOF' || true
Rationale:
- The microfauna model can stay simple, but it cannot keep breaking the nutrient loop if the simulator is supposed to teach ecosystem behavior.
- Reuse conventions and helpers from the shrimp work instead of inventing a second bookkeeping language.
- The goal is ecological credibility, not species-by-species realism.
EOF

create_bead C3 'Implement microfauna matter routing and recycling' task 1 "$C" scientific-core,phase-1,mass-balance,microfauna,ecology 180 "${C3_DESC}" "${C3_COMMENT}"

read -r -d '' C4_DESC <<'EOF' || true
Context:
- Fixing consumer loops is not enough if the broader feed → waste → detritus → DOC/mineralization pathway still has silent sinks or double-counted steps.
- This bead reconciles the full short-loop bookkeeping around feeding and decomposition.

Deliverable:
- Trace how feed inputs, uneaten food, feces, fine detritus, DOC, mineralization, and nitrification connect.
- Fix ownership boundaries so each transfer is represented once and in the right place.
- Update comments/docs around the detritus and DOC model.

Likely touch points:
- `crates/tank_core/src/systems/nitrogen_cycle.rs`
- `crates/tank_core/src/systems/shrimp.rs`
- `crates/tank_core/src/systems/microfauna.rs`
- `crates/tank_core/src/engine.rs`
- `crates/tank_core/src/types/process.rs`

Acceptance:
- a feed pulse can be followed conceptually through the major pools
- hidden sinks/double-counts are removed or documented if still intentionally abstracted
- later carbonate and oxygen work can rely on this bookkeeping
EOF

read -r -d '' C4_COMMENT <<'EOF' || true
Future-self notes:
- This is where simplifications should become explicit. It is acceptable to lump some dissolved organics or waste classes as long as the path is coherent.
- Audit with the instrumentation from A2 turned on.
- Treat the result as a mass-flow map that later developers can reason about quickly.
EOF

create_bead C4 'Audit feeding, detritus, DOC, and mineralization bookkeeping end to end' task 1 "$C" scientific-core,phase-1,mass-balance,detritus,nitrogen 240 "${C4_DESC}" "${C4_COMMENT}"

read -r -d '' C5_DESC <<'EOF' || true
Context:
- Current trimming behavior turns all removed plant biomass into fine detritus, which only matches the specific case where clippings are left in the tank.
- Real aquarium maintenance often exports most trimmed biomass.

Deliverable:
- Replace the current single trimming action with explicit semantics for exported biomass versus in-tank cuttings.
- Update snapshots/events/UI wording so the player understands which maintenance action they chose.
- Ensure exported biomass is reflected correctly in conservation/budget logic.

Likely touch points:
- `crates/tank_core/src/engine.rs`
- `crates/tank_core/src/types/actions.rs`
- `crates/tank_core/src/types/events.rs`
- `crates/tank_tui`

Acceptance:
- the action model distinguishes export from in-tank decay
- event log / UI text reflects the distinction
- budget instrumentation treats export as intentional removal rather than unexplained loss
EOF

read -r -d '' C5_COMMENT <<'EOF' || true
Rationale:
- This bead is small but high-leverage because plant maintenance is common and strongly affects nutrient export.
- The player should be able to learn from this choice rather than the simulator quietly making it for them.
- Keep backward-compat/migration implications in mind if old saves reference the previous single trim action.
EOF

create_bead C5 'Split plant trimming into export vs leave-cuttings actions' task 1 "$C" scientific-core,phase-1,actions,plants,husbandry 120 "${C5_DESC}" "${C5_COMMENT}"

read -r -d '' C6_DESC <<'EOF' || true
Context:
- Once the routing changes land, the project needs repeatable proof that matter is no longer silently disappearing in normal closed-loop cases.
- The same test harness should also distinguish explicit exports such as water changes and trim-and-remove.

Deliverable:
- Regression tests for shrimp grazing, microfauna consumption, feeding, decomposition, and trimming semantics.
- Diagnostics that report conserved-vs-exported deltas clearly enough to debug failures.
- Tolerances appropriate for deterministic floating-point simulation rather than exact arithmetic fantasy.

Likely touch points:
- `crates/tank_core/src/invariants.rs`
- test modules across `tank_core` and scenario crates
- snapshot/debug helpers from A2

Acceptance:
- closed-loop grazing/feeding cases stay within explicit tolerances
- export actions are tracked as exports, not as unexplained mass loss
- the tests are easy to extend when later shrimp or habitat work lands
EOF

read -r -d '' C6_COMMENT <<'EOF' || true
Future-self notes:
- This bead is the contract that keeps future feature work honest.
- Prefer a few crisp invariant tests plus readable debug output over a giant opaque golden-file.
- The real success condition is fast diagnosis when a future change breaks conservation.
EOF

create_bead C6 'Add conservation diagnostics and regression tests for grazing and maintenance loops' task 1 "$C" scientific-core,phase-1,testing,mass-balance,diagnostics 180 "${C6_DESC}" "${C6_COMMENT}"

read -r -d '' D1_DESC <<'EOF' || true
Context:
- The current pH shortcut is too compressed for the simulator’s long-term goals, but the project still needs a pragmatic rather than maximal carbonate model.
- Before implementation, the team should agree on which carbonate variables are explicit state, which are derived, and what numerical strategy is acceptable.

Deliverable:
- A design note describing the chosen carbonate state contract (for example CO2(aq), bicarbonate, carbonate, alkalinity, or a nearby equivalent).
- The intended solver approach, update order, and stability/precision expectations.
- Clear notes on what is intentionally excluded in the first pass.

Likely touch points:
- `crates/tank_core/src/systems/chemistry.rs`
- `crates/tank_core/src/types/water.rs`
- `crates/tank_core/src/types/source_water.rs`

Acceptance:
- the design is specific enough that implementation work can proceed without re-litigating fundamentals
- update order and ownership boundaries are clear
- simplifications are explicit rather than hidden
EOF

read -r -d '' D1_COMMENT <<'EOF' || true
Rationale:
- Carbonate chemistry can sprawl. This bead keeps the project honest about scope while still replacing the current oversimplification.
- Decide the minimum viable mechanistic model that gives better source-water differentiation and day/night pH behavior.
- Write down numeric expectations early so later tuning/debugging has a reference.
EOF

create_bead D1 'Define carbonate-state contract and solver strategy' spike 1 "$D" scientific-core,phase-2,carbonates,design,chemistry 180 "${D1_DESC}" "${D1_COMMENT}"

read -r -d '' D2_DESC <<'EOF' || true
Context:
- After D1 chooses the state contract, the engine needs a real equilibrium update instead of the current `6.3 + log10(alkalinity) - log10(DIC)` shortcut.
- This is the core chemistry change that unlocks more believable pH differentiation and gas-exchange effects.

Deliverable:
- Add the explicit carbonate state and implement the chosen pH/equilibrium solver.
- Update water state, snapshots, and any affected serialization paths.
- Keep the implementation numerically stable within the expected aquarium parameter ranges.

Likely touch points:
- `crates/tank_core/src/systems/chemistry.rs`
- `crates/tank_core/src/types/water.rs`
- `crates/tank_core/src/types/state.rs`
- `crates/tank_core/src/save.rs`

Acceptance:
- pH is computed from the new carbonate representation rather than the old shortcut
- the solver is stable across the shipped source-water profiles and common aquarium ranges
- comments/tests explain the chosen approximation level
EOF

read -r -d '' D2_COMMENT <<'EOF' || true
Future-self notes:
- The goal is a robust aquarium-focused solver, not a general geochemistry package.
- Preserve debuggability: intermediate values and assumptions should be inspectable in tests or dev snapshots.
- Do not bury important constants in magic numbers; keep them named and traceable.
EOF

create_bead D2 'Implement explicit carbonate equilibrium and pH solver' task 0 "$D" scientific-core,phase-2,carbonates,chemistry,pH 300 "${D2_DESC}" "${D2_COMMENT}"

read -r -d '' D3_DESC <<'EOF' || true
Context:
- Aeration currently affects dissolved oxygen but not carbon dioxide stripping or pH.
- A planted tank simulator needs surface exchange that moves both gases, even if the model remains lumped rather than spatially resolved.

Deliverable:
- Extend gas exchange so CO2 responds to aeration/surface exchange and interacts with the carbonate state.
- Use existing geometry and hardware inputs where possible rather than inventing a separate hidden system.
- Keep the first pass consistent with the chosen carbonate update order.

Likely touch points:
- `crates/tank_core/src/systems/chemistry.rs`
- `crates/tank_core/src/systems/dissolved_oxygen.rs`
- `crates/tank_core/src/types/hardware.rs`
- `crates/tank_core/src/types/geometry.rs`

Acceptance:
- high aeration can strip CO2 and influence pH in the expected direction
- gas exchange logic for O2 and CO2 is conceptually aligned
- the model remains deterministic and testable
EOF

read -r -d '' D3_COMMENT <<'EOF' || true
Rationale:
- This bead is what makes aeration scientifically richer than “more oxygen = good.”
- It also gives the simulation a path toward later CO2 hardware without requiring that feature immediately.
- Reuse existing geometry terms like exposed area where they already exist; do not create parallel geometry semantics.
EOF

create_bead D3 'Integrate CO2 gas exchange with aeration, surface exchange, and ambient coupling' task 0 "$D" scientific-core,phase-2,co2,gas-exchange,hardware 240 "${D3_DESC}" "${D3_COMMENT}"

read -r -d '' D4_DESC <<'EOF' || true
Context:
- Current defaults leave important DIC-related rates at or near zero, which means day/night chemistry behavior cannot emerge meaningfully.
- Planted tanks should show at least a simplified mechanistic relationship among photosynthesis, respiration, CO2 availability, and pH drift.

Deliverable:
- Wire plant/algae photosynthesis and biological respiration into the new DIC / CO2 state.
- Revisit defaults so the shipped scenarios actually exercise the new pathway.
- Make the day/night chemistry story visible in tests and/or snapshots.

Likely touch points:
- `crates/tank_core/src/systems/plant_growth.rs`
- `crates/tank_core/src/systems/algae_growth.rs`
- `crates/tank_core/src/systems/chemistry.rs`
- `crates/tank_core/src/types/process.rs`

Acceptance:
- light cycle changes can affect DIC / CO2 / pH in a believable direction
- defaults are no longer effectively disabling the pathway
- interactions remain simple enough to tune
EOF

read -r -d '' D4_COMMENT <<'EOF' || true
Future-self notes:
- This bead is about coupling, not about turning the simulator into a detailed photosynthesis model.
- Prefer a transparent day/night effect that can be explained to players and developers alike.
- Recheck scenario envelopes after this lands because it will likely change algae/plant behavior noticeably.
EOF

create_bead D4 'Connect photosynthesis and respiration to DIC / CO2 and day–night pH behavior' task 1 "$D" scientific-core,phase-2,plants,carbonates,day-night 240 "${D4_DESC}" "${D4_COMMENT}"

read -r -d '' D5_DESC <<'EOF' || true
Context:
- The shipped source-water presets currently differ in GH/KH/TDS-like dimensions, but not enough in pH behavior to justify their names.
- The new carbonate model needs source-water data that actually drives differentiated equilibrium outcomes.

Deliverable:
- Extend source-water presets so they include whatever carbonate-relevant inputs the chosen model requires.
- Revisit the shipped presets (`hard_shrimp`, `moderate`, `soft_acidic`, `ro_like`) so their behavior is meaningfully differentiated.
- Update any docs/comments explaining what each profile is trying to emulate.

Likely touch points:
- `crates/tank_core/src/types/source_water.rs`
- `crates/tank_data/data/source_water/*.toml`
- `crates/tank_data/src/presets.rs`

Acceptance:
- source-water presets no longer collapse into nearly identical pH behavior under the new model
- preset semantics are documented in plain language
- tests can load each preset successfully through the updated data path
EOF

read -r -d '' D5_COMMENT <<'EOF' || true
Rationale:
- Source water is one of the most intuitive scientific levers for players, so the presets should teach something real.
- This bead also sets up later shrimp mineral/toxicity work by making water profiles richer and clearer.
- Avoid overfitting to one real-world tap-water case; target distinct, interpretable envelopes instead.
EOF

create_bead D5 'Upgrade source-water profiles to carry carbonate-relevant inputs and differentiated defaults' task 1 "$D" scientific-core,phase-2,source-water,data,carbonates 180 "${D5_DESC}" "${D5_COMMENT}"

read -r -d '' D6_DESC <<'EOF' || true
Context:
- Carbonate work is numerically delicate enough that it needs strong regression coverage as soon as it lands.
- The tests should protect the intended qualitative behavior without freezing every intermediate value forever.

Deliverable:
- Scenario tests/probes for source-water differentiation, aeration-driven CO2 stripping, and day/night pH effects.
- Debug output or probe utilities that help inspect carbonate state during failures.
- Explicit test notes about acceptable ranges/tolerances.

Likely touch points:
- chemistry/system tests
- scenario fixtures or helpers
- debug/snapshot output used for verification

Acceptance:
- at least the core carbonate stories are codified in tests
- failures are diagnosable rather than opaque
- future tuning can change numbers inside the envelope without rewriting the scientific story
EOF

read -r -d '' D6_COMMENT <<'EOF' || true
Future-self notes:
- Carbonate chemistry bugs can look like “just a weird pH drift.” Make the tests narrative enough that failures point to a plausible cause.
- Keep tolerance choices visible and justified.
- Revisit these tests after habitat and shrimp phases if new coupled dynamics widen the realistic envelope.
EOF

create_bead D6 'Add carbonate regression tests and scenario probes' task 1 "$D" scientific-core,phase-2,testing,carbonates,scenarios 180 "${D6_DESC}" "${D6_COMMENT}"

read -r -d '' E1_DESC <<'EOF' || true
Context:
- The current simulation tracks a tank, substrate, and filter, but not a general habitat model for surfaces and zones.
- To make biofilms, periphyton, redox, and geometry matter for the right reasons, the engine needs a first-class habitat registry.

Deliverable:
- A habitat model covering at least filter media, plant surfaces, glass/hardscape, substrate surface, and deeper substrate.
- Explicit colonizable area and relevant modifiers such as flow and oxygen exposure.
- Enough abstraction that later systems can attach their biomass pools or rates to habitats without reworking the state layout again.

Likely touch points:
- `crates/tank_core/src/types/state.rs`
- `crates/tank_core/src/types/geometry.rs`
- `crates/tank_core/src/types/substrate.rs`
- supporting system modules

Acceptance:
- habitats are explicit engine concepts rather than inferred ad hoc
- each habitat has a clear purpose and minimal required metadata
- later biofilm/light/redox tasks can build on this without redesigning the core state again
EOF

read -r -d '' E1_COMMENT <<'EOF' || true
Rationale:
- “Where things live” is the key missing abstraction for the next scientific step.
- Keep habitats ecological rather than graphical. The goal is not spatial rendering; it is mechanistic differentiation.
- Start with a small fixed set of habitats and only generalize further if later work truly needs it.
EOF

create_bead E1 'Introduce habitat registry and colonizable-area model' task 1 "$E" scientific-core,phase-3,habitats,geometry,architecture 240 "${E1_DESC}" "${E1_COMMENT}"

read -r -d '' E2_DESC <<'EOF' || true
Context:
- Nitrifier capacity is currently capped by a fixed hidden value rather than by habitat and hardware characteristics.
- This prevents nano and larger tanks from differing in mature filter behavior for the right mechanistic reasons.

Deliverable:
- Replace the fixed nitrifier capacity assumption with a habitat-aware formulation tied to media area/volume, flow, and oxygen exposure.
- Keep the first pass simple and inspectable.
- Update comments/data so the new carrying-capacity semantics are obvious.

Likely touch points:
- `crates/tank_core/src/systems/nitrogen_cycle.rs`
- `crates/tank_core/src/types/hardware.rs`
- habitat state introduced in E1
- related process parameters/data

Acceptance:
- biofilter capacity meaningfully responds to habitat/media/flow inputs
- no single hidden constant stands in for every filter setup
- scenario tests can show geometry/hardware differences affecting cycling in a plausible way
EOF

read -r -d '' E2_COMMENT <<'EOF' || true
Future-self notes:
- This is one of the most visible “small tank vs bigger tank” realism wins after concentration normalization.
- Prefer a simple capacity model with clear knobs over a black-box emergent one that is hard to tune.
- Document which parts represent colonizable area versus process-rate multipliers.
EOF

create_bead E2 'Scale biofilter carrying capacity with habitat, media, flow, and oxygen' task 1 "$E" scientific-core,phase-3,biofilter,habitats,nitrogen 240 "${E2_DESC}" "${E2_COMMENT}"

read -r -d '' E3_DESC <<'EOF' || true
Context:
- Periphyton and decomposer biomass are currently too lumped to capture differences among glass film, plant-surface biofilm, filter-associated biofilm, and substrate-associated biofilm.
- A habitat split allows grazing, light exposure, and decomposition to become more ecologically legible.

Deliverable:
- Introduce habitat-specific pools or weights for periphyton and decomposer activity.
- Ensure the first pass is still computationally simple and does not explode the state space unnecessarily.
- Tie the new pools to habitats created in E1.

Likely touch points:
- `crates/tank_core/src/systems/algae_growth.rs`
- `crates/tank_core/src/systems/microfauna.rs`
- `crates/tank_core/src/systems/shrimp.rs`
- habitat-bearing state types

Acceptance:
- at least the major habitats can differ in periphyton/decomposer abundance or productivity
- shrimp and microfauna can graze in a habitat-aware way later
- comments explain what is still lumped versus newly differentiated
EOF

read -r -d '' E3_COMMENT <<'EOF' || true
Rationale:
- This bead gives “microscope life” and biofilm maturity a better substrate without requiring species-level simulation.
- Keep the dimensionality low. Habitat differentiation should reveal mechanisms, not bury the sim under bookkeeping.
- The main question is explanatory power: can a player understand why one surface is fouling or maturing differently from another?
EOF

create_bead E3 'Split periphyton and decomposer pools by habitat' task 1 "$E" scientific-core,phase-3,habitats,periphyton,decomposition 240 "${E3_DESC}" "${E3_COMMENT}"

read -r -d '' E4_DESC <<'EOF' || true
Context:
- The current substrate model does not yet distinguish oxygenated and low-oxygen zones in a way that can support denitrification or root-zone effects.
- A planted tank simulator benefits from at least a simple redox gradient story for deeper substrate and roots.

Deliverable:
- Add a two-zone or similarly pragmatic substrate redox structure.
- Introduce simple denitrification behavior in suboxic zones and hooks for root-zone oxygenation or redox effects.
- Keep the model bounded and explainable; this is not a full sediment biogeochemistry simulator.

Likely touch points:
- `crates/tank_core/src/types/substrate.rs`
- `crates/tank_core/src/types/state.rs`
- `crates/tank_core/src/systems/nitrogen_cycle.rs`
- `crates/tank_core/src/systems/plant_growth.rs`

Acceptance:
- substrate can represent at least oxic vs suboxic behavior
- nitrate removal in low-oxygen zones is possible for explicit mechanistic reasons
- rooted-plant interactions with substrate redox have clear extension hooks even if simplified at first
EOF

read -r -d '' E4_COMMENT <<'EOF' || true
Future-self notes:
- Resist the urge to model every sediment process. The value here is introducing a believable redox story, not complete sediment ecology.
- Make the transition criteria and zone meanings inspectable in debug output.
- This bead should improve explanation of “mature planted substrate behaves differently” without turning maintenance into a chemistry PhD exam.
EOF

create_bead E4 'Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks' task 1 "$E" scientific-core,phase-3,substrate,denitrification,plants,redox 300 "${E4_DESC}" "${E4_COMMENT}"

read -r -d '' E5_DESC <<'EOF' || true
Context:
- Tank geometry already includes depth, but the light model does not yet turn depth and turbidity into different average exposure conditions.
- Depth-sensitive light is a key mechanism for distinguishing shallow and deep planted tanks under the same nominal lamp setting.

Deliverable:
- Extend light handling so depth and turbidity influence light available to plants/algae/habitats.
- Integrate the result with the habitat model where useful.
- Keep the first pass simple enough that users and developers can reason about it.

Likely touch points:
- `crates/tank_core/src/systems/light.rs`
- `crates/tank_core/src/systems/plant_growth.rs`
- `crates/tank_core/src/systems/algae_growth.rs`
- `crates/tank_core/src/types/geometry.rs`

Acceptance:
- deeper or more turbid tanks receive meaningfully different effective light
- plant/algae systems can consume that difference without ad hoc hacks
- tests or scenario probes capture at least one shallow-vs-deep contrast
EOF

read -r -d '' E5_COMMENT <<'EOF' || true
Rationale:
- This bead turns geometry into a scientifically relevant control on growth instead of only a thermal or volumetric one.
- Avoid overcomplication: one clear attenuation story is better than several overlapping fudge factors.
- Make sure the result remains explainable in the TUI later.
EOF

create_bead E5 'Add depth/turbidity light attenuation and habitat-specific light exposure' task 1 "$E" scientific-core,phase-3,light,geometry,plants,algae 180 "${E5_DESC}" "${E5_COMMENT}"

read -r -d '' E6_DESC <<'EOF' || true
Context:
- Geometry scaling already changes some properties, but startup hardware and biomass defaults still remain partly fixed in ways that blur intended size effects.
- The simulator should distinguish purposeful size scaling from accidental mismatch.

Deliverable:
- Revisit scenario/preset logic so filter flow, heater power, initial plant biomass, and stocking defaults scale coherently with geometry or are explicitly overridden.
- Document which defaults scale automatically versus which remain scenario-authored choices.
- Recheck ambient-temperature behavior after the changes.

Likely touch points:
- `crates/tank_scenarios/src/lib.rs`
- `crates/tank_core/src/types/hardware.rs`
- scenario data files and related builders

Acceptance:
- geometry-scaled scenarios no longer inherit obviously mismatched default hardware/biomass
- scaling rules are written down
- tests can distinguish deliberate overrides from automatic scaling behavior
EOF

read -r -d '' E6_COMMENT <<'EOF' || true
Future-self notes:
- This bead is about making comparisons fair. When a bigger tank behaves differently, it should be because the model says it should, not because startup defaults quietly stayed nano-sized.
- Keep scenario authorship flexible: auto-scaling should help, not remove the ability to create intentionally unusual setups.
- Revisit ambient-temperature edge cases because thermal behavior depends on both geometry and hardware.
EOF

create_bead E6 'Scale equipment, plant mass, and stocking defaults with tank geometry' task 1 "$E" scientific-core,phase-3,geometry,hardware,scenarios 180 "${E6_DESC}" "${E6_COMMENT}"

read -r -d '' E7_DESC <<'EOF' || true
Context:
- Habitat and geometry changes will touch many systems at once, so they need scenario-level tests that protect the intended stories.
- The tests should confirm that the new abstractions changed behavior for mechanistic reasons, not simply because parameters were retuned.

Deliverable:
- Scenario probes for biofilter scaling, light-depth differences, habitat-specific fouling or maturation, and substrate-redox effects where practical.
- Readable debug output for habitat-specific state during failures.
- Test notes that explain what each scenario is intended to demonstrate.

Likely touch points:
- scenario/test crates
- habitat/light/nitrogen debug helpers
- any snapshot extensions needed for verification

Acceptance:
- at least one scenario demonstrates each major habitat/geometry story
- failures are diagnosable without hand-tracing the full simulation
- the tests remain resilient to reasonable retuning
EOF

read -r -d '' E7_COMMENT <<'EOF' || true
Rationale:
- Habitat work adds hidden structure; tests are what keep that structure understandable.
- Focus on explanatory scenarios rather than giant kitchen-sink simulations.
- This bead is also the proof that geometry now matters through ecology instead of accidental totals.
EOF

create_bead E7 'Add habitat/geometry scenario tests and probes' task 1 "$E" scientific-core,phase-3,testing,habitats,geometry 180 "${E7_DESC}" "${E7_COMMENT}"

read -r -d '' F1_DESC <<'EOF' || true
Context:
- The current shrimp model already has useful population and reproduction logic, but the next step needs a clearer internal state structure.
- Before adding stages, minerals, and toxicity modifiers, the project should decide what shrimp-level state is explicit versus derived.

Deliverable:
- A design note for the shrimp state model covering reserve/condition, life stage or size class, molt state, and reproduction pipeline.
- Clear boundaries between population-level state and individual-level abstraction.
- Notes on what remains intentionally omitted in the first pass.

Likely touch points:
- `crates/tank_core/src/types/biology.rs`
- `crates/tank_core/src/systems/shrimp.rs`
- shrimp data files

Acceptance:
- later shrimp implementation beads can proceed without reworking the model contract
- the state model is detailed enough for the planned mechanics but still tractable for tuning
- comments/docs explain why a given level of aggregation was chosen
EOF

read -r -d '' F1_COMMENT <<'EOF' || true
Rationale:
- This bead protects the project from sliding into either over-simplified “population only” logic or an unnecessarily expensive individual-based model.
- Keep the state choices closely tied to the behaviors the sim wants to express: molt success, breeding, juvenile survival, and chemistry-related stress.
- The best design is the one that supports explanation and calibration, not maximum detail for its own sake.
EOF

create_bead F1 'Define shrimp state model for reserve, condition, stage, molt, and reproduction' spike 1 "$F" scientific-core,phase-4,shrimp,design,biology 180 "${F1_DESC}" "${F1_COMMENT}"

read -r -d '' F2_DESC <<'EOF' || true
Context:
- A single undifferentiated shrimp population cannot express juvenile vulnerability, adult reproduction, or growth-stage tradeoffs very well.
- The next version should differentiate shrimp enough that lifecycle dynamics are visible and tunable.

Deliverable:
- Add stage or size structure to the shrimp population model consistent with F1.
- Ensure feeding, mortality, reproduction, and stress pathways can see the new structure.
- Keep save/load and snapshot implications under control.

Likely touch points:
- `crates/tank_core/src/systems/shrimp.rs`
- `crates/tank_core/src/types/biology.rs`
- `crates/tank_core/src/types/snapshot.rs`
- save/state code as needed

Acceptance:
- at least juvenile and reproductive-adult dynamics are distinguishable
- state transitions are deterministic and testable
- scenario output can show lifecycle structure in a user-comprehensible way
EOF

read -r -d '' F2_COMMENT <<'EOF' || true
Future-self notes:
- Use the lightest structure that unlocks the target behaviors. A few stages or size classes are likely enough.
- Make sure this integrates with the mass-routing loop from C2 rather than bypassing it.
- Pay attention to snapshot/UI burden; more state is only useful if it remains interpretable.
EOF

create_bead F2 'Implement stage- or size-structured shrimp population dynamics' task 1 "$F" scientific-core,phase-4,shrimp,population,life-history 300 "${F2_DESC}" "${F2_COMMENT}"

read -r -d '' F3_DESC <<'EOF' || true
Context:
- Shrimp husbandry is heavily shaped by minerals and molt success, but the current model does not yet express that linkage explicitly.
- With richer water chemistry and clearer state structure in place, molt outcomes can now depend on something more meaningful than generic stress.

Deliverable:
- Introduce a simplified mineral-budget or availability check that affects molt success/failure.
- Tie the mechanic to the water-profile story established earlier without demanding perfect ionic physiology.
- Surface the outcome in a way that players can understand and act on.

Likely touch points:
- `crates/tank_core/src/systems/shrimp.rs`
- `crates/tank_core/src/types/water.rs`
- source-water/shrimp data files
- snapshots/events/UI text

Acceptance:
- molt success is influenced by explicit chemistry-related state
- the model remains simple enough to tune and explain
- scenario tests can exercise good-vs-poor molt conditions
EOF

read -r -d '' F3_COMMENT <<'EOF' || true
Rationale:
- This bead gives GH/TDS/mineral-related husbandry a real gameplay and scientific role.
- Avoid pretending to model full crustacean physiology; the goal is a credible simplified dependency that explains common aquarium outcomes.
- Keep the player-facing feedback clear so the mechanic teaches rather than mystifies.
EOF

create_bead F3 'Add mineral budget and molt success/failure mechanics' task 1 "$F" scientific-core,phase-4,shrimp,molting,minerals,water-chemistry 240 "${F3_DESC}" "${F3_COMMENT}"

read -r -d '' F4_DESC <<'EOF' || true
Context:
- Current shrimp reproduction is already directionally temperature-sensitive, but the next step is to tie breeding success/failure to a richer set of explicit mechanisms.
- This should enable better simulation of “tank looks okay but breeding still stalls” outcomes.

Deliverable:
- Extend reproduction logic so success depends on temperature envelope, environmental stability, shrimp condition/reserve, density, and relevant water-chemistry stress proxies.
- Keep the first pass aggregated and interpretable.
- Update any reproduction-related species parameters or docs.

Likely touch points:
- `crates/tank_core/src/systems/shrimp.rs`
- `crates/tank_core/src/types/process.rs`
- `crates/tank_data/data/shrimp/neocaridina_davidi.toml`
- events/snapshot output

Acceptance:
- breeding success/failure can diverge across scenarios for explicit reasons
- high temperature or instability can suppress or sabotage reproduction in a believable way
- the mechanic still remains debuggable and tunable
EOF

read -r -d '' F4_COMMENT <<'EOF' || true
Future-self notes:
- The point is not to create hidden fertility dice rolls. Tie outcomes to visible state that the player can monitor and influence.
- Keep the literature-backed temperature signal strong, but do not let temperature become the only story.
- Density and stability should matter because mature tanks often succeed for systemic reasons, not just because they are “warm enough.”
EOF

create_bead F4 'Model reproduction success as a function of temperature, stability, condition, density, and osmotic stress' task 1 "$F" scientific-core,phase-4,shrimp,reproduction,stress 240 "${F4_DESC}" "${F4_COMMENT}"

read -r -d '' F5_DESC <<'EOF' || true
Context:
- The simulator already tracks chemistry that can support better stress logic, but nitrite hazard is still missing an important modifier: chloride.
- Adding this makes stress outcomes less naive and better tied to water composition.

Deliverable:
- Introduce a chloride-aware nitrite hazard modifier suitable for the project’s fidelity level.
- Integrate the result into shrimp stress/mortality logic without overcomplicating the full toxicology model.
- Document assumptions and parameter confidence clearly.

Likely touch points:
- `crates/tank_core/src/systems/shrimp.rs`
- `crates/tank_core/src/types/water.rs`
- source-water and process parameter files
- validation/provenance notes

Acceptance:
- equal nitrite does not imply equal hazard when chloride differs
- the toxicology pathway is transparent enough to explain in docs or debug output
- tests/scenarios cover at least one chloride-mitigation contrast
EOF

read -r -d '' F5_COMMENT <<'EOF' || true
Rationale:
- This bead is a good example of “small addition, big explanatory value.”
- Keep the formulation modest and honest about uncertainty; the win is mechanistic directionality, not toxicology exhaustiveness.
- Treat this as a bridge between chemistry richness and animal outcomes.
EOF

create_bead F5 'Model chloride protection against nitrite hazard and integrate toxic stress accounting' task 2 "$F" scientific-core,phase-4,shrimp,toxicology,nitrite,chloride 180 "${F5_DESC}" "${F5_COMMENT}"

read -r -d '' F6_DESC <<'EOF' || true
Context:
- Once the shrimp model is richer, it needs scenario coverage that demonstrates both success and failure modes.
- These tests should support calibration and gameplay confidence, not just prevent panics.

Deliverable:
- Scenario probes for successful breeding, thermal suppression, chemistry-induced stress, and at least one crash mode.
- Expected qualitative outcomes and acceptable envelopes for each scenario.
- Snapshot/debug support needed to explain failures.

Likely touch points:
- scenario/test crates
- shrimp snapshot/event output
- calibration notes

Acceptance:
- the new shrimp mechanics are exercised by explicit scenario tests
- failures point to understandable causes
- later retuning can preserve intent without freezing every exact count
EOF

read -r -d '' F6_COMMENT <<'EOF' || true
Future-self notes:
- These scenarios should be educational. A player or developer reading them should understand what husbandry story they are meant to tell.
- Prefer a handful of sharp scenarios over a single giant “everything at once” regression.
- These will become inputs to the calibration phase, so keep them readable.
EOF

create_bead F6 'Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes' task 1 "$F" scientific-core,phase-4,testing,shrimp,scenarios 180 "${F6_DESC}" "${F6_COMMENT}"

read -r -d '' G1_DESC <<'EOF' || true
Context:
- The project goal is explicitly scientific, which means important parameters should eventually carry source and confidence metadata.
- The engine/data pipeline needs a minimal schema for provenance before parameter files can be enriched consistently.

Deliverable:
- Add a provenance-friendly parameter representation or adjacent metadata path that supports source, confidence, notes, and valid-range information.
- Ensure data loading remains ergonomic enough for everyday development.
- Decide how provenance is stored for code-defined defaults versus data-file parameters.

Likely touch points:
- `crates/tank_core/src/types/process.rs`
- `crates/tank_data/src/lib.rs`
- data file layout and supporting docs

Acceptance:
- the project has a concrete place to store provenance/confidence metadata
- loaders and data structures remain maintainable
- the schema is flexible enough for both chemistry and biology parameters
EOF

read -r -d '' G1_COMMENT <<'EOF' || true
Rationale:
- Provenance is how the project grows from “plausible sim” into “scientifically grounded sim.”
- Keep the first schema simple and useful; it does not need to solve every citation-management problem on day one.
- Think about developer ergonomics so provenance becomes a habit, not a burden.
EOF

create_bead G1 'Add parameter provenance schema and loader support' task 1 "$G" scientific-core,phase-5,provenance,data,architecture 180 "${G1_DESC}" "${G1_COMMENT}"

read -r -d '' G2_DESC <<'EOF' || true
Context:
- After the schema exists, the most consequential parameters should be annotated first rather than trying to label everything at once.
- This bead focuses on the parameters most likely to matter for validation, tuning, and scientific claims.

Deliverable:
- Add provenance/confidence metadata to high-value parameters across nitrogen kinetics, carbonate chemistry, habitat/biofilter scaling, and shrimp reproduction/toxicology.
- Leave clear notes where a value is provisional or expert-curated rather than strongly literature-backed.
- Ensure the metadata is visible where future tuning work will notice it.

Likely touch points:
- process data files
- shrimp/source-water data files
- any new docs or reports generated in Phase 5

Acceptance:
- high-impact parameters have provenance entries
- confidence/uncertainty is explicit, not implied
- future maintainers can tell which parameters are strong anchors versus temporary approximations
EOF

read -r -d '' G2_COMMENT <<'EOF' || true
Future-self notes:
- Start with the load-bearing parameters, not every constant in the codebase.
- Be honest about uncertainty. A transparent provisional value is better than an unlabeled “scientific-looking” number.
- Use this bead to identify where more literature review is still needed.
EOF

create_bead G2 'Attach provenance and confidence metadata to high-value chemistry and ecology parameters' task 1 "$G" scientific-core,phase-5,provenance,chemistry,biology 240 "${G2_DESC}" "${G2_COMMENT}"

read -r -d '' G3_DESC <<'EOF' || true
Context:
- By this point the simulator should have richer chemistry, habitats, and shrimp outcomes, but it still needs explicit target stories to validate against.
- Those targets should be framed as scenario envelopes and causal expectations rather than fantasy exact replicas of every real tank.

Deliverable:
- A curated set of validation scenarios with literature-backed or otherwise justified expectations.
- Notes on what each scenario is intended to reproduce qualitatively: cycling variability, aeration effects, algae pressure, breeding windows, etc.
- Clear separation between “validated directionally” and “still heuristic.”

Likely touch points:
- validation docs
- scenario data/tests
- provenance metadata from G2

Acceptance:
- core scientific claims made by the simulator are tied to scenario envelopes
- each envelope has a short rationale and confidence statement
- future tuning can reuse the same validation language
EOF

read -r -d '' G3_COMMENT <<'EOF' || true
Rationale:
- This bead turns scattered scientific intent into an explicit validation suite.
- The success criterion is causal plausibility, not perfect prediction of every aquarium.
- Keep the validation set small enough to maintain and rich enough to matter.
EOF

create_bead G3 'Build literature-backed scenario envelopes and expected qualitative outcomes' task 1 "$G" scientific-core,phase-5,validation,scenarios,research 240 "${G3_DESC}" "${G3_COMMENT}"

read -r -d '' G4_DESC <<'EOF' || true
Context:
- Once validation scenarios exist, the project needs a repeatable way to compare current outputs to target envelopes and capture tuning changes.
- Calibration should become a workflow, not a vague memory of past runs.

Deliverable:
- A repeatable report or scriptable workflow that runs scenarios, summarizes key outputs, and compares them to target envelopes.
- Output suitable for developer review and future release notes.
- Clear notation for where the simulator is passing, marginal, or still heuristic.

Likely touch points:
- scenario/test tooling
- any reporting helpers or scripts added to the repo
- docs for interpreting calibration results

Acceptance:
- calibration can be rerun after major model changes
- reports are understandable without diving into raw traces
- the workflow is cheap enough to keep using
EOF

read -r -d '' G4_COMMENT <<'EOF' || true
Future-self notes:
- Make the report concise and decision-oriented. The point is to guide tuning, not to drown the team in raw time series.
- Leave room for both human-written interpretation and machine-generated summaries.
- This will become the backbone for future release confidence.
EOF

create_bead G4 'Create calibration-report workflow comparing simulated outputs to target envelopes' task 2 "$G" scientific-core,phase-5,calibration,tooling,reports 240 "${G4_DESC}" "${G4_COMMENT}"

read -r -d '' G5_DESC <<'EOF' || true
Context:
- A more advanced simulation will raise user expectations, so the project needs clear messaging about what the model measures, what it estimates, and where it remains deliberately approximate.
- Internal docs should also explain how the new architecture is intended to be extended.

Deliverable:
- Developer docs covering the new scientific core concepts, especially units, habitats, carbonate state, and provenance.
- Updated TUI/API wording where labels or interpretations changed.
- A concise scope/limitations narrative suitable for future README or release-note use.

Likely touch points:
- `spec.md`
- TUI/API rendering/help text
- any new architecture or calibration docs from earlier beads

Acceptance:
- a new contributor can understand the upgraded model without reverse-engineering the code
- user-facing wording does not overclaim scientific precision
- the project narrative aligns with the actual implemented fidelity
EOF

read -r -d '' G5_COMMENT <<'EOF' || true
Rationale:
- Good docs are part of scientific honesty.
- This bead also reduces future rework by making extension points and naming conventions explicit.
- Keep the tone educational: the simulator should teach players what it knows and what it is still approximating.
EOF

create_bead G5 'Update developer docs, TUI/API messaging, and scientific-scope narrative' task 2 "$G" scientific-core,phase-5,docs,tui,api,communication 180 "${G5_DESC}" "${G5_COMMENT}"

read -r -d '' G6_DESC <<'EOF' || true
Context:
- After all major scientific-core changes land, the simulator will need a final rebalance and a clear statement of what improved.
- This is the bead that turns a pile of refactors into a coherent “next version.”

Deliverable:
- One final pass over shipped scenarios and high-value parameters after all earlier beads are merged.
- A concise release narrative summarizing what changed scientifically, what new behaviors should emerge, and what still remains out of scope.
- A list of follow-on backlog candidates discovered during the pass.

Likely touch points:
- scenario data
- parameter files
- calibration outputs
- release/docs notes

Acceptance:
- the upgraded simulator has a documented post-refactor tuning state
- the release narrative ties changes back to the project’s overarching scientific goals
- obvious follow-on work is captured cleanly instead of being lost in memory
EOF

read -r -d '' G6_COMMENT <<'EOF' || true
Future-self notes:
- This is the “make it legible” bead, not just a tuning sweep.
- Use the calibration workflow and validation scenarios rather than intuition-only tweaks.
- Capture what was intentionally postponed so future roadmaps can pick it up without re-opening old debates.
EOF

create_bead G6 'Run end-to-end rebalance pass and capture release narrative for the scientific-core upgrade' task 2 "$G" scientific-core,phase-5,retuning,release,calibration 180 "${G6_DESC}" "${G6_COMMENT}"

# ---------------------------------------------------------------------------
# Dependency graph
# ---------------------------------------------------------------------------

add_dep "$B" "$A"
add_dep "$C" "$A"
add_dep "$D" "$B"
add_dep "$E" "$B"
add_dep "$F" "$C"
add_dep "$F" "$D"
add_dep "$F" "$E"
add_dep "$G" "$B"
add_dep "$G" "$C"
add_dep "$G" "$D"
add_dep "$G" "$E"
add_dep "$G" "$F"
add_dep "$A2" "$A1"
add_dep "$A3" "$A1"
add_dep "$A3" "$A2"
add_dep "$B1" "$A1"
add_dep "$B2" "$B1"
add_dep "$B2" "$A2"
add_dep "$B3" "$B2"
add_dep "$B3" "$A3"
add_dep "$B4" "$B2"
add_dep "$B5" "$B2"
add_dep "$B6" "$B3"
add_dep "$B6" "$B4"
add_dep "$B6" "$B5"
add_dep "$B7" "$B1"
add_dep "$B7" "$B2"
add_dep "$B8" "$B7"
add_dep "$C1" "$A1"
add_dep "$C2" "$C1"
add_dep "$C2" "$B2"
add_dep "$C3" "$C1"
add_dep "$C3" "$B2"
add_dep "$C4" "$C2"
add_dep "$C4" "$C3"
add_dep "$C4" "$B3"
add_dep "$C5" "$C1"
add_dep "$C5" "$A2"
add_dep "$C6" "$C4"
add_dep "$C6" "$C5"
add_dep "$C6" "$A2"
add_dep "$D1" "$B1"
add_dep "$D1" "$A1"
add_dep "$D2" "$D1"
add_dep "$D2" "$B2"
add_dep "$D2" "$A2"
add_dep "$D3" "$D2"
add_dep "$D4" "$D2"
add_dep "$D4" "$B4"
add_dep "$D4" "$B5"
add_dep "$D4" "$C4"
add_dep "$D5" "$D2"
add_dep "$D5" "$B6"
add_dep "$D6" "$D3"
add_dep "$D6" "$D4"
add_dep "$D6" "$D5"
add_dep "$D6" "$A3"
add_dep "$E1" "$B1"
add_dep "$E1" "$A1"
add_dep "$E1" "$A2"
add_dep "$E2" "$E1"
add_dep "$E2" "$B3"
add_dep "$E3" "$E1"
add_dep "$E3" "$C4"
add_dep "$E3" "$B5"
add_dep "$E4" "$E1"
add_dep "$E4" "$E3"
add_dep "$E4" "$D3"
add_dep "$E4" "$B4"
add_dep "$E5" "$E1"
add_dep "$E5" "$B4"
add_dep "$E5" "$B5"
add_dep "$E6" "$E1"
add_dep "$E6" "$A3"
add_dep "$E6" "$B6"
add_dep "$E7" "$E2"
add_dep "$E7" "$E4"
add_dep "$E7" "$E5"
add_dep "$E7" "$E6"
add_dep "$E7" "$A3"
add_dep "$F1" "$C2"
add_dep "$F1" "$D5"
add_dep "$F1" "$E3"
add_dep "$F2" "$F1"
add_dep "$F2" "$C2"
add_dep "$F3" "$F2"
add_dep "$F3" "$D5"
add_dep "$F3" "$B8"
add_dep "$F4" "$F2"
add_dep "$F4" "$F3"
add_dep "$F4" "$C2"
add_dep "$F4" "$D5"
add_dep "$F5" "$F2"
add_dep "$F5" "$D5"
add_dep "$F5" "$B8"
add_dep "$F6" "$F3"
add_dep "$F6" "$F4"
add_dep "$F6" "$F5"
add_dep "$F6" "$A3"
add_dep "$G1" "$A1"
add_dep "$G2" "$G1"
add_dep "$G2" "$B6"
add_dep "$G2" "$D5"
add_dep "$G2" "$E2"
add_dep "$G2" "$F4"
add_dep "$G3" "$A3"
add_dep "$G3" "$B6"
add_dep "$G3" "$C6"
add_dep "$G3" "$D6"
add_dep "$G3" "$E7"
add_dep "$G3" "$F6"
add_dep "$G3" "$G2"
add_dep "$G4" "$G3"
add_dep "$G5" "$B7"
add_dep "$G5" "$D6"
add_dep "$G5" "$E7"
add_dep "$G5" "$F6"
add_dep "$G5" "$G4"
add_dep "$G6" "$G4"
add_dep "$G6" "$G5"

# ---------------------------------------------------------------------------
# Sanity checks and final flush
# ---------------------------------------------------------------------------
"${BR[@]}" dep cycles
"${BR[@]}" sync --flush-only
echo 'Backlog creation complete. Review with: br epic status && br ready && br list --pretty'