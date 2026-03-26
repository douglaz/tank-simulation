#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# create_scientific_backlog_br.sh
# =============================================================================
# Scientific Core Overhaul Backlog — Aquarium Ecosystem Simulator
#
# Creates a comprehensive, hierarchical backlog of ~65 beads (1 root epic,
# 7 phase epics, ~33 tasks, ~26 subtasks) using `br` (beads_rust) to track
# the scientific refactor from v0.1 through v0.6.
#
# BACKGROUND AND MOTIVATION
# -------------------------
# The v0.1 simulator has a solid engine-first architecture: deterministic
# seed-based execution, REST API + TUI separation, first-class geometry,
# 19 integration tests covering cycling/O2/shrimp/thermal/water-change
# behavior. But a detailed scientific review (docs/scientific_specs.md,
# docs/aquarium_sim_review_vnext.md) identified eight major issues in the
# scientific core that undermine credibility:
#
#   1. TOTAL-MASS KINETICS — Monod/half-saturation terms in nitrogen_cycle.rs,
#      plant_growth.rs, and algae_growth.rs use tank totals (e.g.,
#      state.water.ammonia_total_mg_n_total) instead of concentrations.
#      This makes a 100L tank with 1 mg/L TAN appear 10× less nutrient-
#      limited than a 10L tank at the same concentration.
#      (nitrogen_cycle.rs:76-93,137-180,214-219; process.rs:48,50,59,61,70,72)
#
#   2. MASS CONSERVATION BREAKS — shrimp.rs:136-146 removes periphyton and
#      fine detritus from the tank without returning any mass as feces,
#      excretion, or respiration. microfauna.rs:45-55 does the same.
#      Matter vanishes, breaking the food web and underestimating bioload.
#
#   3. OVERSIMPLIFIED pH — chemistry.rs:18 uses pH = 6.3 + log10(alk) -
#      log10(DIC), which collapses all shipped source waters to ~6.3 pH
#      regardless of named chemistry differences (hard_shrimp=6.31,
#      moderate=6.30, soft_acidic=6.33).
#
#   4. OUTPUT UNIT AMBIGUITY — water.rs stores nitrogen as _mg_n_total
#      (mg of nitrogen element) but snapshot.rs:15-16 exposes fields named
#      nitrite_mg_l / nitrate_mg_l, which read like full-ion concentrations
#      (NO2-/NO3- as ion) rather than nitrogen-as-N values.
#
#   5. FIXED BIOFILTER CAPACITY — nitrogen_cycle.rs uses a logistic carrying
#      capacity for nitrifier biomass that doesn't scale with media area,
#      flow rate, or oxygen availability. A nano sponge filter and a large
#      canister share the same effective maturity ceiling.
#
#   6. INCOMPLETE GEOMETRY SCALING — tank_scenarios/src/lib.rs scales tank
#      dimensions but leaves filter flow at 200 lph, plant biomass at 5g
#      per guild, and heater power unscaled. Bigger tanks get different
#      behavior from both real and accidental causes.
#
#   7. MISSING CO2/CARBONATE SYSTEM — dissolved_oxygen.rs handles O2 surface
#      exchange and aeration but there is no CO2 counterpart. process.rs
#      sets respiration_dic_rate and photosynthesis_dic_rate to 0.0,
#      so DIC barely changes and day/night pH behavior cannot emerge.
#
#   8. NO HABITAT DIFFERENTIATION — biofilms, nitrifiers, decomposers, and
#      periphyton live in a single undifferentiated pool. There's no concept
#      of filter media surface, glass, plant-surface biofilm, or substrate
#      zones. Biofilter capacity can't depend on media because media isn't
#      modeled as a habitat.
#
# OVERARCHING GOALS
# -----------------
# - Make the simulation chemically and ecologically self-consistent before
#   adding content (fish, diseases, graphics, saltwater)
# - Ensure identical concentrations produce identical kinetic behavior
#   regardless of tank volume (scale-independence principle)
# - Close the food web so grazing, feeding, and maintenance conserve mass
#   within explicit tolerances
# - Replace the pH shortcut with carbonate equilibrium so source water,
#   aeration, and photosynthesis meaningfully shape pH behavior
# - Make "where things live" an explicit model concept so habitat area,
#   flow, and oxygen exposure drive carrying capacity and ecology
# - Deepen shrimp realism only after the underlying chemistry and ecology
#   are trustworthy enough to support it
# - Build parameter provenance and calibration infrastructure so the
#   simulator can defend its scientific claims
# - Preserve the solid existing architecture: engine-first, deterministic,
#   seed-based, with API + TUI separation
#
# PHASE STRUCTURE AND DEPENDENCIES
# ---------------------------------
#   Phase 0 (A)  — Guardrails: semantics inventory, instrumentation, baselines
#   Phase 1A (B) — Units & Kinetics: concentration normalization, helper APIs
#   Phase 1B (C) — Mass Conservation: consumer loops, maintenance actions
#   Phase 2 (D)  — Carbonate Chemistry: CO2/HCO3-/CO3--, pH solver, gas exchange
#   Phase 3 (E)  — Habitats: registry, biofilter scaling, substrate redox, light
#   Phase 4 (F)  — Shrimp Realism: stages, molting, reproduction, toxicology
#   Phase 5 (G)  — Provenance: parameter metadata, validation, calibration
#
# Dependency ordering:
#
#   A ──→ B ──→ D ──→ F ──→ G
#   A ──→ C ──→ F     ↑     ↑
#         B ──→ E ────┘     │
#         All phases ───────┘
#
#   Phases 1A (B) and 1B (C) can proceed partly in parallel after Phase 0.
#   Tasks B1/B2 and C1 are independent starting points once A1 completes.
#   Phase 2 (D) and Phase 3 (E) can also overlap once Phase 1A completes.
#   Phase 5 task G1 (provenance schema) can start early, gated only on A1.
#
# SUBTASK PHILOSOPHY
# ------------------
# Subtasks are added to tasks that bundle multiple distinct deliverables,
# touch different subsystems, or exceed 240 minutes of estimated work.
# Each subtask should be independently completable and verifiable.
#
# External inter-task dependencies point to parent tasks. Subtask ordering
# is internal to the parent task. This keeps the cross-task dependency
# graph at the same granularity as the original while gaining trackability
# within large work items.
#
# Decomposed tasks:
#   A2 (instrumentation + migration)                 → 3 subtasks
#   B3 (nitrogen normalization across 4 subsystems)   → 5 subtasks
#   C2 (4 distinct waste pathways)                    → 4 subtasks
#   D2 (state + solver + old-code removal + save)     → 4 subtasks
#   E1 (type system + area computation + modifiers)   → 3 subtasks
#   E4 (zone state + denitrification + root hooks)    → 3 subtasks
#   F2 (state + transitions + integration + snapshot)  → 4 subtasks
#
# EXPLICITLY OUT OF SCOPE FOR THIS ROADMAP
# -----------------------------------------
# - Phosphorus cycling: the model stores phosphate_mg_p_total but does not
#   close a P loop. Full P conservation would be a separate future epic.
# - Systematic Q10 temperature coefficients: temperature effects are already
#   modeled via species-specific curves (shrimp reproduction, algae growth).
#   A formal Q10 review across all rate processes is deferred to post-v0.6.
# - Fish, saltwater, disease, multiplayer, graphics: per spec.md scope.
# - Full sediment biogeochemistry: substrate redox is simplified to 2 zones.
# - Individual-based shrimp model: population uses stage/class aggregation.
# - API versioning: the API is internal (TUI ↔ API on localhost) and not
#   yet public. Field renames are handled in B7; save migration in A2.
#
# EXISTING TEST SUITE IMPACT
# --------------------------
# The project has 19 integration tests in crates/tank_core/tests/:
#   cycling.rs, shrimp_population.rs, dissolved_oxygen.rs,
#   tank_size_thermal_response.rs, ambient_temperature_action.rs,
#   chemistry.rs, water_change_mass_balance.rs, water_change_do_mixing.rs,
#   plant_algae_integration.rs, rooted_substrate_advantage.rs,
#   substrate_filter_integration.rs, filter_cleanliness_decay.rs,
#   overfeeding_effects.rs, thermal_reproduction_penalty.rs,
#   determinism.rs, save_load.rs, events.rs, validation.rs, end_to_end.rs
#
# Many of these tests will need tolerance adjustments or envelope-style
# rewrites as kinetics change. Phase 0 task A3 captures the baselines
# that make this migration explicit rather than ad hoc.
#
# HOW TO USE THIS SCRIPT
# ----------------------
# 1. Install beads_rust: cargo install beads_rust
# 2. Navigate to the repository root: cd /path/to/tank-simulation
# 3. Ensure no existing backlog: rm -rf .beads/ (if re-running)
# 4. Run: bash docs/create_scientific_backlog_br.sh
# 5. Review: br epic status && br ready && br list --pretty
#
# CAUTION: `read -r -d '' VAR <<'EOF' || true` is used throughout because
# `read` returns non-zero when it hits EOF without finding the NUL delimiter.
# The `|| true` prevents set -e from aborting the script. This is a standard
# bash idiom for multi-line string assignment via heredoc.
# =============================================================================

if ! command -v br >/dev/null 2>&1; then
  echo 'Error: `br` is not installed or not on PATH.' >&2
  echo 'Install beads_rust first, then rerun this script from the repository root.' >&2
  exit 1
fi

if [[ -f .beads/issues.jsonl ]] && [[ -s .beads/issues.jsonl ]]; then
  echo 'Error: existing .beads/issues.jsonl detected.' >&2
  echo 'This script assumes a clean backlog and aborts to avoid duplicate beads.' >&2
  echo 'To re-run, first: rm -rf .beads/' >&2
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

# ===========================================================================
# ROOT EPIC
# ===========================================================================

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
- Prefer explicit assumptions, helper APIs, and regression envelopes over "magic stability" tuning.
- The goal is not perfect limnology; the goal is a credible, inspectable, and teachable aquarium simulator whose failure modes emerge for the right reasons.
- The project already has a strong foundation: 5-crate workspace, deterministic engine, 19 integration tests, REST API, TUI. This roadmap repairs the scientific core without rearchitecting what works.

Parallelism notes for planning:
- After Phase 0 (A) completes, Phase 1A (B) and Phase 1B (C) can proceed in parallel because B targets kinetic semantics while C targets mass routing. The key cross-phase link is that C2 and C3 depend on B2 (concentration helpers), but B2 sits early in Phase 1A and can unblock C work quickly.
- After Phase 1A (B) completes, Phase 2 (D) and Phase 3 (E) can overlap. D addresses chemistry depth while E addresses spatial/habitat structure. The only cross-link is E4 depending on D3 (gas exchange patterns for redox).
- Phase 5 task G1 (provenance schema) depends only on A1 and can start as soon as the semantics inventory exists, even while other phases are still in progress.
EOF

create_bead ROOT 'Scientific core overhaul roadmap (v0.2–v0.6)' epic 1 "" scientific-core,roadmap,planning '' "${ROOT_DESC}" "${ROOT_COMMENT}"

# ===========================================================================
# PHASE EPICS
# ===========================================================================

read -r -d '' A_DESC <<'EOF' || true
This phase creates the safety rails needed to refactor the scientific core without losing track of intended behavior.
It captures what the current model means, adds instrumentation for conservation and unit debugging, and preserves reference scenario envelopes so future changes can be judged against qualitative expectations instead of memory.
EOF

read -r -d '' A_COMMENT <<'EOF' || true
Why this phase comes first:
- Large scientific refactors are hard to tune if there is no shared language for current units, invariants, and expected scenario behavior.
- Migration pain is cheaper to think about before new state variables and save fields are added.
- Later phases should be able to prove improvement rather than merely "feel better."

Scope of existing infrastructure to build on:
- 19 integration tests already exist covering cycling, DO, shrimp, thermal, water changes, substrate, plants/algae, determinism, save/load, and end-to-end scenarios.
- 3 shipped scenarios (nano_cycle, medium_planted, warm_room) with associated data files.
- Invariant enforcement already exists in invariants.rs and engine.rs.
- Save/load with version field already exists in save.rs.
EOF

create_bead A 'Phase 0 — guardrails, baselines, and migration safety' epic 1 "$ROOT" scientific-core,phase-0,guardrails,testing '' "${A_DESC}" "${A_COMMENT}"

read -r -d '' B_DESC <<'EOF' || true
This phase repairs the most fundamental scientific issue in the current model: many rate laws are parameterized against absolute tank totals rather than concentrations.
It also makes internal-vs-display chemistry semantics explicit so the engine, API, and TUI stop speaking slightly different chemical languages.
EOF

read -r -d '' B_COMMENT <<'EOF' || true
Why this matters:
- If the same concentration behaves differently only because a tank has more liters, the model is not physically credible. This is currently the case: nitrogen_cycle.rs Monod terms use state.water.ammonia_total_mg_n_total (total mg in tank) where they should use concentration (mg/L). With aob_k_tan_mg = 0.5 in a 10L tank, effective K_s ≈ 0.05 mg/L; in a 100L tank, effective K_s ≈ 0.005 mg/L. That's a 10× difference for identical chemistry.
- Once kinetics are concentration-based, later geometry and habitat effects can be added for real reasons instead of hidden size artifacts.
- Explicit naming prevents quiet semantic drift in data files and UI outputs.

Parallelism with Phase 1B (C):
- B1 and C1 are independent once A1 completes — one decides unit conventions, the other decides matter-routing conventions.
- B2 (helpers) should land early because C2/C3 depend on it for concentration queries.
- B3-B5 (kinetic normalization) and C2-C5 (conservation fixes) can then proceed in parallel on separate PRs.
EOF

create_bead B 'Phase 1A — units, concentration semantics, and kinetic normalization' epic 0 "$ROOT" scientific-core,phase-1,units,kinetics,chemistry '' "${B_DESC}" "${B_COMMENT}"

read -r -d '' C_DESC <<'EOF' || true
This phase closes the most important open loops in the food web and maintenance model.
The goal is to stop matter from disappearing when animals graze or when the player performs husbandry actions, while keeping the simulation simple enough to remain testable and tunable.
EOF

read -r -d '' C_COMMENT <<'EOF' || true
Why this matters:
- Disappearing biomass makes tanks look cleaner and more stable than they should.
  In shrimp.rs:136-146, grazing removes periphyton and fine detritus but returns nothing — no feces, no TAN excretion, no O2 demand. microfauna.rs:45-55 does the same.
- Closed loops are required before DIC, oxygen demand, waste loading, and long-term nutrient turnover can be trusted.
- Maintenance actions should reflect real export-vs-in-tank-decay choices rather than silently choosing one behavior. Currently engine.rs:189-197 routes ALL trimmed plant biomass to fine_detritus_g_total, which only matches "leave clippings in tank."

Parallelism with Phase 1A (B):
- C1 (matter routing conventions) is independent of B1 (unit conventions) — they address different design questions.
- C2 and C3 both depend on B2 (concentration helpers) but NOT on B3-B5 (kinetic normalization). This means conservation work can proceed as soon as helpers land.
- C4 (bookkeeping audit) depends on B3 because it needs nitrogen kinetics to be correct before auditing the full pathway.
EOF

create_bead C 'Phase 1B — mass conservation and husbandry action semantics' epic 0 "$ROOT" scientific-core,phase-1,mass-balance,ecology,actions '' "${C_DESC}" "${C_COMMENT}"

read -r -d '' D_DESC <<'EOF' || true
This phase replaces the current pH shortcut with an explicit carbonate-system model and couples gas exchange to both oxygen and carbon dioxide.
It aims to make source-water profiles, aeration, photosynthesis, and respiration visibly matter for pH and dissolved inorganic carbon behavior.
EOF

read -r -d '' D_COMMENT <<'EOF' || true
Why this matters:
- The current pH formula (chemistry.rs:18: pH = 6.3 + log10(alk) - log10(DIC)) collapses all shipped source-water presets to nearly the same pH. hard_shrimp ≈ 6.31, moderate ≈ 6.30, soft_acidic ≈ 6.33, ro_like ≈ 6.70. Named waters that differ in hardness and ions should differ meaningfully in pH behavior.
- Planted tanks are strongly shaped by the interaction among CO2 availability, alkalinity, gas exchange, and photosynthesis. The current model sets respiration_dic_rate and photosynthesis_dic_rate to 0.0 (process.rs:135-136), so day/night chemistry behavior cannot emerge.
- dissolved_oxygen.rs already handles O2 surface exchange and aeration (lines 27-33) using K_LA and saturation-deficit logic. CO2 exchange should follow a parallel structure.
- Later habitat and shrimp stress work will inherit better chemistry if DIC/pH behavior is mechanistic now.

Scientific foundation:
- Carbonate equilibrium: CO2(aq) ↔ H2CO3 ↔ HCO3- + H+ ↔ CO3-- + 2H+
- Key constants: pKa1 ≈ 6.35, pKa2 ≈ 10.33 at 25°C (temperature-dependent)
- Henry's law for CO2 dissolution: [CO2(aq)] = K_H × pCO2
- Alkalinity ≈ [HCO3-] + 2[CO3--] + [OH-] - [H+] (simplified for freshwater)
- The simulator doesn't need a general geochemistry package — it needs a robust freshwater carbonate solver for pH 5.5–8.5 and alkalinity 0–10 meq/L.
EOF

create_bead D 'Phase 2 — carbonate chemistry, CO2 exchange, and pH realism' epic 1 "$ROOT" scientific-core,phase-2,carbonates,chemistry,gas-exchange '' "${D_DESC}" "${D_COMMENT}"

read -r -d '' E_DESC <<'EOF' || true
This phase makes "where things live" matter.
Instead of treating periphyton, decomposers, and nitrifiers as if they inhabit a single undifferentiated tank, it introduces habitats, surface area, redox structure, and geometry-aware defaults so tank size and equipment differences act through real ecological mechanisms.
EOF

read -r -d '' E_COMMENT <<'EOF' || true
Why this matters:
- Biofilter maturity should depend on media area, flow, and oxygen, not a hidden cap. nitrogen_cycle.rs uses a logistic carrying capacity that doesn't scale with hardware.
- Light, periphyton, denitrification, and root-zone effects all depend on habitat and depth, but the current light system (light.rs) doesn't attenuate with depth despite geometry.rs carrying fill_height_cm.
- Geometry should influence stability through surface area, depth, and carrying surfaces instead of accidental parameter mismatches.
- The current substrate model (substrate.rs) tracks nutrient charge and kind but has no concept of oxic vs suboxic zones, which means denitrification cannot be represented mechanistically.

Target habitats (minimal set):
- Filter media (colonizable area from flow rate and media type)
- Glass / hardscape (tank wall area from geometry)
- Plant surfaces (dynamic, tied to plant biomass)
- Substrate surface (top layer, oxygenated)
- Substrate deep (suboxic, potential denitrification zone)

Parallelism with Phase 2 (D):
- E1-E3 can proceed as soon as Phase 1A completes, independent of carbonate work.
- E4 (substrate redox + denitrification) depends on D3 (gas exchange) because it establishes patterns for gas-phase interactions that the redox model should follow.
- E5-E7 can overlap with D4-D6.
EOF

create_bead E 'Phase 3 — habitatized ecology, substrate redox, and geometry-aware scaling' epic 1 "$ROOT" scientific-core,phase-3,habitats,geometry,ecology '' "${E_DESC}" "${E_COMMENT}"

read -r -d '' F_DESC <<'EOF' || true
This phase deepens shrimp realism once chemistry, conservation, and habitats are trustworthy enough to support it.
It focuses on state structure, molting, mineral constraints, reproduction success/failure, and stress from temperature and water chemistry.
EOF

read -r -d '' F_COMMENT <<'EOF' || true
Why this is later:
- Shrimp realism built on top of broken mass balance or weak chemistry would force ad hoc tuning. By this point the model has concentration-based kinetics, closed feeding loops, real carbonate chemistry, and habitat-aware ecology.
- Shrimp outcomes can now be attached to explicit mechanisms: NH3 speciation from pH (which is now mechanistic), chloride from source water profiles (which are now differentiated), minerals from GH (which is already tracked in water.rs), and habitat quality from the habitat registry.
- The current shrimp model (shrimp.rs) already has useful population, reproduction, and temperature-stress logic. This phase extends it with structure, not replaces it.

Scientific notes on Neocaridina davidi:
- Optimal breeding temperature: 22-28°C. Spawning observed at 28°C but not 33°C (PubMed 29949439).
- Molt frequency increases with temperature; molt success depends on Ca2+/Mg2+ availability for exoskeleton formation.
- Nitrite toxicity is modulated by chloride:nitrite ratio — chloride competes with nitrite for gill uptake in freshwater crustaceans.
- Juvenile survival is strongly stage-dependent; newly hatched shrimp are more sensitive to water chemistry than adults.
EOF

create_bead F 'Phase 4 — shrimp life history, toxicity, and reproduction realism' epic 2 "$ROOT" scientific-core,phase-4,shrimp,biology,toxicology '' "${F_DESC}" "${F_COMMENT}"

read -r -d '' G_DESC <<'EOF' || true
This phase turns the upgraded scientific core into something that can be explained, calibrated, and defended.
It captures parameter provenance, codifies literature-backed scenario envelopes, and leaves behind a developer-facing and player-facing narrative about what the model is and is not claiming to simulate.
EOF

read -r -d '' G_COMMENT <<'EOF' || true
Why this phase is not optional:
- "Scientifically grounded" requires traceability and calibration, not only plausible-looking outputs.
- Future tuning is cheaper when confidence, source, and intended behavior are attached to parameters and scenarios.
- Release notes and docs should communicate model scope honestly, especially where the sim is still heuristic.
- One exception is G1: the provenance schema can start earlier as enabling infrastructure, even though most calibration/reporting work lands after the science refactors.

What "scientifically grounded" means for this project:
- Not: perfect predictive accuracy for any specific real aquarium.
- Yes: every major mechanism is directionally correct, scale-independent, mass-conserving, and traceable to a literature source or explicit expert judgment.
- Yes: failure modes (algae blooms, cycling crashes, shrimp die-offs) emerge from identifiable mechanistic causes, not from parameter magic.
- Yes: the model's limitations are documented and honest.
EOF

create_bead G 'Phase 5 — provenance, calibration, validation, and release narrative' epic 2 "$ROOT" scientific-core,phase-5,validation,calibration,research,docs '' "${G_DESC}" "${G_COMMENT}"

# ===========================================================================
# PHASE 0 — GUARDRAILS (A1–A3)
# ===========================================================================

read -r -d '' A1_DESC <<'EOF' || true
Context:
- The current simulator already has meaningful chemistry and ecology state, but the meaning of many fields is implicit and spread across code and data.
- The review identified ambiguous units, hidden assumptions, and shortcuts whose intent should be captured before refactors begin.

Deliverable:
- A repo-local design note that maps every major pool and rate to its scientific meaning, unit, compartment, and display semantics.
- Explicit notes for known shortcuts, known nonphysical behavior, and assumptions that are acceptable for now.

Key fields to inventory (with current storage semantics):
  water.rs:12  ammonia_total_mg_n_total     — total mg N element in tank
  water.rs:13  nitrite_mg_n_total           — total mg N element
  water.rs:14  nitrate_mg_n_total           — total mg N element
  water.rs:15  phosphate_mg_p_total         — total mg P element
  water.rs:16  dissolved_oxygen_mg_total    — total mg O2
  water.rs:17  dissolved_inorganic_carbon_mg_c_total  — total mg C element
  water.rs:18  dissolved_organic_carbon_mg_c_total    — total mg C element
  water.rs:19  dissolved_organic_nitrogen_mg_n_total  — total mg N element
  water.rs:20  alkalinity_meq_total         — total milliequivalents

Likely touch points:
- crates/tank_core/src/types/*.rs
- crates/tank_core/src/systems/*.rs
- crates/tank_data/data/**/*.toml

Acceptance:
- major pools are cataloged with owner/unit/meaning
- current invariants and missing invariants are listed
- the note is good enough that a future refactor does not need to rediscover model intent from scratch
EOF

read -r -d '' A1_COMMENT <<'EOF' || true
Rationale:
- Preserve intent, not bugs. The point is to know what the current model is trying to represent before changing it.
- This note should become the "Rosetta stone" for later field renames and test updates.
- Capture uncertainties explicitly instead of letting them remain tribal knowledge.

Specific things to document:
- Which pools are N-as-element vs N-as-ion (currently all N-as-element internally, but snapshot names suggest ion)
- Which rate parameters carry implicit volume assumptions (all Monod K_s values currently do)
- Which state variables are totals vs concentrations (currently all totals in WaterState)
- Which processes are intentionally simplified vs accidentally wrong
- Where the existing 19 integration tests depend on specific numeric values vs qualitative behavior
EOF

create_bead A1 'Inventory current scientific semantics, units, invariants, and shortcuts' task 1 "$A" scientific-core,phase-0,guardrails,analysis 180 "${A1_DESC}" "${A1_COMMENT}"

# ---------------------------------------------------------------------------
# A2 — Instrumentation + Migration Scaffolding (decomposed into 3 subtasks)
# ---------------------------------------------------------------------------
# This task bundles two distinct concerns: (a) per-tick mass budget tracking
# for detecting conservation violations, and (b) save-schema versioning for
# handling state structure changes across releases. These are decomposed
# because they touch different code areas, require different testing
# strategies, and could be assigned to different developers.

read -r -d '' A2_DESC <<'EOF' || true
Context:
- The next phases will add new state fields, rename units, and re-route matter through the system.
- Refactoring without instrumentation makes it too easy to silently create or destroy mass, or to break save compatibility accidentally.

Deliverable:
- Lightweight per-tick budget instrumentation for major tracked quantities (at minimum N, C, O2 demand, and tracked ions where feasible).
- Debug hooks or test helpers that expose budget deltas without forcing them into normal gameplay output.
- Save/version scaffolding so later state changes can be migrated intentionally instead of ad hoc.

Likely touch points:
- crates/tank_core/src/invariants.rs
- crates/tank_core/src/engine.rs
- crates/tank_core/src/save.rs
- crates/tank_core/src/types/state.rs
- crates/tank_core/src/types/snapshot.rs

Acceptance:
- test code can inspect budget deltas
- save/version path exists for upcoming field additions and renames
- instrumentation can be enabled in dev/test builds without cluttering the player experience

Note: this task is decomposed into three subtasks (A2a–A2c) for independent tracking.
EOF

read -r -d '' A2_COMMENT <<'EOF' || true
Future-self notes:
- Not every action should conserve mass inside the tank. Water changes and "trim-and-remove" intentionally export matter, so instrumentation should distinguish internal conservation from explicit exports.
- Keep diagnostics developer-facing first; surface only the most useful summaries in the TUI later.
- Versioning early is cheaper than backfilling migrations after multiple incompatible changes land.
- The existing save.rs already has a version field in SaveData. Build on that rather than creating a parallel system.

Architecture note:
- Consider a BudgetLedger struct that records sources and sinks per element (N, C, O2) per tick. Engine::step_hours can snapshot the ledger before and after each system step, making it easy to pinpoint which system is violating conservation.
- The ledger should be opt-in (behind a debug flag or test helper) to avoid runtime cost in normal gameplay.
EOF

create_bead A2 'Add conservation/debug instrumentation and save-schema migration scaffolding' task 1 "$A" scientific-core,phase-0,guardrails,testing,migration 240 "${A2_DESC}" "${A2_COMMENT}"

# A2 subtask a: per-tick mass budget tracking
read -r -d '' A2a_DESC <<'EOF' || true
Context:
- The core engine (engine.rs) steps through systems hourly. Each system modifies TankState fields (water pools, biology pools, detritus). There is currently no mechanism to verify that modifications within a closed tick are mass-conserving.

Deliverable:
- A BudgetLedger or equivalent that can record per-element (N, C, O2) sources and sinks during a tick.
- The ledger should capture: which system contributed what delta, and the net balance.
- Focus on the major elements: total nitrogen (TAN + NO2 + NO3 + DON + organic N in biomass), total carbon (DIC + DOC + organic C in biomass), and total oxygen demand.

Likely touch points:
- crates/tank_core/src/engine.rs (step_hours, step loop)
- crates/tank_core/src/types/state.rs (new struct or field)
- crates/tank_core/src/invariants.rs (budget check after step)

Acceptance:
- a test can run N ticks and inspect per-tick budget deltas
- closed-system ticks (no water change, no export) show near-zero net delta
- the instrumentation compiles out or is zero-cost when not in use
EOF

read -r -d '' A2a_COMMENT <<'EOF' || true
Implementation guidance:
- The simplest approach: snapshot total-N and total-C before and after each tick, then assert delta ≈ 0 (within floating-point tolerance) for closed-system ticks.
- A more granular approach: each system function returns a BudgetDelta struct { n_in, n_out, c_in, c_out, o2_in, o2_out }, and the engine sums them.
- Start with the simple approach; upgrade to granular if debugging needs it.
- "Total N" means: ammonia_total + nitrite_total + nitrate_total + don_total + N_in_all_biomass_pools. Define this sum once as a helper.
EOF

create_bead A2a 'Implement per-tick mass budget tracking for N, C, and O2' task 1 "$A2" scientific-core,phase-0,instrumentation,testing 90 "${A2a_DESC}" "${A2a_COMMENT}"

# A2 subtask b: debug hooks and test helpers
read -r -d '' A2b_DESC <<'EOF' || true
Context:
- Budget tracking is only useful if tests can inspect it and developers can enable it during debugging.
- The existing test suite uses direct state assertions but has no budget-level inspection.

Deliverable:
- Test helper functions that wrap engine stepping and return budget deltas alongside the final state.
- Optional debug output (log or snapshot extension) that surfaces budget summaries per tick.
- Clear API so future tests can assert budget invariants with one-liners.

Likely touch points:
- crates/tank_core/tests/ (new test utility module or extension of existing helpers)
- crates/tank_core/src/engine.rs (expose budget data)

Acceptance:
- at least one existing test is retrofitted to also check budget invariants
- debug output can be enabled via feature flag or test configuration
- the API is ergonomic enough that new tests will naturally use it
EOF

read -r -d '' A2b_COMMENT <<'EOF' || true
Design note:
- Consider a test wrapper like: let (state, budget) = engine.step_hours_with_budget(n);
- Budget assertions should use a configurable tolerance (e.g., 1e-6 mg) to account for floating-point arithmetic.
- The debug output should identify the system responsible for the largest budget imbalance, making it easy to locate conservation bugs.
EOF

create_bead A2b 'Add debug hooks and test helpers for budget inspection' task 1 "$A2" scientific-core,phase-0,testing,api 60 "${A2b_DESC}" "${A2b_COMMENT}"

# A2 subtask c: save-schema migration scaffolding
read -r -d '' A2c_DESC <<'EOF' || true
Context:
- save.rs already serializes TankState with a version field. The next phases will add fields (carbonate state in D2, habitat state in E1, stage-structured shrimp in F2) and rename fields (nitrogen pool names in B7).
- Without migration scaffolding, each change risks breaking saves or requiring manual re-creation.

Deliverable:
- A migration registry or strategy in save.rs that maps version N → version N+1 state transforms.
- At minimum: detect version mismatch, apply known migrations in order, fail gracefully for unknown future versions.
- Document the migration contract so each later phase knows how to register its state changes.

Likely touch points:
- crates/tank_core/src/save.rs
- crates/tank_core/src/types/state.rs (version constant)

Acceptance:
- a test can save at version N, bump version, add a field, and load with the migration applied
- unknown version → clear error message (not silent data loss)
- the migration path is documented for later contributors
EOF

read -r -d '' A2c_COMMENT <<'EOF' || true
Future-self notes:
- The migration system doesn't need to handle arbitrary schema evolution. It needs to handle the specific field additions and renames planned for phases 1-4.
- Consider serde's #[serde(default)] for new fields and #[serde(alias)] for renames as the simplest migration strategy.
- More complex migrations (splitting a field into multiple, changing units) may need explicit transform functions per version bump.
- Keep the migration code in save.rs close to the serialization logic it modifies.
EOF

create_bead A2c 'Add save-schema versioning and migration scaffolding' task 1 "$A2" scientific-core,phase-0,migration,save 90 "${A2c_DESC}" "${A2c_COMMENT}"

# ---------------------------------------------------------------------------
# A3 — Baseline scenario envelopes
# ---------------------------------------------------------------------------

read -r -d '' A3_DESC <<'EOF' || true
Context:
- The current code already has scenario presets and regression tests for cycling, oxygen, shrimp reproduction, water changes, ambient temperature, and tank-size effects.
- Those scenarios should become explicit envelopes that future refactors compare against, even if exact trajectories change.

Deliverable:
- Baseline fixtures or notes for current scenarios such as nano_cycle, medium_planted, and warm_room.
- Envelope-based tests or documentation describing the intended qualitative behavior, not just exact current numbers.
- Coverage of the behavioral stories that matter most: cycling timeline, stable-state nutrient ranges, DO behavior, shrimp reproduction windows, thermal response, and algae/plant competition.

Likely touch points:
- crates/tank_data/data/scenarios/*.toml
- crates/tank_scenarios/src/lib.rs
- crates/tank_core/tests/ (existing test modules)

Acceptance:
- each reference scenario has a short "what should happen and why" note
- later phases can compare against qualitative envelopes rather than brittle exact traces
- tank-size, oxygen, and reproduction behavior are explicitly preserved where still desired
EOF

read -r -d '' A3_COMMENT <<'EOF' || true
Rationale:
- The goal is not to freeze today's outputs; it is to preserve the useful behavioral intent while allowing scientifically motivated changes.
- Scenario envelopes should be broad enough to survive tuning but narrow enough to catch regressions.
- These baselines will be especially useful once parameter retuning begins after concentration normalization (B6).

Existing tests to review and potentially convert to envelope-style:
- cycling.rs — TAN peak timing and magnitude, NO2 intermediate, NO3 accumulation
- dissolved_oxygen.rs — DO response to light/dark, aeration, biomass
- shrimp_population.rs — reproduction onset, steady-state population
- tank_size_thermal_response.rs — thermal inertia scaling
- water_change_mass_balance.rs — dilution accuracy
- plant_algae_integration.rs — competition dynamics

Each test should capture: "after N hours of scenario X, quantity Y should be in range [a, b] because Z."
The "because Z" part is critical — it prevents cargo-cult tolerance widening when numbers change.
EOF

create_bead A3 'Capture baseline scenario envelopes for current v0.1 behavior' task 1 "$A" scientific-core,phase-0,guardrails,testing,scenarios 240 "${A3_DESC}" "${A3_COMMENT}"

# ===========================================================================
# PHASE 1A — UNITS, CONCENTRATION SEMANTICS, AND KINETIC NORMALIZATION (B1–B8)
# ===========================================================================

read -r -d '' B1_DESC <<'EOF' || true
Context:
- Internally, several nitrogen pools are stored as mass of nitrogen (water.rs:12-19: ammonia_total_mg_n_total, etc.), while snapshots and UI labels currently read like full-ion concentrations (snapshot.rs:15-16: nitrite_mg_l, nitrate_mg_l).
- TDS and conductivity also need a clearer story about what is estimated, what is tracked, and what remains out of scope.

Deliverable:
- A written unit policy covering internal storage, helper method naming, and display naming.
- Clear decisions on when to use mg N/L, when to convert to ion-style mg/L for display, and how to label estimated TDS/conductivity.
- A pragmatic strategy for strong types/newtypes versus lighter naming conventions.

Key decisions to make:
1. Internal storage: keep as mg-element-total (current), or switch to mg-element-per-L?
   → Recommendation: keep totals internally (avoids recomputing volume on every read), provide concentration helpers.
2. Display: show mg N/L (scientific, compatible with EPA/test kit reporting) or mg-ion/L (hobby-friendly)?
   → Recommendation: offer both, label clearly. Conversion factors: NO3_ion = N × 62/14 ≈ 4.43×; NO2_ion = N × 46/14 ≈ 3.29×.
3. Type safety: full newtypes (MgNTotal, MgNPerL, MgIonPerL) or naming convention?
   → Recommendation: start with naming convention + a few key newtypes for the most error-prone conversions.

Likely touch points:
- crates/tank_core/src/types/water.rs
- crates/tank_core/src/types/snapshot.rs
- crates/tank_core/src/types/process.rs
- crates/tank_api
- crates/tank_tui

Acceptance:
- field names and helper names have a consistent convention
- display policy is written down before refactors start
- the chosen strategy is explicit enough to prevent semantic drift in future data files
EOF

read -r -d '' B1_COMMENT <<'EOF' || true
Rationale:
- This is a design decision bead, not a bike-shed bead. Make the semantics obvious enough that future contributors do not have to guess what a field means.
- Prefer clarity over type-system maximalism. A smaller number of well-named helpers can beat a forest of wrapper types if the API is disciplined.
- Decide once, then apply consistently across engine, snapshots, data files, docs, and tests.

Scientific context on units:
- EPA ammonia criteria use TAN (Total Ammonia Nitrogen) in mg N/L. Most scientific literature uses N-as-element.
- Hobby test kits typically report NO3 and NO2 as full-ion mg/L (e.g., API kit reads NO3 as ion).
- The simulator serves both audiences: developers working with scientific literature, and players accustomed to hobby test-kit values.
- The unit policy should make this dual audience explicit.
EOF

create_bead B1 'Decide internal unit taxonomy and display policy' spike 0 "$B" scientific-core,phase-1,units,api,tui 180 "${B1_DESC}" "${B1_COMMENT}"

read -r -d '' B2_DESC <<'EOF' || true
Context:
- Multiple systems currently compute limitations directly from tank totals. For example, nitrogen_cycle.rs:90 computes doc_total / (doc_total + k_doc) using state.water.dissolved_organic_carbon_mg_c_total directly.
- The code needs one canonical place to ask for concentrations, areal densities, or compartment-specific views so later systems stop re-deriving them ad hoc.

Deliverable:
- Helper methods for canonical concentration queries such as TAN, nitrite, nitrate, phosphate, DOC, DIC, and dissolved oxygen.
- Volume helpers on TankState or a ConcentrationView struct.
- Where needed, helpers for habitat/compartment area- or volume-normalized quantities.
- A small API that later systems can use instead of touching raw totals directly.

Specific helpers to implement (at minimum):
  tan_mg_n_per_l()          — TAN concentration as mg N/L
  nitrite_mg_n_per_l()      — nitrite concentration as mg N/L
  nitrate_mg_n_per_l()      — nitrate concentration as mg N/L
  doc_mg_c_per_l()          — dissolved organic carbon as mg C/L
  dic_mg_c_per_l()          — dissolved inorganic carbon as mg C/L
  do_mg_per_l()             — dissolved oxygen as mg O2/L
  phosphate_mg_p_per_l()    — phosphate as mg P/L
  alkalinity_meq_per_l()    — alkalinity as meq/L
  water_volume_l()          — from geometry (length × width × fill_height - substrate_volume)

Likely touch points:
- crates/tank_core/src/types/water.rs (new impl block or module)
- crates/tank_core/src/types/state.rs
- crates/tank_core/src/types/geometry.rs

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
- The helpers need access to both WaterState (for pool totals) and TankGeometry (for volume). Consider impl on TankState or a method that takes both.
- snapshot.rs already does some of these conversions (lines 66-78). Consolidate there to avoid duplication.
EOF

create_bead B2 'Implement canonical concentration and compartment helper APIs' task 0 "$B" scientific-core,phase-1,units,helpers,chemistry 240 "${B2_DESC}" "${B2_COMMENT}"

# ---------------------------------------------------------------------------
# B3 — Normalize nitrogen-cycle kinetics (decomposed into 5 subtasks)
# ---------------------------------------------------------------------------
# This is the single most important scientific fix in the entire backlog.
# It touches 4 distinct microbial guilds (AOB, NOB, comammox, decomposers)
# plus the parameter naming across code and data files. Decomposing into
# subtasks allows each guild's normalization to be reviewed and tested
# independently, which reduces risk and makes debugging easier.
#
# Internal ordering: B3a (AOB, establishes the pattern) → B3b-d (parallel,
# apply same pattern to other guilds) → B3e (regression test covering all).

read -r -d '' B3_DESC <<'EOF' || true
Context:
- nitrogen_cycle.rs currently uses several Monod or half-saturation terms expressed against absolute totals. All calls to monod_rate() (line 342-352) pass tank totals for substrate and total-based K_s values.
- This creates nonphysical tank-size behavior when two tanks share the same concentration but differ in total liters.

Concrete example from current defaults:
  With 1 mg/L N available and aob_k_tan_mg = 0.5:
  - In a 10L tank: S=10, K=0.5  → factor = 10/(10+0.5) = 0.952
  - In a 100L tank: S=100, K=0.5 → factor = 100/(100+0.5) = 0.995
  But the chemistry is identical. The 100L tank appears nearly fully saturated while the 10L tank shows meaningful limitation. This is backwards from reality.

Deliverable:
- Refactor AOB/NOB/comammox and decomposer-related kinetics to use concentration-based inputs from the B2 helper layer.
- Rename parameters in code and data so their units communicate the new semantics.
- Update tests to confirm same concentration gives the same limitation behavior regardless of tank size.

Likely touch points:
- crates/tank_core/src/systems/nitrogen_cycle.rs (lines 76-93, 137-180, 213-219, 342-352)
- crates/tank_core/src/types/process.rs (lines 38,48,50,59,61,70,72)
- crates/tank_data/data/process/default.toml

Acceptance:
- no Monod-style nitrogen/oxygen limitation term depends on raw total tank mass when it should depend on concentration
- parameter names and docs reflect the new units
- a regression test explicitly checks the tank-size-independence case

Note: this task is decomposed into five subtasks (B3a–B3e) for per-guild tracking.
EOF

read -r -d '' B3_COMMENT <<'EOF' || true
Scientific background:
- Monod (1949): μ = μ_max × [S]/(K_s + [S]) where [S] is substrate CONCENTRATION (mg/L), not total mass. K_s is the half-saturation constant — the concentration at which growth rate = half of maximum.
- For nitrifying bacteria in aquarium biofilters:
  - AOB K_s(TAN) ≈ 0.5–2.0 mg N/L, K_s(DO) ≈ 0.3–1.0 mg O2/L
  - NOB K_s(NO2) ≈ 0.2–1.0 mg N/L, K_s(DO) ≈ 0.5–1.5 mg O2/L (NOB are more sensitive to low O2)
  - Comammox K_s(TAN) is typically lower than AOB, ≈ 0.05–0.5 mg N/L (competitive advantage at low ammonia)
  - Decomposer K_s(DOC) ≈ 1–10 mg C/L
- These are CONCENTRATION values. The current code stores them as total-mass values, which only accidentally work for one specific tank size.

Rationale:
- This bead is the heart of the scientific refactor. Until it lands, larger tanks appear "more saturated" for nonphysical reasons.
- Do not simply divide by volume in a scattered way; route through the helper layer (B2) so the semantics stay centralized.
- When in doubt, choose the simpler scientifically credible formulation rather than a more elaborate but poorly constrained one.

Retuning expectations:
- After normalization, the old K_s values become meaningless. New values should be set to literature-reasonable concentration ranges and then tuned against the baseline envelopes (A3).
- This is expected. The point is to get the math right first, then tune for qualitative behavior.
EOF

create_bead B3 'Normalize nitrogen-cycle kinetics to concentration-based terms' task 0 "$B" scientific-core,phase-1,kinetics,nitrogen,chemistry 300 "${B3_DESC}" "${B3_COMMENT}"

# B3 subtask a: AOB normalization (establishes the pattern for all guilds)
read -r -d '' B3a_DESC <<'EOF' || true
Context:
- AOB (ammonia-oxidizing bacteria) perform the first step of nitrification: TAN → NO2.
- In nitrogen_cycle.rs, AOB kinetics are at lines 133-155:
  - Line 139: DO limitation uses do_budget / (do_budget + pp.aob_k_do_mg) with do_budget in mg total
  - Line 145: TAN limitation via monod_rate() with state.water.ammonia_total_mg_n_total (mg total) and pp.aob_k_tan_mg (mg total)
- AOB is the most safety-critical guild because TAN removal rate directly determines cycling timeline and shrimp safety.

Deliverable:
- Rewrite AOB TAN and DO limitation to use concentration helpers from B2.
- Change monod_rate() call to pass concentration (mg/L) and concentration-based K_s (mg/L).
- Rename aob_k_tan_mg → aob_k_tan_mg_n_per_l (or per the unit policy from B1).
- Update default.toml value to a concentration equivalent (literature range: 0.5–2.0 mg N/L for K_s(TAN)).
- Add inline comment explaining the Monod form: μ = μ_max × [S]/(K_s + [S]).

Acceptance:
- AOB limitation factor is computed from concentration, not total mass
- parameter name unambiguously communicates concentration units
- existing cycling tests still pass (may need tolerance adjustment)
EOF

read -r -d '' B3a_COMMENT <<'EOF' || true
Why AOB first:
- AOB sets the pattern for NOB, comammox, and decomposers. Getting it right — including the helper call convention, parameter rename strategy, and test update approach — makes the remaining guilds straightforward.
- TAN is the primary shrimp-safety parameter. Correctness here has the highest gameplay and scientific impact.
- The monod_rate() utility function (nitrogen_cycle.rs:342-352) may need its signature updated to expect concentration. Do that as part of this subtask.

Retuning note:
- Current aob_k_tan_mg = 0.5 mg total. In a default 12L nano tank (30×20×20cm), this is ~0.042 mg N/L. That's unusually low — literature values are 0.5–2.0 mg N/L. The old value likely compensated for the total-mass formulation. Set the new concentration-based value to a literature-reasonable range.
EOF

create_bead B3a 'Normalize AOB TAN and DO limitation to concentration-based kinetics' task 0 "$B3" scientific-core,phase-1,kinetics,nitrogen,aob 90 "${B3a_DESC}" "${B3a_COMMENT}"

# B3 subtask b: NOB normalization
read -r -d '' B3b_DESC <<'EOF' || true
Context:
- NOB (nitrite-oxidizing bacteria) perform the second step: NO2 → NO3.
- In nitrogen_cycle.rs, NOB kinetics are at lines 205-236:
  - Line 213: DO limitation uses do_budget / (do_budget + pp.nob_k_do_mg) with do_budget in mg total
  - Line 219: nitrite limitation via monod_rate() with state.water.nitrite_mg_n_total (mg total) and pp.nob_k_nitrite_mg (mg total)

Deliverable:
- Rewrite NOB NO2 and DO limitation to use concentration helpers following the AOB pattern.
- Rename nob_k_nitrite_mg → nob_k_nitrite_mg_n_per_l, nob_k_do_mg → nob_k_do_mg_per_l.
- Update default.toml values to concentration equivalents.

Acceptance:
- NOB limitation uses concentration, not total mass
- parameter names communicate concentration units
- cycling tests still show NO2 intermediate peak and NO3 accumulation
EOF

read -r -d '' B3b_COMMENT <<'EOF' || true
NOB-specific notes:
- NOB are generally more sensitive to low DO than AOB. Literature K_s(DO) for NOB ≈ 0.5–1.5 mg O2/L vs AOB ≈ 0.3–1.0 mg O2/L. This matters because low-O2 conditions can create NO2 accumulation (incomplete nitrification), which is a real and dangerous aquarium phenomenon.
- The relative K_s(DO) values between AOB and NOB should be preserved after retuning — NOB should have higher K_s(DO) to maintain the correct selectivity.
EOF

create_bead B3b 'Normalize NOB nitrite and DO limitation to concentration-based kinetics' task 0 "$B3" scientific-core,phase-1,kinetics,nitrogen,nob 60 "${B3b_DESC}" "${B3b_COMMENT}"

# B3 subtask c: comammox normalization
read -r -d '' B3c_DESC <<'EOF' || true
Context:
- Comammox bacteria oxidize TAN directly to NO3 (complete ammonia oxidation).
- In nitrogen_cycle.rs, comammox kinetics are at lines 163-195:
  - Line 173: DO limitation uses do_budget / (do_budget + pp.comammox_k_do_mg) with do_budget in mg total
  - Line 179: TAN limitation via monod_rate() with tan_after_aob (mg total) and pp.comammox_k_tan_mg (mg total)

Deliverable:
- Rewrite comammox TAN and DO limitation to use concentration helpers.
- Rename comammox_k_tan_mg → comammox_k_tan_mg_n_per_l, comammox_k_do_mg → comammox_k_do_mg_per_l.
- Update default.toml values.

Acceptance:
- comammox limitation uses concentration, not total mass
- parameter names communicate units
- comammox retains competitive advantage at low TAN (lower K_s than AOB)
EOF

read -r -d '' B3c_COMMENT <<'EOF' || true
Comammox-specific notes:
- Comammox is a relatively recent discovery (Daims et al. 2015, van Kessel et al. 2015). These organisms are increasingly recognized in aquarium biofilters.
- Key characteristic: lower K_s(TAN) than conventional AOB, meaning comammox dominate at low ammonia concentrations. This should be preserved after retuning — comammox K_s(TAN) should be lower than AOB K_s(TAN).
- The current comammox_vmax_fraction (process.rs:68) scales comammox rate relative to AOB. This is a reasonable simplification to maintain.
EOF

create_bead B3c 'Normalize comammox TAN and DO limitation to concentration-based kinetics' task 0 "$B3" scientific-core,phase-1,kinetics,nitrogen,comammox 60 "${B3c_DESC}" "${B3c_COMMENT}"

# B3 subtask d: decomposer normalization
read -r -d '' B3d_DESC <<'EOF' || true
Context:
- Decomposers in nitrogen_cycle.rs mineralize DOC and DON, driving part of the detritus-to-nutrient recycling pathway.
- In nitrogen_cycle.rs, decomposer kinetics are at lines 76-110:
  - Line 83: DO limitation uses do_total / (do_total + 2.0) with do_total in mg total. Note: the "2.0" is a hardcoded total-mass K_s, not even a named parameter.
  - Line 90: DOC limitation uses doc_total / (doc_total + decomposer_k_doc_mg) with both in mg total.
  - doc_total = state.water.dissolved_organic_carbon_mg_c_total (mg C total)

Deliverable:
- Rewrite decomposer DOC and DO limitation to use concentration helpers.
- Extract the hardcoded 2.0 mg DO total into a named parameter (decomposer_k_do_mg_per_l or similar).
- Rename decomposer_k_doc_mg → decomposer_k_doc_mg_c_per_l.
- Update default.toml values.

Acceptance:
- decomposer limitation uses concentration, not total mass
- no hardcoded total-mass constants remain in the decomposer section
- mineralization rate is scale-independent
EOF

read -r -d '' B3d_COMMENT <<'EOF' || true
Decomposer-specific notes:
- The hardcoded 2.0 on line 83 is an especially dangerous hidden assumption. In a 10L tank, this effectively means K_s(DO) ≈ 0.2 mg/L (reasonable). In a 100L tank, K_s(DO) ≈ 0.02 mg/L (decomposers are almost never O2-limited, which may not be realistic for heavily loaded tanks).
- Decomposer K_s(DOC) in freshwater systems is typically 1–10 mg C/L. The current decomposer_k_doc_mg = 5.0 (mg total) is ~0.42 mg C/L in a 12L tank — quite low, suggesting decomposers are easily saturated. After normalization, this should be re-examined.
- Decomposer activity drives the DOC → DIC pathway, which later connects to carbonate chemistry (Phase 2). Getting the rate right here matters for the entire carbon cycle.
EOF

create_bead B3d 'Normalize decomposer DOC and DO limitation to concentration-based kinetics' task 0 "$B3" scientific-core,phase-1,kinetics,decomposer,doc 60 "${B3d_DESC}" "${B3d_COMMENT}"

# B3 subtask e: tank-size-independence regression test
read -r -d '' B3e_DESC <<'EOF' || true
Context:
- After normalizing all four guilds (AOB, NOB, comammox, decomposer), the project needs a definitive test proving that the fix works: same concentration in different volumes → same Monod factor.

Deliverable:
- A regression test that:
  1. Creates two tank states with different volumes (e.g., 10L and 100L) but identical concentrations of TAN, NO2, DOC, and DO.
  2. Runs one tick of nitrogen_cycle on each.
  3. Asserts that the Monod limitation factors are equal within floating-point tolerance.
  4. Asserts that the concentration changes per hour are equal (not just the factors).
- The test should be named clearly (e.g., test_concentration_kinetics_are_volume_independent) and include comments explaining what it protects.

Acceptance:
- the test passes after all B3a-d subtasks are complete
- the test would have failed against the pre-refactor code (verify this before the refactor if possible)
- the test is easy to extend when later systems are also normalized
EOF

read -r -d '' B3e_COMMENT <<'EOF' || true
Test design notes:
- Use a controlled setup: disable plant/algae/shrimp/microfauna systems to isolate nitrogen kinetics.
- Set identical concentrations but 10× volume difference: this maximizes the signal if totals leak through.
- Assert both the Monod factor AND the per-liter concentration delta. A test that only checks the factor could miss volume leaks in the rate-to-mass conversion step.
- Consider also testing an edge case: very small tank (1L) and very large tank (1000L) at the same concentration to stress the extremes.
- This test becomes a permanent guardrail: any future code that accidentally reintroduces total-mass kinetics will break it.
EOF

create_bead B3e 'Add tank-size-independence regression test for all nitrogen kinetics' task 0 "$B3" scientific-core,phase-1,testing,kinetics,nitrogen 60 "${B3e_DESC}" "${B3e_COMMENT}"

# ---------------------------------------------------------------------------
# B4 — Plant normalization (not decomposed — cohesive single-system work)
# ---------------------------------------------------------------------------

read -r -d '' B4_DESC <<'EOF' || true
Context:
- Plant growth currently depends on nutrient and light logic that is directionally useful but not yet normalized through explicit concentration semantics.
- plant_growth.rs computes nutrient limitation using available nitrogen pools but the formulation may use totals in some paths.
- Rooted plants will also need a cleaner separation between water-column and substrate access in later phases (E4).

Deliverable:
- Refactor plant nutrient limitation terms to use concentration-based helpers and explicit semantics for nutrient source (water column vs substrate).
- Keep the model simple enough for tuning now, while leaving hooks for later root-zone work.
- Update parameter names where they currently imply total-mass behavior (e.g., plant_half_saturation_n_mg_total in process.rs).

Likely touch points:
- crates/tank_core/src/systems/plant_growth.rs
- crates/tank_core/src/systems/light.rs
- crates/tank_core/src/types/process.rs (plant_half_saturation_n_mg_total and related)
- crates/tank_data/data/plants/*.toml

Acceptance:
- plant limitation terms are concentration-based and no longer implicitly depend on tank size
- rooted and water-column uptake assumptions are explicit in code comments or docs
- tests or scenario notes cover at least one plant-heavy case after normalization
EOF

read -r -d '' B4_COMMENT <<'EOF' || true
Future-self notes:
- Keep the first pass modest. The immediate goal is correct semantics, not a full plant physiology simulator.
- Leave room for later habitat/redox work (E4) by making nutrient-source assumptions explicit now. For example: "rooted plants access N from both water column and substrate porewater" should be a commented assumption that can later be replaced with actual habitat-specific queries.
- Retuning may matter more than model complexity at this stage.
- The plant_half_saturation_n_mg_total parameter name tells you it's a total — rename to plant_half_saturation_n_mg_n_per_l or similar.

Scientific notes:
- Rooted aquarium plants (e.g., Echinodorus, Cryptocoryne) obtain nutrients from both root uptake (substrate) and foliar uptake (water column). The relative importance depends on species and substrate nutrient availability.
- Water-column feeders (e.g., Rotala, floating plants) depend entirely on dissolved nutrients.
- The current model treats both plant types similarly. Phase 3 (E4) will add substrate-zone differentiation.
EOF

create_bead B4 'Normalize plant nutrient uptake and growth limitation semantics' task 1 "$B" scientific-core,phase-1,plants,kinetics,ecology 240 "${B4_DESC}" "${B4_COMMENT}"

# ---------------------------------------------------------------------------
# B5 — Algae normalization (not decomposed — cohesive single-system work)
# ---------------------------------------------------------------------------

read -r -d '' B5_DESC <<'EOF' || true
Context:
- Algae growth is one of the main visible outcomes players care about, but it currently sits on top of simplified and partly size-coupled logic.
- algae_growth.rs:31-38 computes nutrient limitation that may use totals.
- The next version should make algae respond to light, nutrients, and temperature through clearer concentration-based rules.

Deliverable:
- Refactor algae limitation terms to use canonical concentration helpers.
- Preserve a simple model shape while making light/nutrient/temperature interactions explicit enough to tune.
- Prepare algae behavior to interact later with habitats, periphyton, and CO2 (Phases 2 and 3).

Likely touch points:
- crates/tank_core/src/systems/algae_growth.rs
- crates/tank_core/src/systems/light.rs
- crates/tank_core/src/types/process.rs (algae_half_saturation_n_mg_total)

Acceptance:
- no nutrient limitation term depends directly on raw tank totals
- algae behavior remains playable/tunable after the refactor
- code comments explain what interactions are represented versus intentionally abstracted
EOF

read -r -d '' B5_COMMENT <<'EOF' || true
Rationale:
- Algae blooms are a core emergent outcome for this project, so the growth model must at least be mechanistically sane.
- Avoid premature overfitting. A well-explained low-dimensional model is preferable to a more complex one with weak parameter grounding.
- This bead sets up later habitatized periphyton work (E3) without requiring it immediately.

Scientific notes:
- Algae growth in aquaria is primarily limited by: light availability, dissolved N (usually as NH4+ or NO3), dissolved P, and temperature.
- The current model likely over-indexes on N limitation because P cycling is not closed. This is acceptable for v0.2 but should be noted in the algae system comments.
- Periphyton (attached algae) and planktonic algae have different light and nutrient strategies. The current model lumps them. Phase 3 (E3) will split them.
EOF

create_bead B5 'Normalize algae kinetics around concentration, light, and temperature interactions' task 1 "$B" scientific-core,phase-1,algae,kinetics,ecology 240 "${B5_DESC}" "${B5_COMMENT}"

# ---------------------------------------------------------------------------
# B6 — Parameter retuning after normalization
# ---------------------------------------------------------------------------

read -r -d '' B6_DESC <<'EOF' || true
Context:
- Once kinetics are normalized (B3, B4, B5), old parameter values are no longer guaranteed to mean the same thing.
- Data files and scenario presets need an explicit retuning pass so the simulator remains stable and interpretable.

Deliverable:
- Review and retune relevant values in process, plant, shrimp, source-water, and scenario data files after the semantic changes from B3-B5.
- Update comments in data files where unit meanings or intended ranges changed.
- Confirm that the main shipped scenarios still tell useful stories after retuning.

Likely touch points:
- crates/tank_data/data/process/default.toml
- crates/tank_data/data/plants/*.toml
- crates/tank_data/data/scenarios/*.toml
- any supporting docs created in A1/A3

Acceptance:
- parameter files no longer silently carry pre-refactor assumptions
- at least the main scenarios are rerun and checked after retuning
- changed semantics are documented where future maintainers will actually see them
EOF

read -r -d '' B6_COMMENT <<'EOF' || true
Future-self notes:
- Retuning is part of the scientific refactor, not an afterthought.
- Be explicit about whether a change is "new science" versus "compensating for old unit mistakes."
- Capture why tuned values moved, especially if they are temporary envelopes pending later habitat or carbonate work.
- Use the baseline envelopes from A3 as the tuning target: "nano_cycle should still show cycling completion within 500-800 hours" is more useful than "make the numbers look right."

Process:
1. Run each shipped scenario with the old parameters under new kinetics.
2. Compare against A3 envelopes.
3. Adjust K_s values to literature-reasonable concentration ranges.
4. Iterate until scenarios tell qualitatively correct stories.
5. Document the new values with rationale and confidence level.
EOF

create_bead B6 'Retune process parameters and scenario defaults after normalization' task 1 "$B" scientific-core,phase-1,retuning,data,scenarios 240 "${B6_DESC}" "${B6_COMMENT}"

# ---------------------------------------------------------------------------
# B7 — Snapshot/API field names
# ---------------------------------------------------------------------------

read -r -d '' B7_DESC <<'EOF' || true
Context:
- Snapshot and UI fields such as nitrite_mg_l and nitrate_mg_l (snapshot.rs:15-16) currently risk misleading users about whether values are nitrogen-as-N or full-ion concentrations.
- As the simulation gets more scientific, ambiguous output language becomes a product problem as well as an engineering problem.

Deliverable:
- Rename snapshot/API fields where needed or add explicit conversion fields for display.
- Update TUI/API text so the displayed chemistry matches the internal semantics chosen in B1.
- Keep backward compatibility considerations visible if save/load or API consumers are affected.

Implementation options (per B1 decision):
  Option A: Rename to nitrite_mg_n_per_l, nitrate_mg_n_per_l (explicit N-as-element).
  Option B: Add dual fields: nitrite_mg_n_per_l (internal) + nitrite_mg_ion_per_l (display).
  Option C: Keep short names but add a unit label field or documentation.

Likely touch points:
- crates/tank_core/src/types/snapshot.rs
- crates/tank_api (response structs)
- crates/tank_tui (rendering logic)
- save/schema code if field names change

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

Conversion factors (for reference):
  NO3 as ion = N × (62.004/14.007) ≈ N × 4.427
  NO2 as ion = N × (46.005/14.007) ≈ N × 3.284
  NH4+ as ion = N × (18.039/14.007) ≈ N × 1.288
  NH3 as molecule = N × (17.031/14.007) ≈ N × 1.216
  PO4 as ion = P × (94.971/30.974) ≈ P × 3.066

These conversions should be defined as named constants, not magic numbers scattered through rendering code.
EOF

create_bead B7 'Update snapshot/API field names and display conversions' task 1 "$B" scientific-core,phase-1,api,tui,units 180 "${B7_DESC}" "${B7_COMMENT}"

# ---------------------------------------------------------------------------
# B8 — TDS/conductivity accounting
# ---------------------------------------------------------------------------

read -r -d '' B8_DESC <<'EOF' || true
Context:
- Current TDS/conductivity output undercounts relevant dissolved species and risks sounding more precise than it is.
- water.rs total_tracked_ions_mg() only sums Ca, Mg, Na, K, bicarbonate, chloride, and sulfate. Snapshot TDS is tracked_ions / volume, and conductivity is estimated as tds / 0.65.
- Missing from displayed TDS: nitrate, ammonium, phosphate, DOC contribution, and any future fertilizers.

Deliverable:
- Review which dissolved species should contribute to displayed TDS/conductivity in the current model scope.
- Add missing tracked contributors where the state already exists, and rename/relabel the metric where estimation remains coarse.
- Ensure the TUI/API explain what the number means.

Likely touch points:
- crates/tank_core/src/types/water.rs (total_tracked_ions_mg method)
- crates/tank_core/src/types/snapshot.rs
- crates/tank_tui

Acceptance:
- displayed TDS/conductivity is either materially improved or explicitly framed as an estimate
- tracked ions and omitted contributors are documented
- future fertilizers/ions can be added without redefining the metric from scratch
EOF

read -r -d '' B8_COMMENT <<'EOF' || true
Future-self notes:
- Honest approximation beats false precision. If the number is "tracked major ions" rather than true TDS, say so.
- The player-facing goal is actionable understanding: what changed, why, and whether to trust the number for husbandry decisions.
- Keep the computation extensible because later fertilizer, chloride, and carbonate work will change what "tracked dissolved solids" means.

Scientific notes:
- True TDS requires measuring all dissolved species including organics. In practice, most aquarium test kits estimate TDS from conductivity.
- The TDS/0.65 = conductivity conversion is a rough approximation that varies with ion composition. For freshwater, the factor ranges from 0.55 to 0.75 depending on dominant ions.
- Consider renaming to "estimated TDS (major ions)" or similar.
EOF

create_bead B8 'Expand TDS/conductivity accounting and label any remaining estimates honestly' task 2 "$B" scientific-core,phase-1,tds,conductivity,ui 180 "${B8_DESC}" "${B8_COMMENT}"

# ===========================================================================
# PHASE 1B — MASS CONSERVATION AND HUSBANDRY ACTION SEMANTICS (C1–C6)
# ===========================================================================

read -r -d '' C1_DESC <<'EOF' || true
Context:
- Shrimp, microfauna, feeding, decomposition, and plant trimming all move matter around the system, but the current code does not consistently say where consumed or removed material goes.
- Before implementing fixes, the model needs one explicit routing policy for assimilation, excretion, feces, respiration, detritus, and exported biomass.

Deliverable:
- A short design note or code comment standard describing how C/N/energy-like quantities move through consumer and maintenance actions.
- Clear rules for what stays in-tank, what becomes waste, and what counts as export.
- A documented stance on where the model remains intentionally lumped/abstract.

Routing policy to define (at minimum):
  Ingestion → assimilation efficiency (fraction retained as body mass/reserve)
  Ingestion → fecal fraction (→ fine detritus)
  Assimilation → respiration fraction (→ O2 demand + DIC production)
  Assimilation → excretion fraction (→ TAN, dissolved N)
  Plant trimming → export (removed from system) vs leave-cuttings (→ fine detritus)
  Feed input → uneaten fraction (→ DOC/DON leaching → fine detritus)

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs
- crates/tank_core/src/systems/microfauna.rs
- crates/tank_core/src/engine.rs
- crates/tank_core/src/systems/nitrogen_cycle.rs

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

Scientific reference points:
- For freshwater invertebrate grazers, typical assimilation efficiency ≈ 40-70% of ingested organic matter.
- Fecal production ≈ 30-60% of ingestion (inverse of assimilation efficiency).
- Of assimilated material: ~60-80% goes to respiration (energy), ~10-20% to growth/reserve, ~10-20% excreted as dissolved waste (TAN, urea, DON).
- These are order-of-magnitude guidelines, not precise values. The routing conventions should use named parameters so they can be tuned later.
EOF

create_bead C1 'Define closed-loop matter-routing conventions for consumers and maintenance actions' task 0 "$C" scientific-core,phase-1,mass-balance,design,actions 180 "${C1_DESC}" "${C1_COMMENT}"

# ---------------------------------------------------------------------------
# C2 — Shrimp ingestion loop (decomposed into 4 subtasks)
# ---------------------------------------------------------------------------
# The shrimp feeding loop is the most visible conservation violation in the
# current model. Decomposing into subtasks along the metabolic pathway
# (assimilation, excretion, feces, respiration) makes each pathway
# independently testable and reviewable.

read -r -d '' C2_DESC <<'EOF' || true
Context:
- Shrimp currently remove periphyton and fine detritus (shrimp.rs:136-146) without returning enough of that mass to the system.
- state.animal.daily_food_consumed_g records how much was eaten (line 147) but no mass flows back as feces, dissolved waste, or O2 demand.
- This makes grazing look cleaner and less bioload-generating than it should.

Deliverable:
- Refactor shrimp feeding so consumed matter is partitioned into at least:
  1. assimilation → body reserve / condition
  2. excretion → TAN (dissolved N)
  3. feces → fine detritus
  4. respiration → O2 demand + DIC production
- Keep the first pass simple but mass-aware.
- Surface any new parameters clearly in process or species data.

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs (lines 119-148)
- crates/tank_core/src/types/biology.rs (AnimalState may need reserve field)
- crates/tank_core/src/types/process.rs (new routing parameters)
- crates/tank_data/data/shrimp/neocaridina_davidi.toml

Acceptance:
- shrimp grazing no longer deletes matter by default
- at least one regression test shows conservation behavior through the shrimp loop
- the new routing integrates cleanly with nitrogen/DIC/DO systems rather than bypassing them

Note: this task is decomposed into four subtasks (C2a–C2d) for per-pathway tracking.
EOF

read -r -d '' C2_COMMENT <<'EOF' || true
Future-self notes:
- Keep the first implementation transparent. A few explicit fractions with clear names are better than opaque "efficiency" magic.
- This bead is a prerequisite for later molt/reproduction realism (F1-F4) because condition and stress should emerge from the same intake/waste loop.
- Avoid double-counting with decomposition or generic waste systems; pick one owner for each transfer.

Mass balance example (for 1g of periphyton consumed):
- Assimilation efficiency = 0.5 → 0.5g assimilated, 0.5g feces
- Of assimilated 0.5g: ~0.35g respired (→ O2 demand, DIC), ~0.05g excreted (→ TAN), ~0.10g retained (→ body reserve)
- Feces 0.5g → fine detritus (which then enters decomposition pathway)
- N content: if periphyton is ~5% N by mass, 1g consumed = 0.05g N. Of that, ~0.025g N in feces, ~0.005g N excreted as TAN, ~0.005g N retained, ~0.015g N respired as part of organic matter oxidation.
- These fractions should be named parameters, not hardcoded.
EOF

create_bead C2 'Implement shrimp ingestion → assimilation → excretion → feces → respiration loop' task 0 "$C" scientific-core,phase-1,mass-balance,shrimp,ecology 240 "${C2_DESC}" "${C2_COMMENT}"

# C2 subtask a: assimilation pathway
read -r -d '' C2a_DESC <<'EOF' || true
Deliverable:
- Add an assimilation efficiency parameter (e.g., shrimp_assimilation_efficiency: 0.5) to process params or shrimp species data.
- After shrimp.rs computes total consumed (currently line 147), partition consumed mass into assimilated and fecal fractions.
- Assimilated mass goes toward body reserve or condition. If AnimalState doesn't have a reserve field, add one with appropriate initialization.
- Document the N and C content assumptions for consumed food (periphyton and detritus have different compositions).

Acceptance:
- consumed food is split into assimilated and fecal fractions using a named parameter
- assimilated mass is tracked (even if initially it just feeds a reserve counter)
- the split is testable: assert assimilated + fecal = consumed within tolerance
EOF

read -r -d '' C2a_COMMENT <<'EOF' || true
Implementation notes:
- The body reserve can start as a simple f64 (reserve_g) on AnimalState that accumulates assimilated mass. Later shrimp realism (Phase 4) will use this for condition, molt readiness, and reproduction decisions.
- Assimilation efficiency varies with food quality. Periphyton (living biofilm) typically has higher assimilation efficiency (~50-70%) than detritus (~30-50%). Consider using a single average efficiency for the first pass and splitting later.
- The C:N ratio of consumed food matters for how much N vs C enters the assimilation pathway. Periphyton C:N ≈ 6-10 (relatively N-rich); detritus C:N ≈ 10-20 (more C-rich).
EOF

create_bead C2a 'Implement shrimp assimilation pathway with body reserve tracking' task 0 "$C2" scientific-core,phase-1,mass-balance,shrimp,assimilation 60 "${C2a_DESC}" "${C2a_COMMENT}"

# C2 subtask b: excretion pathway
read -r -d '' C2b_DESC <<'EOF' || true
Deliverable:
- Add an excretion fraction parameter (e.g., shrimp_excretion_fraction_of_assimilated: 0.10).
- Route the excretion fraction of assimilated N as TAN back into state.water.ammonia_total_mg_n_total.
- This represents dissolved nitrogenous waste (primarily ammonia in aquatic invertebrates).

Acceptance:
- shrimp grazing now generates TAN in proportion to food consumed
- the TAN contribution is visible in nitrogen budget diagnostics
- tests can verify: more shrimp eating more food → more TAN production
EOF

read -r -d '' C2b_COMMENT <<'EOF' || true
Scientific background:
- Aquatic invertebrates are primarily ammonotelic — they excrete nitrogenous waste as ammonia (NH3/NH4+).
- For freshwater shrimp, ammonia excretion is typically 5-15% of assimilated nitrogen.
- This is one of the most important conservation fixes because it closes the N loop: food → shrimp → ammonia → nitrification → nitrate. Without it, shrimp grazing reduces bioload instead of contributing to it.
- The TAN production from shrimp excretion should be proportional to the N content of assimilated food, not just the total mass.
EOF

create_bead C2b 'Implement shrimp excretion pathway returning TAN to water' task 0 "$C2" scientific-core,phase-1,mass-balance,shrimp,excretion 60 "${C2b_DESC}" "${C2b_COMMENT}"

# C2 subtask c: fecal pathway
read -r -d '' C2c_DESC <<'EOF' || true
Deliverable:
- Route the fecal fraction (1 - assimilation_efficiency) of consumed food back to state.detritus.fine_detritus_g_total.
- This represents undigested food that re-enters the decomposition pathway.

Acceptance:
- feces are returned to fine detritus pool, closing the particle loop
- the detritus pool grows rather than shrinks when shrimp graze heavily (because feces partially replace consumed detritus)
- total particulate matter (consumed - feces) = assimilated (mass balance check)
EOF

read -r -d '' C2c_COMMENT <<'EOF' || true
Implementation notes:
- This is the simplest subtask: feces = consumed × (1 - assimilation_efficiency).
- The fecal matter has different C:N ratio than the original food (higher C:N because N is preferentially assimilated). For the first pass, using the same composition as the source food is acceptable; later refinement can adjust.
- Shrimp feces are a significant source of fine particulate organic matter in real aquaria. In heavily stocked tanks, fecal accumulation drives decomposer activity and oxygen demand.
EOF

create_bead C2c 'Implement shrimp fecal pathway returning undigested matter to detritus' task 0 "$C2" scientific-core,phase-1,mass-balance,shrimp,feces 45 "${C2c_DESC}" "${C2c_COMMENT}"

# C2 subtask d: respiration pathway and conservation test
read -r -d '' C2d_DESC <<'EOF' || true
Deliverable:
- Add a respiration fraction parameter (fraction of assimilated energy that goes to metabolic respiration).
- Route respiration: consume O2 (increment dissolved_oxygen demand) and produce DIC (increment DIC pool).
- Add a conservation regression test that runs a shrimp-grazing scenario and verifies N and C budget closure within tolerance.

Acceptance:
- shrimp metabolic activity generates O2 demand proportional to feeding rate
- DIC production from shrimp respiration is non-zero and directed correctly
- conservation test shows total N and total C are preserved (within tolerance) for a closed-system tick with shrimp grazing
EOF

read -r -d '' C2d_COMMENT <<'EOF' || true
Scientific background:
- Respiration: (CH2O) + O2 → CO2 + H2O (simplified). For every mg of organic C respired, ~2.67 mg O2 is consumed and ~3.67 mg CO2 is produced (as mg CO2, or ~1.0 mg C as DIC).
- This stoichiometry means shrimp feeding generates O2 demand in proportion to how much food they process — a key bioload signal that's currently missing.
- The respiration DIC should feed into the same DIC pool that later carbonate chemistry (Phase 2) will use. This is how animal respiration affects pH in real aquaria.

Conservation test design:
- Set up a closed tank (no water change, no aeration, no plants/algae) with shrimp and food source.
- Run N ticks. Assert: initial_total_N ≈ final_total_N and initial_total_C ≈ final_total_C (within tolerance for floating-point).
- Also assert: DO decreased, DIC increased, TAN increased, detritus changed as expected.
EOF

create_bead C2d 'Implement shrimp respiration O2/DIC pathway and add conservation regression test' task 0 "$C2" scientific-core,phase-1,mass-balance,shrimp,respiration,testing 75 "${C2d_DESC}" "${C2d_COMMENT}"

# ---------------------------------------------------------------------------
# C3 — Microfauna matter routing (not decomposed — smaller scope)
# ---------------------------------------------------------------------------

read -r -d '' C3_DESC <<'EOF' || true
Context:
- Microfauna currently consume periphyton/detritus in a way that also deletes matter from the system (microfauna.rs:45-55). Periphyton consumed at line 48 and detritus at line 52-53 are subtracted from their pools but never returned as waste.
- Even if the microfauna model remains abstract, it still needs explicit routing to waste, biomass, and respiration-like demand.

Deliverable:
- Refactor microfauna feeding to follow the same closed-loop conventions defined in C1.
- Keep the abstraction lightweight while ensuring it does not silently erase load from the tank.
- Ensure the new loop interacts correctly with detritus and nutrient recycling.
- Reuse the routing pattern established in C2 (assimilation/feces/excretion/respiration fractions).

Likely touch points:
- crates/tank_core/src/systems/microfauna.rs (lines 45-55)
- any shared helper code created for C2
- crates/tank_core/src/types/process.rs (microfauna parameters at lines 119-123)

Acceptance:
- microfauna consumption no longer acts as a hidden sink
- routing is consistent with the shrimp conventions where appropriate
- tests or budget diagnostics confirm the change
EOF

read -r -d '' C3_COMMENT <<'EOF' || true
Rationale:
- The microfauna model can stay simple, but it cannot keep breaking the nutrient loop if the simulator is supposed to teach ecosystem behavior.
- Reuse conventions and helpers from the shrimp work (C2) instead of inventing a second bookkeeping language.
- The goal is ecological credibility, not species-by-species realism.
- Microfauna consume less total mass than shrimp but their consumption still matters for the budget because they process periphyton and fine detritus continuously.

Implementation note:
- The microfauna system uses a population_index (0-1 scale) rather than discrete counts. Routing fractions should be proportional to consumption amount, just like shrimp.
- Microfauna excretion products go to the same pools as shrimp excretion: TAN for N waste, DIC for respired C, fine detritus for feces.
EOF

create_bead C3 'Implement microfauna matter routing and recycling' task 1 "$C" scientific-core,phase-1,mass-balance,microfauna,ecology 180 "${C3_DESC}" "${C3_COMMENT}"

# ---------------------------------------------------------------------------
# C4 — Feed/detritus/DOC bookkeeping audit
# ---------------------------------------------------------------------------

read -r -d '' C4_DESC <<'EOF' || true
Context:
- Fixing consumer loops (C2, C3) is not enough if the broader feed → waste → detritus → DOC/mineralization pathway still has silent sinks or double-counted steps.
- This bead reconciles the full short-loop bookkeeping around feeding and decomposition.

Deliverable:
- Trace how feed inputs, uneaten food, feces, fine detritus, DOC, mineralization, and nitrification connect.
- Fix ownership boundaries so each transfer is represented once and in the right place.
- Update comments/docs around the detritus and DOC model.
- Produce a documented mass-flow map that future developers can reference.

Key pathway to trace:
  Feed (player action) → uneaten fraction → DOC/DON leaching → fine detritus
                        → consumed by shrimp/microfauna → assimilation + feces + excretion + respiration
  Fine detritus → decomposer mineralization → DOC → DIC/TAN (via decomposition)
  Fine detritus → dissolution → DOC/DON
  Decomposition → O2 demand + DIC + TAN

Likely touch points:
- crates/tank_core/src/systems/nitrogen_cycle.rs (decomposer section)
- crates/tank_core/src/systems/shrimp.rs (feeding section)
- crates/tank_core/src/systems/microfauna.rs
- crates/tank_core/src/engine.rs (Feed action handler)
- crates/tank_core/src/types/process.rs (leaching rates)

Acceptance:
- a feed pulse can be followed conceptually through the major pools
- hidden sinks/double-counts are removed or documented if still intentionally abstracted
- later carbonate and oxygen work can rely on this bookkeeping
EOF

read -r -d '' C4_COMMENT <<'EOF' || true
Future-self notes:
- This is where simplifications should become explicit. It is acceptable to lump some dissolved organics or waste classes as long as the path is coherent.
- Audit with the instrumentation from A2 turned on: feed the tank, run the budget tracker, and see if total N/C stays constant.
- Treat the result as a mass-flow map that later developers can reason about quickly.
- Pay special attention to the DOC pathway: feed leaching (process.rs:28 feed_leach_rate_per_hour) and detritus dissolution (process.rs:30 fine_detritus_dissolution_rate_per_hour) both produce DOC. Decomposers then consume DOC. Make sure these three processes are consistent.
EOF

create_bead C4 'Audit feeding, detritus, DOC, and mineralization bookkeeping end to end' task 1 "$C" scientific-core,phase-1,mass-balance,detritus,nitrogen 240 "${C4_DESC}" "${C4_COMMENT}"

# ---------------------------------------------------------------------------
# C5 — Split plant trimming into export vs leave-cuttings
# ---------------------------------------------------------------------------

read -r -d '' C5_DESC <<'EOF' || true
Context:
- Current trimming behavior (engine.rs:189-197) turns all removed plant biomass into fine detritus, which only matches the specific case where clippings are left in the tank.
- Real aquarium maintenance often exports most trimmed biomass.

Deliverable:
- Replace the current single trimming action with explicit semantics for exported biomass versus in-tank cuttings.
- engine.rs currently has: PlayerAction::TrimPlants { fraction }. This should become two variants or a parameter:
  - TrimPlantsAndRemove { fraction } → biomass exits the system (export)
  - TrimPlantsAndLeaveCuttings { fraction } → biomass enters fine_detritus (current behavior)
- Update snapshots/events/UI wording so the player understands which maintenance action they chose.
- Ensure exported biomass is reflected correctly in conservation/budget logic (explicit export, not unexplained loss).

Likely touch points:
- crates/tank_core/src/engine.rs (lines 189-197)
- crates/tank_core/src/types/actions.rs (PlayerAction enum)
- crates/tank_core/src/types/events.rs (event descriptions)
- crates/tank_tui (trim action UI)

Acceptance:
- the action model distinguishes export from in-tank decay
- event log / UI text reflects the distinction
- budget instrumentation treats export as intentional removal rather than unexplained loss
EOF

read -r -d '' C5_COMMENT <<'EOF' || true
Rationale:
- This bead is small but high-leverage because plant maintenance is common and strongly affects nutrient export.
- In real planted tanks, trimming-and-removing is a primary nutrient export mechanism. A heavily planted tank with regular trim-and-remove exports significant N and P, which reduces the need for water changes. The simulator should let the player experience and learn from this.
- Keep backward-compat/migration implications in mind if old saves reference the previous single trim action. The A2c migration scaffolding should handle this.

Design consideration:
- Two separate action variants is cleaner than a boolean flag, because the intent is more readable in event logs and action queues.
- Default should probably be TrimPlantsAndRemove (more common in practice) to avoid accidentally leaving clippings in the tank.
EOF

create_bead C5 'Split plant trimming into export vs leave-cuttings actions' task 1 "$C" scientific-core,phase-1,actions,plants,husbandry 120 "${C5_DESC}" "${C5_COMMENT}"

# ---------------------------------------------------------------------------
# C6 — Conservation diagnostics and regression tests
# ---------------------------------------------------------------------------

read -r -d '' C6_DESC <<'EOF' || true
Context:
- Once the routing changes (C2-C5) land, the project needs repeatable proof that matter is no longer silently disappearing in normal closed-loop cases.
- The same test harness should also distinguish explicit exports such as water changes and trim-and-remove.

Deliverable:
- Regression tests for shrimp grazing, microfauna consumption, feeding, decomposition, and trimming semantics.
- Diagnostics that report conserved-vs-exported deltas clearly enough to debug failures.
- Tolerances appropriate for deterministic floating-point simulation rather than exact arithmetic fantasy.

Test scenarios to cover:
  1. Closed-system grazing: shrimp eat periphyton, verify total N conserved (TAN increases, periphyton decreases, feces appear).
  2. Closed-system feeding: add feed, verify total N/C conserved through the whole decomposition chain.
  3. Trim-and-remove: verify biomass exported, total N decreases by exactly the exported amount.
  4. Trim-and-leave: verify biomass → detritus transfer, total N conserved.
  5. Water change: verify dilution + replacement chemistry is mass-balanced.

Likely touch points:
- crates/tank_core/src/invariants.rs
- test modules across tank_core and scenario crates
- snapshot/debug helpers from A2

Acceptance:
- closed-loop grazing/feeding cases stay within explicit tolerances
- export actions are tracked as exports, not as unexplained mass loss
- the tests are easy to extend when later shrimp or habitat work lands
EOF

read -r -d '' C6_COMMENT <<'EOF' || true
Future-self notes:
- This bead is the contract that keeps future feature work honest. Any change that breaks conservation should break these tests.
- Prefer a few crisp invariant tests plus readable debug output over a giant opaque golden-file.
- The real success condition is fast diagnosis when a future change breaks conservation.
- Use the budget helpers from A2a-b to make assertions concise: assert!(budget.net_n_delta().abs() < 1e-6, "N budget violated: {}", budget.explain());
EOF

create_bead C6 'Add conservation diagnostics and regression tests for grazing and maintenance loops' task 1 "$C" scientific-core,phase-1,testing,mass-balance,diagnostics 180 "${C6_DESC}" "${C6_COMMENT}"

# ===========================================================================
# PHASE 2 — CARBONATE CHEMISTRY, CO2 EXCHANGE, AND pH REALISM (D1–D6)
# ===========================================================================

read -r -d '' D1_DESC <<'EOF' || true
Context:
- The current pH shortcut (chemistry.rs:18) is too compressed for the simulator's long-term goals, but the project still needs a pragmatic rather than maximal carbonate model.
- Before implementation, the team should agree on which carbonate variables are explicit state, which are derived, and what numerical strategy is acceptable.

Deliverable:
- A design note describing the chosen carbonate state contract. Recommended minimal state:
  - Explicit: total DIC (already exists), alkalinity (already exists), temperature (already exists)
  - Derived: CO2(aq), HCO3-, CO3--, pH (from equilibrium solver)
  - Or alternative: explicit CO2(aq) + alkalinity, derive the rest
- The intended solver approach, update order, and stability/precision expectations.
- Clear notes on what is intentionally excluded in the first pass.

Key design choices:
  1. What is state vs derived?
     → Option A: DIC + alkalinity are state; CO2, HCO3, CO3, pH are derived each tick via equilibrium.
     → Option B: CO2(aq) + alkalinity are state; DIC and pH are derived.
     → Recommendation: Option A (DIC + alk as state) because DIC is already a state variable and alkalinity is already tracked.
  2. Solver approach?
     → Iterative: Newton-Raphson on the charge-balance equation.
     → Analytical: closed-form for CO2-only system (no other weak acids).
     → Recommendation: analytical closed-form using DIC, alkalinity, and temperature-adjusted pKa values. Sufficient for freshwater aquarium range.
  3. Temperature dependence of equilibrium constants?
     → Yes, at least for pKa1 (CO2 ↔ HCO3-). pKa2 matters less in the 6-8 pH range.

Likely touch points:
- crates/tank_core/src/systems/chemistry.rs
- crates/tank_core/src/types/water.rs
- crates/tank_core/src/types/source_water.rs

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

Scientific reference:
- Carbonate equilibrium in freshwater:
  CO2(aq) + H2O ↔ H2CO3 ↔ H+ + HCO3-    (pKa1 ≈ 6.35 at 25°C)
  HCO3- ↔ H+ + CO3--                       (pKa2 ≈ 10.33 at 25°C)
- In the pH 6-8 range typical of aquaria, HCO3- dominates. CO3-- is negligible below pH 8.
- Alkalinity ≈ [HCO3-] + 2[CO3--] - [H+] + [OH-]. For aquarium pH ranges, alkalinity ≈ [HCO3-].
- Given DIC and alkalinity, pH can be solved analytically from the charge balance.
- Temperature corrections: pKa1(T) ≈ 6.352 - 0.0238 × (T-25), which is good enough for 15-35°C.
EOF

create_bead D1 'Define carbonate-state contract and solver strategy' spike 1 "$D" scientific-core,phase-2,carbonates,design,chemistry 180 "${D1_DESC}" "${D1_COMMENT}"

# ---------------------------------------------------------------------------
# D2 — Carbonate equilibrium solver (decomposed into 4 subtasks)
# ---------------------------------------------------------------------------
# This is the core chemistry change for Phase 2. Decomposing into subtasks
# separates state structure changes from solver logic from old-code removal
# from serialization concerns, each of which involves different risks and
# review considerations.

read -r -d '' D2_DESC <<'EOF' || true
Context:
- After D1 chooses the state contract, the engine needs a real equilibrium update instead of the current 6.3 + log10(alkalinity) - log10(DIC) shortcut.
- This is the core chemistry change that unlocks more believable pH differentiation and gas-exchange effects.

Deliverable:
- Add the explicit carbonate state and implement the chosen pH/equilibrium solver.
- Update water state, snapshots, and any affected serialization paths.
- Keep the implementation numerically stable within the expected aquarium parameter ranges (pH 5.5–8.5, alkalinity 0–10 meq/L, temperature 15–35°C).

Likely touch points:
- crates/tank_core/src/systems/chemistry.rs (replace lines 6-20)
- crates/tank_core/src/types/water.rs (new or renamed fields)
- crates/tank_core/src/types/state.rs
- crates/tank_core/src/save.rs (migration for new fields)

Acceptance:
- pH is computed from the new carbonate representation rather than the old shortcut
- the solver is stable across the shipped source-water profiles and common aquarium ranges
- comments/tests explain the chosen approximation level

Note: this task is decomposed into four subtasks (D2a–D2d) for independent tracking.
EOF

read -r -d '' D2_COMMENT <<'EOF' || true
Future-self notes:
- The goal is a robust aquarium-focused solver, not a general geochemistry package.
- Preserve debuggability: intermediate values and assumptions should be inspectable in tests or dev snapshots.
- Do not bury important constants in magic numbers; keep them named and traceable.
- The existing dissolved_inorganic_carbon_mg_c_total field in WaterState can continue as the DIC state variable. The solver derives CO2(aq), HCO3-, CO3--, and pH from it + alkalinity + temperature.
EOF

create_bead D2 'Implement explicit carbonate equilibrium and pH solver' task 0 "$D" scientific-core,phase-2,carbonates,chemistry,pH 300 "${D2_DESC}" "${D2_COMMENT}"

# D2 subtask a: add carbonate state to WaterState
read -r -d '' D2a_DESC <<'EOF' || true
Deliverable:
- If the design from D1 requires new state fields (e.g., explicit CO2(aq) or carbonate species), add them to WaterState.
- If DIC + alkalinity remain the only state variables (recommended), add derived-value fields or a CarbonateDerived struct for CO2(aq), HCO3-, CO3--, and pH.
- Update source water profiles (source_water.rs) to include any new carbonate-relevant inputs needed by the solver.
- Initialize the new fields appropriately in all constructors and scenario builders.

Acceptance:
- WaterState carries whatever the solver needs as input
- derived carbonate values have a clear home (struct or computed on demand)
- no compilation errors from missing field initializations
EOF

read -r -d '' D2a_COMMENT <<'EOF' || true
Design recommendation:
- Keep DIC (dissolved_inorganic_carbon_mg_c_total) and alkalinity (alkalinity_meq_total) as the two independent state variables. All other carbonate species are derived.
- Add a method like water.carbonate_equilibrium(volume_l, temp_c) -> CarbonateResult { co2_aq_mg_c_per_l, hco3_mg_c_per_l, co3_mg_c_per_l, ph } that can be called whenever derived values are needed.
- Don't store derived values as state — this avoids stale-value bugs.
EOF

create_bead D2a 'Add carbonate state variables or derived-value struct to WaterState' task 0 "$D2" scientific-core,phase-2,carbonates,state 90 "${D2a_DESC}" "${D2a_COMMENT}"

# D2 subtask b: implement the pH solver
read -r -d '' D2b_DESC <<'EOF' || true
Deliverable:
- Implement the carbonate equilibrium solver per the D1 design.
- The solver takes DIC (mg C/L), alkalinity (meq/L), and temperature (°C) and returns pH and species distribution.
- Include temperature-dependent pKa values.
- Validate against known reference points:
  - Pure water in equilibrium with atmospheric CO2 (~410 ppm): pH ≈ 5.6, [CO2(aq)] ≈ 0.6 mg/L
  - Typical hard tap water (KH 8, DIC 48 mg C/L): pH ≈ 7.5-8.0
  - Soft acidic water (KH 2, DIC 12 mg C/L): pH ≈ 6.5-7.0

Acceptance:
- solver returns correct pH for at least 3 reference cases
- solver is stable (no NaN, no oscillation) across pH 5.5-8.5
- temperature dependence is implemented and produces directionally correct results (higher T → slightly lower pH for same DIC/alk)
EOF

read -r -d '' D2b_COMMENT <<'EOF' || true
Solver approach (analytical, recommended):
Given:
  CT = total DIC (mol/L) = DIC_mg_c / (12.011 * volume_L)  [converting mg C total to mol/L]
  Alk = alkalinity (eq/L) = alk_meq / (1000 * volume_L)     [converting meq total to eq/L]
  Ka1 = 10^(-pKa1), Ka2 = 10^(-pKa2), Kw = 10^(-14)

From charge balance: Alk = [HCO3-] + 2[CO3--] + [OH-] - [H+]
And carbonate species: [HCO3-] = CT * Ka1*[H+] / ([H+]^2 + Ka1*[H+] + Ka1*Ka2)
                       [CO3--] = CT * Ka1*Ka2 / ([H+]^2 + Ka1*[H+] + Ka1*Ka2)

Substituting into charge balance gives a polynomial in [H+]. For the aquarium pH range (6-8), [CO3--] and [OH-] are small, so Alk ≈ [HCO3-] is a good first approximation:
  [H+] ≈ Ka1 * (CT - Alk) / Alk   (when Alk < CT)
  pH ≈ pKa1 + log10(Alk / (CT - Alk))

Then refine with 1-2 Newton-Raphson iterations for precision.

This is fast (no iterative loop in the common case), stable, and accurate enough for aquarium simulation.
EOF

create_bead D2b 'Implement carbonate equilibrium pH solver with temperature-dependent constants' task 0 "$D2" scientific-core,phase-2,carbonates,chemistry,solver 120 "${D2b_DESC}" "${D2b_COMMENT}"

# D2 subtask c: replace old pH shortcut
read -r -d '' D2c_DESC <<'EOF' || true
Deliverable:
- Replace the call to compute_ph_from_totals() (chemistry.rs:6-20) with the new carbonate equilibrium solver.
- Update step_hourly_chemistry() to use the new solver for pH computation each tick.
- Remove the old compute_ph_from_totals function (or keep it temporarily as a comparison reference if useful for validation).
- Verify that all code paths that read state.water.ph now get the equilibrium-derived value.

Acceptance:
- the old shortcut formula is no longer used for gameplay pH
- pH values for shipped source waters are now meaningfully differentiated (not all ≈6.3)
- existing chemistry tests are updated to use new expected ranges
EOF

read -r -d '' D2c_COMMENT <<'EOF' || true
Expected behavior change:
- The shipped source waters should now produce distinctly different pH values:
  - hard_shrimp (KH 8): pH ≈ 7.5-8.0
  - moderate (KH 4): pH ≈ 7.0-7.5
  - soft_acidic (KH 2-3): pH ≈ 6.5-7.0
  - ro_like (KH 0.1): pH ≈ 5.5-6.5
- This is a significant behavior change that will affect downstream systems (shrimp stress, algae growth, nitrification rates). Test thoroughly.
- Consider keeping the old formula as a #[cfg(test)] comparison function for validation during the transition.
EOF

create_bead D2c 'Replace old pH shortcut with new carbonate equilibrium solver' task 0 "$D2" scientific-core,phase-2,chemistry,migration 45 "${D2c_DESC}" "${D2c_COMMENT}"

# D2 subtask d: update serialization for new carbonate state
read -r -d '' D2d_DESC <<'EOF' || true
Deliverable:
- If new fields were added to WaterState (D2a), register a save migration using the A2c scaffolding.
- Update snapshot.rs to expose derived carbonate values (CO2(aq), perhaps HCO3-) for display.
- Add the new values to the API response and TUI display where relevant.

Acceptance:
- save files from the previous version load successfully with the migration applied
- snapshot includes at least pH and CO2(aq) from the new solver
- no serialization panics on old or new save data
EOF

read -r -d '' D2d_COMMENT <<'EOF' || true
Migration notes:
- If only DIC and alkalinity remain as state (recommended), no migration is needed for the state variables themselves — they already exist.
- The change is in how pH is computed, not what is stored. This is the best-case scenario for backward compatibility.
- If the design requires new state fields (e.g., explicit CO2(aq)), those need #[serde(default)] for old saves.
- The snapshot should show CO2(aq) in mg/L because planted tank hobbyists commonly measure this via KH/pH drop-checker tables.
EOF

create_bead D2d 'Update serialization and snapshot for carbonate state changes' task 1 "$D2" scientific-core,phase-2,save,snapshot,api 45 "${D2d_DESC}" "${D2d_COMMENT}"

# ---------------------------------------------------------------------------
# D3 — CO2 gas exchange
# ---------------------------------------------------------------------------

read -r -d '' D3_DESC <<'EOF' || true
Context:
- Aeration currently affects dissolved oxygen (dissolved_oxygen.rs:27-33) but not carbon dioxide stripping or pH.
- dissolved_oxygen.rs already has a gas exchange model: delta_do = k_la × (do_sat - do_current) × volume.
- A planted tank simulator needs surface exchange that moves both gases, even if the model remains lumped rather than spatially resolved.

Deliverable:
- Extend gas exchange so CO2 responds to aeration/surface exchange and interacts with the carbonate state.
- Use existing geometry and hardware inputs (K_LA, top_exchange_factor, aeration_intensity) where possible.
- Keep the first pass consistent with the chosen carbonate update order.

Parallel to existing O2 exchange:
  O2: delta_o2 = k_la × (do_sat_mg_l - do_mg_l) × volume_l
  CO2: delta_co2 = k_la_co2 × (co2_sat_mg_l - co2_current_mg_l) × volume_l
  where co2_sat depends on atmospheric pCO2 (~410 ppm) and Henry's law.

Key difference from O2:
- CO2 is much more soluble than O2 (Henry's law constant is ~30× higher).
- K_LA for CO2 is related to K_LA for O2 by the ratio of diffusion coefficients: K_LA(CO2) ≈ K_LA(O2) × (D_CO2/D_O2)^0.5 ≈ K_LA(O2) × 0.91.
- CO2 exchange affects DIC, which affects pH through the carbonate equilibrium.

Likely touch points:
- crates/tank_core/src/systems/chemistry.rs (or new co2_exchange module)
- crates/tank_core/src/systems/dissolved_oxygen.rs (share geometry/KLA terms)
- crates/tank_core/src/types/hardware.rs
- crates/tank_core/src/types/geometry.rs

Acceptance:
- high aeration can strip CO2 and influence pH in the expected direction (pH rises as CO2 decreases)
- gas exchange logic for O2 and CO2 is conceptually aligned (shared K_LA basis)
- the model remains deterministic and testable
EOF

read -r -d '' D3_COMMENT <<'EOF' || true
Rationale:
- This bead is what makes aeration scientifically richer than "more oxygen = good." In real planted tanks, heavy aeration strips CO2, which is good for fish but bad for plants that need dissolved CO2 for photosynthesis.
- It also gives the simulation a path toward later CO2 hardware (injection) without requiring that feature immediately.
- Reuse existing geometry terms like exposed area (top_exchange_factor()) where they already exist; do not create parallel geometry semantics.

Scientific notes:
- Atmospheric CO2 ≈ 410 ppm → CO2 equilibrium in water at 25°C ≈ 0.6 mg/L CO2 ≈ 0.16 mg C/L.
- In a non-aerated tank with active biology, CO2 can accumulate well above equilibrium (5-30 mg/L), driving pH down.
- Strong aeration strips CO2 toward equilibrium, raising pH. This is the primary mechanism by which aeration affects pH in freshwater.
- Henry's law: [CO2(aq)] = K_H × pCO2. K_H ≈ 3.4 × 10^-2 mol/(L·atm) at 25°C.
EOF

create_bead D3 'Integrate CO2 gas exchange with aeration, surface exchange, and ambient coupling' task 0 "$D" scientific-core,phase-2,co2,gas-exchange,hardware 240 "${D3_DESC}" "${D3_COMMENT}"

# ---------------------------------------------------------------------------
# D4 — Photosynthesis/respiration ↔ DIC/CO2 coupling
# ---------------------------------------------------------------------------

read -r -d '' D4_DESC <<'EOF' || true
Context:
- Current defaults leave important DIC-related rates at or near zero (process.rs:135-136: respiration_dic_rate_mg_c_per_g_per_hour = 0.0, photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0).
- This means day/night chemistry behavior cannot emerge meaningfully.
- Planted tanks should show at least a simplified mechanistic relationship among photosynthesis, respiration, CO2 availability, and pH drift.

Deliverable:
- Wire plant/algae photosynthesis and biological respiration into the new DIC / CO2 state.
- Revisit defaults so the shipped scenarios actually exercise the new pathway.
- Make the day/night chemistry story visible in tests and/or snapshots.

Expected behavior:
  During light: plants consume CO2 (↓DIC) → pH rises
  During dark: respiration produces CO2 (↑DIC) → pH drops
  Net effect: diurnal pH swing of 0.2-1.0 pH units in heavily planted tanks

Likely touch points:
- crates/tank_core/src/systems/plant_growth.rs (add DIC consumption)
- crates/tank_core/src/systems/algae_growth.rs (add DIC consumption)
- crates/tank_core/src/systems/chemistry.rs (respiration DIC production)
- crates/tank_core/src/types/process.rs (set non-zero defaults)

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
- The dissolved_oxygen.rs system already has a photosynthesis O2 production rate (plant_photosynthesis_o2_mg_per_g_per_hour). The DIC rate should be stoichiometrically consistent: for every mg O2 produced by photosynthesis, ~0.375 mg C is consumed from DIC (from CH2O stoichiometry: CO2 + H2O → CH2O + O2, so 12g C consumed per 32g O2 produced).
EOF

create_bead D4 'Connect photosynthesis and respiration to DIC / CO2 and day–night pH behavior' task 1 "$D" scientific-core,phase-2,plants,carbonates,day-night 240 "${D4_DESC}" "${D4_COMMENT}"

# ---------------------------------------------------------------------------
# D5 — Source-water profile upgrade
# ---------------------------------------------------------------------------

read -r -d '' D5_DESC <<'EOF' || true
Context:
- The shipped source-water presets currently differ in GH/KH/TDS-like dimensions, but not enough in pH behavior to justify their names.
- The new carbonate model needs source-water data that actually drives differentiated equilibrium outcomes.

Deliverable:
- Extend source-water presets so they include whatever carbonate-relevant inputs the chosen model requires (e.g., equilibrium pH or pCO2 target).
- Revisit the shipped presets (hard_shrimp, moderate, soft_acidic, ro_like) so their behavior is meaningfully differentiated.
- Update any docs/comments explaining what each profile is trying to emulate.

Expected differentiation after upgrade:
  hard_shrimp: KH 8, high DIC → pH 7.5-8.0, well-buffered
  moderate: KH 4, moderate DIC → pH 7.0-7.5
  soft_acidic: KH 2-3, low DIC → pH 6.5-7.0, weakly buffered
  ro_like: KH 0.1, minimal DIC → pH 5.5-6.5, very poorly buffered

Likely touch points:
- crates/tank_core/src/types/source_water.rs
- crates/tank_data/data/source_water/*.toml
- crates/tank_data/src/presets.rs

Acceptance:
- source-water presets no longer collapse into nearly identical pH behavior under the new model
- preset semantics are documented in plain language
- tests can load each preset successfully through the updated data path
EOF

read -r -d '' D5_COMMENT <<'EOF' || true
Rationale:
- Source water is one of the most intuitive scientific levers for players, so the presets should teach something real.
- This bead also sets up later shrimp mineral/toxicity work (F3, F5) by making water profiles richer and clearer.
- Avoid overfitting to one real-world tap-water case; target distinct, interpretable envelopes instead.

Real-world reference points:
- Hard shrimp water (Sulawesi-type or high-KH tap): KH 6-10°, GH 6-10°, pH 7.5-8.2
- Moderate community water (typical treated tap): KH 3-6°, GH 4-8°, pH 7.0-7.5
- Soft acidic (blackwater, some Asian tap): KH 0-2°, GH 2-4°, pH 5.5-6.8
- RO/DI remineralized: KH 0-1°, GH 3-5° (added minerals), pH 6.0-7.0
EOF

create_bead D5 'Upgrade source-water profiles to carry carbonate-relevant inputs and differentiated defaults' task 1 "$D" scientific-core,phase-2,source-water,data,carbonates 180 "${D5_DESC}" "${D5_COMMENT}"

# ---------------------------------------------------------------------------
# D6 — Carbonate regression tests
# ---------------------------------------------------------------------------

read -r -d '' D6_DESC <<'EOF' || true
Context:
- Carbonate work is numerically delicate enough that it needs strong regression coverage as soon as it lands.
- The tests should protect the intended qualitative behavior without freezing every intermediate value forever.

Deliverable:
- Scenario tests/probes for:
  1. Source-water pH differentiation: hard_shrimp vs ro_like should produce pH values at least 1.0 units apart.
  2. Aeration-driven CO2 stripping: high aeration should raise pH compared to no aeration.
  3. Day/night pH drift: light-on pH should be higher than light-off pH in a planted tank.
  4. Water change pH effect: changing from RO to hard water should shift pH upward.
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
- Carbonate chemistry bugs can look like "just a weird pH drift." Make the tests narrative enough that failures point to a plausible cause.
- Keep tolerance choices visible and justified. Example: "pH should be in [7.3, 7.8] for hard_shrimp because KH 8 + equilibrium DIC → analytical pH ≈ 7.5."
- Revisit these tests after habitat and shrimp phases if new coupled dynamics widen the realistic envelope.
EOF

create_bead D6 'Add carbonate regression tests and scenario probes' task 1 "$D" scientific-core,phase-2,testing,carbonates,scenarios 180 "${D6_DESC}" "${D6_COMMENT}"

# ===========================================================================
# PHASE 3 — HABITATIZED ECOLOGY, SUBSTRATE REDOX, GEOMETRY-AWARE SCALING (E1–E7)
# ===========================================================================

# ---------------------------------------------------------------------------
# E1 — Habitat registry (decomposed into 3 subtasks)
# ---------------------------------------------------------------------------
# The habitat registry is a new architectural concept that doesn't exist in
# the current codebase. Decomposing separates the type system design from
# the area computation logic from the per-habitat modifiers, each of which
# requires different expertise and review focus.

read -r -d '' E1_DESC <<'EOF' || true
Context:
- The current simulation tracks a tank, substrate, and filter, but not a general habitat model for surfaces and zones.
- To make biofilms, periphyton, redox, and geometry matter for the right reasons, the engine needs a first-class habitat registry.

Deliverable:
- A habitat model covering at least: filter media, plant surfaces, glass/hardscape, substrate surface, and deeper substrate.
- Explicit colonizable area and relevant modifiers such as flow and oxygen exposure.
- Enough abstraction that later systems can attach their biomass pools or rates to habitats without reworking the state layout again.

Target habitats (minimal set):
  FilterMedia   — colonizable area from filter media_surface_cm2 + flow_lph
  GlassHardscape — 2×(L×H) + 2×(W×H) interior wall area, O2 at water-column level
  PlantSurfaces  — dynamic, proportional to plant biomass × leaf_area_per_g_cm2
  SubstrateSurface — L×W footprint area, oxic boundary layer
  SubstrateDeep   — L×W × substrate_depth, suboxic/anoxic zone

Likely touch points:
- crates/tank_core/src/types/state.rs (new HabitatRegistry or similar)
- crates/tank_core/src/types/geometry.rs (area computations)
- crates/tank_core/src/types/substrate.rs (zone differentiation)
- supporting system modules

Acceptance:
- habitats are explicit engine concepts rather than inferred ad hoc
- each habitat has a clear purpose and minimal required metadata
- later biofilm/light/redox tasks can build on this without redesigning the core state again

Note: this task is decomposed into three subtasks (E1a–E1c) for independent tracking.
EOF

read -r -d '' E1_COMMENT <<'EOF' || true
Rationale:
- "Where things live" is the key missing abstraction for the next scientific step.
- Keep habitats ecological rather than graphical. The goal is not spatial rendering; it is mechanistic differentiation.
- Start with a small fixed set of habitats and only generalize further if later work truly needs it.

Design principle:
- A habitat is a surface or zone where biofilm can colonize, decomposition can occur, or redox conditions can differ from the bulk water column.
- Each habitat knows: its colonizable area (cm²), its flow exposure (normalized 0-1), its oxygen exposure (normalized 0-1), and optionally its light exposure.
- Biomass pools (nitrifiers, decomposers, periphyton) will later be indexed by habitat. For now, the registry just provides the geometry and modifiers.
EOF

create_bead E1 'Introduce habitat registry and colonizable-area model' task 1 "$E" scientific-core,phase-3,habitats,geometry,architecture 240 "${E1_DESC}" "${E1_COMMENT}"

# E1 subtask a: habitat type and registry data structure
read -r -d '' E1a_DESC <<'EOF' || true
Deliverable:
- Define a HabitatKind enum: FilterMedia, GlassHardscape, PlantSurfaces, SubstrateSurface, SubstrateDeep.
- Define a Habitat struct: { kind: HabitatKind, colonizable_area_cm2: f64, flow_exposure: f64, o2_exposure: f64, light_exposure: f64 }.
- Add a Vec<Habitat> or HabitatRegistry to TankState.
- Ensure habitats are serializable and included in save/load.

Acceptance:
- the type system compiles and habitats are part of TankState
- each habitat kind has a documented purpose
- later subtasks can compute area and modifiers without changing the types
EOF

read -r -d '' E1a_COMMENT <<'EOF' || true
Design notes:
- Use an enum + struct rather than trait objects — the habitat set is fixed and small.
- Consider making the registry a fixed-size array indexed by HabitatKind ordinal for cache-friendly access.
- flow_exposure and o2_exposure are normalized multipliers (0.0-1.0) that later systems use to scale rates. For example, filter media has high flow exposure, glass has low flow exposure.
- light_exposure indicates how much light reaches the habitat. Filter media inside a canister has 0.0; glass at water surface might be 0.5; substrate surface depends on depth.
EOF

create_bead E1a 'Define habitat type enum and registry data structure' task 1 "$E1" scientific-core,phase-3,habitats,types 90 "${E1a_DESC}" "${E1a_COMMENT}"

# E1 subtask b: compute colonizable areas from geometry
read -r -d '' E1b_DESC <<'EOF' || true
Deliverable:
- Implement area computation for each habitat kind from existing TankGeometry, HardwareState, and PlantGuildState:
  - FilterMedia: from filter media_surface_cm2 (hardware) or estimated from flow rate
  - GlassHardscape: from tank dimensions (2×L×H + 2×W×H interior area)
  - PlantSurfaces: proportional to total plant biomass × leaf_area_per_g parameter
  - SubstrateSurface: from tank footprint (L × W)
  - SubstrateDeep: from footprint × substrate depth
- These should update each tick (plant surfaces change as biomass changes) or at least each day.

Acceptance:
- habitat areas are computed from existing geometry, not hardcoded
- changing tank geometry or plant biomass changes habitat areas
- a test verifies area computation for at least one scenario
EOF

read -r -d '' E1b_COMMENT <<'EOF' || true
Computation notes:
- Tank dimensions in geometry.rs are in cm: length_cm, width_cm, height_cm, fill_height_cm, substrate_depth_cm.
- Glass area: interior walls below fill height. 2 × (length × fill_height) + 2 × (width × fill_height). Subtract substrate area where substrate meets glass.
- Plant surfaces: needs a new parameter leaf_area_per_g_cm2 (typical aquatic plants: 100-300 cm²/g depending on species).
- Filter media area: the current HardwareState has filter flow (flow_lph) but not media surface area. May need to add media_surface_cm2 or estimate from flow (e.g., 100 cm²/lph as a rough starting point).
EOF

create_bead E1b 'Compute colonizable areas from geometry, hardware, and plant state' task 1 "$E1" scientific-core,phase-3,habitats,geometry 90 "${E1b_DESC}" "${E1b_COMMENT}"

# E1 subtask c: flow and oxygen exposure modifiers
read -r -d '' E1c_DESC <<'EOF' || true
Deliverable:
- Set default flow_exposure and o2_exposure values for each habitat kind:
  - FilterMedia: flow=1.0 (high flow), o2=0.8-1.0 (well-oxygenated from flow)
  - GlassHardscape: flow=0.1-0.3 (ambient circulation), o2=1.0 (water column)
  - PlantSurfaces: flow=0.2-0.5, o2=varies with photosynthesis
  - SubstrateSurface: flow=0.1, o2=0.5-0.8 (diffusion boundary layer)
  - SubstrateDeep: flow=0.0, o2=0.0-0.3 (suboxic)
- Make at least flow_exposure responsive to filter hardware (flow rate) and aeration.
- Set light_exposure based on habitat depth and light system.

Acceptance:
- each habitat has non-trivial, mechanistically motivated modifier values
- filter flow changes affect filter media flow exposure
- modifiers are named and documented for future tuning
EOF

read -r -d '' E1c_COMMENT <<'EOF' || true
Design notes:
- These modifiers will later scale biofilm growth rates, nitrifier capacity, and decomposer activity per habitat. Getting them approximately right now saves retuning later.
- o2_exposure for SubstrateDeep should be very low (0.0-0.1) because this is where anaerobic/suboxic conditions develop, enabling denitrification (E4).
- Consider making o2_exposure for SubstrateDeep responsive to rooted plant presence (roots oxygenate the rhizosphere).
EOF

create_bead E1c 'Set flow and oxygen exposure modifiers per habitat' task 1 "$E1" scientific-core,phase-3,habitats,modifiers 60 "${E1c_DESC}" "${E1c_COMMENT}"

# ---------------------------------------------------------------------------
# E2 — Biofilter scaling
# ---------------------------------------------------------------------------

read -r -d '' E2_DESC <<'EOF' || true
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

Acceptance:
- biofilter capacity meaningfully responds to habitat/media/flow inputs
- no single hidden constant stands in for every filter setup
- scenario tests can show geometry/hardware differences affecting cycling in a plausible way
EOF

read -r -d '' E2_COMMENT <<'EOF' || true
Future-self notes:
- This is one of the most visible "small tank vs bigger tank" realism wins after concentration normalization.
- Prefer a simple capacity model with clear knobs over a black-box emergent one that is hard to tune.
- Document which parts represent colonizable area versus process-rate multipliers.
- The key insight: a nano tank with a small sponge filter and a medium tank with a large canister filter should have very different nitrifier capacity ceilings, and this should emerge from habitat area × flow exposure, not from a tank-size lookup table.
EOF

create_bead E2 'Scale biofilter carrying capacity with habitat, media, flow, and oxygen' task 1 "$E" scientific-core,phase-3,biofilter,habitats,nitrogen 240 "${E2_DESC}" "${E2_COMMENT}"

# ---------------------------------------------------------------------------
# E3 — Periphyton/decomposer split by habitat
# ---------------------------------------------------------------------------

read -r -d '' E3_DESC <<'EOF' || true
Context:
- Periphyton and decomposer biomass are currently too lumped to capture differences among glass film, plant-surface biofilm, filter-associated biofilm, and substrate-associated biofilm.
- A habitat split allows grazing, light exposure, and decomposition to become more ecologically legible.

Deliverable:
- Introduce habitat-specific pools or weights for periphyton and decomposer activity.
- Ensure the first pass is still computationally simple and does not explode the state space unnecessarily.
- Tie the new pools to habitats created in E1.

Likely touch points:
- crates/tank_core/src/systems/algae_growth.rs (periphyton section)
- crates/tank_core/src/systems/microfauna.rs (grazing targets)
- crates/tank_core/src/systems/shrimp.rs (grazing targets)
- habitat-bearing state types

Acceptance:
- at least the major habitats can differ in periphyton/decomposer abundance or productivity
- shrimp and microfauna can graze in a habitat-aware way later
- comments explain what is still lumped versus newly differentiated
EOF

read -r -d '' E3_COMMENT <<'EOF' || true
Rationale:
- This bead gives "microscope life" and biofilm maturity a better substrate without requiring species-level simulation.
- Keep the dimensionality low. Habitat differentiation should reveal mechanisms, not bury the sim under bookkeeping.
- The main question is explanatory power: can a player understand why one surface is fouling or maturing differently from another?

Ecological notes:
- Glass biofilm: primarily periphyton (algae) because glass gets direct light. This is what hobbyists scrape off during maintenance.
- Filter media biofilm: primarily nitrifiers and heterotrophic bacteria. Very little periphyton because it's dark inside the filter.
- Plant surface biofilm: mixed — some periphyton, some epiphytic bacteria. Healthy plants often resist heavy biofilm.
- Substrate surface: moderate periphyton + decomposers processing settled detritus.
- Substrate deep: no periphyton (no light), anaerobic/suboxic bacteria, denitrifiers.
EOF

create_bead E3 'Split periphyton and decomposer pools by habitat' task 1 "$E" scientific-core,phase-3,habitats,periphyton,decomposition 240 "${E3_DESC}" "${E3_COMMENT}"

# ---------------------------------------------------------------------------
# E4 — Substrate redox (decomposed into 3 subtasks)
# ---------------------------------------------------------------------------
# This task introduces a new scientific concept (redox zones) to the
# substrate model. Decomposing separates the state structure from the
# denitrification process from the root-zone interactions, each of which
# has different scientific foundations and testing requirements.

read -r -d '' E4_DESC <<'EOF' || true
Context:
- The current substrate model (substrate.rs) does not yet distinguish oxygenated and low-oxygen zones in a way that can support denitrification or root-zone effects.
- A planted tank simulator benefits from at least a simple redox gradient story for deeper substrate and roots.

Deliverable:
- Add a two-zone (oxic/suboxic) substrate redox structure.
- Introduce simple denitrification behavior in suboxic zones.
- Add hooks for root-zone oxygenation or redox effects.
- Keep the model bounded and explainable; this is not a full sediment biogeochemistry simulator.

Likely touch points:
- crates/tank_core/src/types/substrate.rs
- crates/tank_core/src/types/state.rs
- crates/tank_core/src/systems/nitrogen_cycle.rs (denitrification)
- crates/tank_core/src/systems/plant_growth.rs (root-zone hooks)

Acceptance:
- substrate can represent at least oxic vs suboxic behavior
- nitrate removal in low-oxygen zones is possible for explicit mechanistic reasons
- rooted-plant interactions with substrate redox have clear extension hooks even if simplified at first

Note: this task is decomposed into three subtasks (E4a–E4c) for independent tracking.
EOF

read -r -d '' E4_COMMENT <<'EOF' || true
Future-self notes:
- Resist the urge to model every sediment process. The value here is introducing a believable redox story, not complete sediment ecology.
- Make the transition criteria and zone meanings inspectable in debug output.
- This bead should improve explanation of "mature planted substrate behaves differently" without turning maintenance into a chemistry PhD exam.

Scientific background:
- In aquarium substrates, oxygen penetration depth is typically 1-5 mm from the surface. Below that, conditions become suboxic (low O2) to anoxic (no O2).
- In suboxic zones, facultative anaerobes use NO3- as terminal electron acceptor: denitrification (NO3- → N2 gas).
- Denitrification removes N from the system permanently (as N2 gas), which is actually beneficial in heavily stocked tanks.
- Plant roots oxygenate the rhizosphere (root-adjacent substrate), creating micro-oxic zones within otherwise suboxic substrate. This is the "root oxidation" effect visible as lighter-colored substrate around roots.
EOF

create_bead E4 'Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks' task 1 "$E" scientific-core,phase-3,substrate,denitrification,plants,redox 300 "${E4_DESC}" "${E4_COMMENT}"

# E4 subtask a: oxic/suboxic zone state
read -r -d '' E4a_DESC <<'EOF' || true
Deliverable:
- Add oxic and suboxic zone concepts to SubstrateLayerState or SubstrateKind.
- At minimum: o2_penetration_depth_cm (dynamic, depends on biological O2 demand and diffusion), and a suboxic zone below that depth.
- The oxic zone should correspond to the SubstrateSurface habitat from E1; the suboxic zone to SubstrateDeep.

Acceptance:
- substrate state distinguishes oxic and suboxic zones
- zone boundaries are computed from mechanistic inputs (not hardcoded depth)
- the zones integrate with the habitat registry from E1
EOF

read -r -d '' E4a_COMMENT <<'EOF' || true
Simplified model for O2 penetration depth:
- O2 penetration ≈ sqrt(2 × D_O2 × [O2_surface] / R_consumption)
  where D_O2 is diffusion coefficient in sediment (~1e-5 cm²/s), [O2_surface] is O2 concentration at substrate surface, and R_consumption is volumetric O2 consumption rate by bacteria.
- For a first pass: estimate penetration depth as 2-5 mm, modified by biological load and substrate porosity.
- The key behavior: heavily loaded tanks → shallower O2 penetration → larger suboxic zone → more denitrification.
EOF

create_bead E4a 'Add oxic/suboxic zone state to substrate model' task 1 "$E4" scientific-core,phase-3,substrate,redox,state 90 "${E4a_DESC}" "${E4a_COMMENT}"

# E4 subtask b: denitrification in suboxic zone
read -r -d '' E4b_DESC <<'EOF' || true
Deliverable:
- Implement simplified denitrification in the suboxic substrate zone:
  NO3- → N2(gas), removing N from the system permanently.
- Rate should depend on: suboxic zone volume, NO3 availability (via diffusion from water column), and DOC availability (carbon source for denitrifiers).
- Add denitrifier biomass or activity index to the SubstrateDeep habitat, or use a simplified rate without explicit biomass.

Acceptance:
- suboxic substrate removes some nitrate over time
- denitrification rate responds to NO3 concentration and DOC availability
- N removed by denitrification is tracked as explicit export (not unexplained loss) in budget diagnostics
EOF

read -r -d '' E4b_COMMENT <<'EOF' || true
Scientific notes:
- Denitrification stoichiometry (simplified): 5 CH2O + 4 NO3- → 2 N2↑ + 4 HCO3- + CO2 + 3 H2O
- This means denitrification: (a) removes NO3, (b) consumes DOC, (c) produces alkalinity (HCO3-), (d) produces DIC.
- The alkalinity production is significant: denitrification partially compensates for alkalinity consumed by nitrification. This is a real and important self-regulating mechanism in mature planted tanks.
- Rate-limiting factors in order of importance: NO3 availability at the suboxic boundary, DOC diffusion, suboxic zone volume.
- Keep the first pass simple: rate = denitrification_vmax × [NO3] / ([NO3] + K_no3) × [DOC] / ([DOC] + K_doc) × suboxic_volume_fraction.
EOF

create_bead E4b 'Implement simplified denitrification in suboxic substrate zones' task 1 "$E4" scientific-core,phase-3,substrate,denitrification,nitrogen 120 "${E4b_DESC}" "${E4b_COMMENT}"

# E4 subtask c: root-zone oxygenation hooks
read -r -d '' E4c_DESC <<'EOF' || true
Deliverable:
- Add hooks for rooted plants to influence substrate redox conditions.
- At minimum: rooted plant biomass increases O2 penetration depth or creates micro-oxic zones.
- This should make "mature planted substrate" behave differently from "unplanted substrate" for mechanistic reasons.

Acceptance:
- rooted plant presence visibly influences substrate O2 penetration
- the effect is proportional to root biomass, not a binary on/off
- tests or scenario probes demonstrate the difference
EOF

read -r -d '' E4c_COMMENT <<'EOF' || true
Scientific background:
- Radial oxygen loss (ROL) from plant roots is well-documented in aquatic macrophytes. Roots release O2 into the surrounding sediment, creating an oxic rhizosphere within otherwise suboxic substrate.
- This has dual effects: (a) prevents toxic reduced compounds (H2S, Fe2+) from accumulating near roots, (b) enables nitrification in the rhizosphere (a micro-nitrogen-cycle around each root).
- For the simulator: parameterize as o2_release_per_g_root_per_hour and increase O2 penetration depth proportionally to root biomass.
- Keep it simple: root_oxidation_effect = plant.root_biomass_g × o2_release_rate. This widens the oxic zone, reducing the effective suboxic volume.
EOF

create_bead E4c 'Add root-zone oxygenation hooks for rooted plants' task 1 "$E4" scientific-core,phase-3,substrate,plants,roots 90 "${E4c_DESC}" "${E4c_COMMENT}"

# ---------------------------------------------------------------------------
# E5 — Light attenuation
# ---------------------------------------------------------------------------

read -r -d '' E5_DESC <<'EOF' || true
Context:
- Tank geometry already includes depth (fill_height_cm in geometry.rs), but the light model does not yet turn depth and turbidity into different average exposure conditions.
- Depth-sensitive light is a key mechanism for distinguishing shallow and deep planted tanks under the same nominal lamp setting.

Deliverable:
- Extend light handling so depth and turbidity influence light available to plants/algae/habitats.
- Integrate the result with the habitat model where useful.
- Keep the first pass simple enough that users and developers can reason about it.

Likely model:
  I(z) = I_0 × exp(-k × z)  (Beer-Lambert law)
  where k = extinction coefficient (depends on turbidity, dissolved color, algae density)
  z = depth below surface

Likely touch points:
- crates/tank_core/src/systems/light.rs
- crates/tank_core/src/systems/plant_growth.rs
- crates/tank_core/src/systems/algae_growth.rs
- crates/tank_core/src/types/geometry.rs

Acceptance:
- deeper or more turbid tanks receive meaningfully different effective light
- plant/algae systems can consume that difference without ad hoc hacks
- tests or scenario probes capture at least one shallow-vs-deep contrast
EOF

read -r -d '' E5_COMMENT <<'EOF' || true
Rationale:
- This bead turns geometry into a scientifically relevant control on growth instead of only a thermal or volumetric one.
- Avoid overcomplication: one clear attenuation story (Beer-Lambert) is better than several overlapping fudge factors.
- Make sure the result remains explainable in the TUI later.

Scientific notes:
- Typical extinction coefficient for clear freshwater: k ≈ 0.1-0.3 per cm. For turbid or tannin-stained water: k ≈ 0.5-2.0 per cm.
- In a 20cm deep nano tank with k=0.2: light at bottom = I_0 × exp(-0.2×20) ≈ I_0 × 0.018 (1.8% of surface light). This is significant.
- In a 50cm deep tank: light at bottom ≈ I_0 × 0.00005 — essentially dark. This is why tall tanks need strong lighting.
- Average light across the water column (more relevant for planktonic algae): I_avg = I_0 × (1 - exp(-k×H)) / (k×H).
EOF

create_bead E5 'Add depth/turbidity light attenuation and habitat-specific light exposure' task 1 "$E" scientific-core,phase-3,light,geometry,plants,algae 180 "${E5_DESC}" "${E5_COMMENT}"

# ---------------------------------------------------------------------------
# E6 — Geometry-aware equipment/biomass scaling
# ---------------------------------------------------------------------------

read -r -d '' E6_DESC <<'EOF' || true
Context:
- Geometry scaling (tank_scenarios/src/lib.rs) already changes some properties, but startup hardware and biomass defaults still remain partly fixed.
- Filter flow stays at ~200 lph if enabled (lib.rs:581-589), plant biomass stays fixed at ~5g per guild (lib.rs:687-699), heater power is not geometry-aware.

Deliverable:
- Revisit scenario/preset logic so filter flow, heater power, initial plant biomass, and stocking defaults scale coherently with geometry or are explicitly overridden.
- Document which defaults scale automatically versus which remain scenario-authored choices.
- Recheck ambient-temperature behavior after the changes.

Scaling rules to implement:
  filter_flow_lph: scale with volume (e.g., 10× volume per hour turnover)
  heater_power_w: scale with volume and exposed surface area (W/L guideline)
  initial_plant_biomass_g: scale with substrate footprint (g/cm²)
  initial_shrimp_count: scale with volume (shrimp/L stocking density)

Likely touch points:
- crates/tank_scenarios/src/lib.rs
- crates/tank_core/src/types/hardware.rs
- scenario data files and related builders

Acceptance:
- geometry-scaled scenarios no longer inherit obviously mismatched default hardware/biomass
- scaling rules are written down
- tests can distinguish deliberate overrides from automatic scaling behavior
EOF

read -r -d '' E6_COMMENT <<'EOF' || true
Future-self notes:
- This bead is about making comparisons fair. When a bigger tank behaves differently, it should be because the model says it should, not because startup defaults quietly stayed nano-sized.
- Keep scenario authorship flexible: auto-scaling should help, not remove the ability to create intentionally unusual setups (e.g., overstocked nano, understocked large tank).
- Revisit ambient-temperature edge cases because thermal behavior depends on both geometry (volume, surface area) and hardware (heater power).

Common aquarium sizing guidelines:
- Filter: 5-10× tank volume per hour flow rate
- Heater: 0.5-1.0 W per liter (depends on ambient-to-target temperature delta)
- Plants: 5-15g/1000cm² of substrate footprint for a "moderately planted" setup
- Shrimp: 2-5 shrimp per liter for Neocaridina (varies with filtration and plant density)
EOF

create_bead E6 'Scale equipment, plant mass, and stocking defaults with tank geometry' task 1 "$E" scientific-core,phase-3,geometry,hardware,scenarios 180 "${E6_DESC}" "${E6_COMMENT}"

# ---------------------------------------------------------------------------
# E7 — Habitat/geometry scenario tests
# ---------------------------------------------------------------------------

read -r -d '' E7_DESC <<'EOF' || true
Context:
- Habitat and geometry changes will touch many systems at once, so they need scenario-level tests that protect the intended stories.
- The tests should confirm that the new abstractions changed behavior for mechanistic reasons, not simply because parameters were retuned.

Deliverable:
- Scenario probes for:
  1. Biofilter scaling: bigger filter media → higher nitrifier capacity → faster cycling
  2. Light-depth: deep tank → less bottom light → different plant/algae behavior vs shallow tank
  3. Habitat-specific fouling: glass periphyton vs filter biofilm developing at different rates
  4. Substrate redox: mature planted substrate shows NO3 removal (denitrification) that unplanted substrate does not
  5. Equipment scaling: scaled-up tank with appropriately scaled equipment should show similar per-liter behavior
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

# ===========================================================================
# PHASE 4 — SHRIMP LIFE HISTORY, TOXICITY, AND REPRODUCTION REALISM (F1–F6)
# ===========================================================================

read -r -d '' F1_DESC <<'EOF' || true
Context:
- The current shrimp model already has useful population and reproduction logic, but the next step needs a clearer internal state structure.
- Before adding stages, minerals, and toxicity modifiers, the project should decide what shrimp-level state is explicit versus derived.

Deliverable:
- A design note for the shrimp state model covering:
  - Reserve/condition: food reserve, body condition index
  - Life stage: juvenile, sub-adult, adult (minimum 3 stages)
  - Molt state: inter-molt interval, readiness, last molt success
  - Reproduction: egg carrying state, clutch size, incubation progress
- Clear boundaries between population-level state and individual-level abstraction.
- Notes on what remains intentionally omitted in the first pass (e.g., sex ratio, individual tracking).

Likely touch points:
- crates/tank_core/src/types/biology.rs (AnimalState redesign)
- crates/tank_core/src/systems/shrimp.rs
- shrimp data files

Acceptance:
- later shrimp implementation beads can proceed without reworking the model contract
- the state model is detailed enough for the planned mechanics but still tractable for tuning
- comments/docs explain why a given level of aggregation was chosen
EOF

read -r -d '' F1_COMMENT <<'EOF' || true
Rationale:
- This bead protects the project from sliding into either over-simplified "population only" logic or an unnecessarily expensive individual-based model.
- Keep the state choices closely tied to the behaviors the sim wants to express: molt success, breeding, juvenile survival, and chemistry-related stress.
- The best design is the one that supports explanation and calibration, not maximum detail for its own sake.

Recommended approach: stage-structured cohort model
- Track counts per stage: juveniles, sub-adults, adults (optionally: berried females).
- Track aggregate metrics per stage: average condition, average reserve.
- Individuals are not tracked — this is a population model with stage structure.
- This is the same level of abstraction used in many fisheries models (Leslie matrix / stage-structured matrix model) and is well-suited to hourly discrete-time simulation.
EOF

create_bead F1 'Define shrimp state model for reserve, condition, stage, molt, and reproduction' spike 1 "$F" scientific-core,phase-4,shrimp,design,biology 180 "${F1_DESC}" "${F1_COMMENT}"

# ---------------------------------------------------------------------------
# F2 — Stage-structured shrimp population (decomposed into 4 subtasks)
# ---------------------------------------------------------------------------
# This is the largest behavioral change to the shrimp model. Decomposing
# separates state structure, transition logic, system integration, and
# snapshot concerns into independently reviewable units.

read -r -d '' F2_DESC <<'EOF' || true
Context:
- A single undifferentiated shrimp population cannot express juvenile vulnerability, adult reproduction, or growth-stage tradeoffs very well.
- The next version should differentiate shrimp enough that lifecycle dynamics are visible and tunable.

Deliverable:
- Add stage or size structure to the shrimp population model consistent with F1.
- Ensure feeding, mortality, reproduction, and stress pathways can see the new structure.
- Keep save/load and snapshot implications under control.

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs
- crates/tank_core/src/types/biology.rs (AnimalState)
- crates/tank_core/src/types/snapshot.rs
- save/state code as needed

Acceptance:
- at least juvenile and reproductive-adult dynamics are distinguishable
- state transitions are deterministic and testable
- scenario output can show lifecycle structure in a user-comprehensible way

Note: this task is decomposed into four subtasks (F2a–F2d) for independent tracking.
EOF

read -r -d '' F2_COMMENT <<'EOF' || true
Future-self notes:
- Use the lightest structure that unlocks the target behaviors. Three stages (juvenile, sub-adult, adult) are likely enough.
- Make sure this integrates with the mass-routing loop from C2 rather than bypassing it. Each stage should have feeding, excretion, and respiration rates.
- Pay attention to snapshot/UI burden; more state is only useful if it remains interpretable.
EOF

create_bead F2 'Implement stage- or size-structured shrimp population dynamics' task 1 "$F" scientific-core,phase-4,shrimp,population,life-history 300 "${F2_DESC}" "${F2_COMMENT}"

# F2 subtask a: stage state structure
read -r -d '' F2a_DESC <<'EOF' || true
Deliverable:
- Replace the current single population count in AnimalState with a stage-structured model.
- At minimum: juvenile_count, subadult_count, adult_count, berried_count.
- Each stage should carry: count, average_condition (0-1), average_reserve_g.
- Total population = sum of all stage counts.
- Maintain backward compatibility: the existing shrimp_count field should become a derived sum.

Acceptance:
- AnimalState has per-stage population data
- existing code that reads total shrimp count still works (via sum or accessor)
- serialization includes new fields with appropriate defaults for old saves
EOF

read -r -d '' F2a_COMMENT <<'EOF' || true
Design notes:
- Consider a ShrimpStage struct { count: u32, avg_condition: f64, avg_reserve_g: f64 } and a stages: [ShrimpStage; 4] array on AnimalState (indexed by StageKind enum).
- The current shrimp_count becomes stages.iter().map(|s| s.count).sum().
- For old saves without stage data: use #[serde(default)] to initialize all shrimp as adults with the saved count and default condition.
EOF

create_bead F2a 'Add stage-structured state to shrimp population model' task 1 "$F2" scientific-core,phase-4,shrimp,state,biology 90 "${F2a_DESC}" "${F2a_COMMENT}"

# F2 subtask b: stage transitions
read -r -d '' F2b_DESC <<'EOF' || true
Deliverable:
- Implement stage transitions: juvenile → sub-adult → adult, gated by age/size thresholds and condition.
- Use the existing shrimp_juvenile_maturation_days parameter (process.rs:115) as the baseline transition time.
- Add a sub-adult → adult transition with a similar parameterized duration.
- Transition from berried → adult + new juveniles (hatching) based on incubation period.

Acceptance:
- newly hatched shrimp enter as juveniles
- juveniles mature to sub-adults after maturation_days
- sub-adults become adults after a further growth period
- berried adults hatch after incubation, producing a clutch of juveniles
EOF

read -r -d '' F2b_COMMENT <<'EOF' || true
Biological parameters for Neocaridina davidi:
- Juvenile period: ~30-45 days from hatch to sexual maturity
- Sub-adult to adult: ~15-30 days
- Egg incubation: 21-30 days depending on temperature (faster at higher temp)
- Average clutch size: 20-30 eggs for a mature female
- These can be parameterized in the shrimp species data file.
EOF

create_bead F2b 'Implement stage transitions (juvenile → sub-adult → adult → berried)' task 1 "$F2" scientific-core,phase-4,shrimp,transitions,life-history 90 "${F2b_DESC}" "${F2b_COMMENT}"

# F2 subtask c: integrate stages with feeding, mortality, and reproduction
read -r -d '' F2c_DESC <<'EOF' || true
Deliverable:
- Connect the shrimp feeding loop (C2) to stage-specific rates: juveniles eat less, adults eat more.
- Apply stage-specific mortality: juveniles have higher baseline mortality than adults.
- Gate reproduction on adult stage (only adults can become berried).
- Stress and toxicity (future F3-F5 work) should also be stage-aware once implemented.

Acceptance:
- juvenile mortality is higher than adult mortality
- feeding rate scales with stage/size
- only adults reproduce
- tests show that a population with juveniles has different dynamics than all-adults
EOF

read -r -d '' F2c_COMMENT <<'EOF' || true
Stage-specific parameters:
- Juvenile feeding rate: ~30-50% of adult rate (smaller body size)
- Juvenile baseline mortality: ~2-5× adult baseline (higher vulnerability)
- Sub-adult feeding rate: ~70-80% of adult rate
- These multiplicative factors should be in the species data file, not hardcoded.
EOF

create_bead F2c 'Integrate stage structure with feeding, mortality, and reproduction systems' task 1 "$F2" scientific-core,phase-4,shrimp,integration 60 "${F2c_DESC}" "${F2c_COMMENT}"

# F2 subtask d: update snapshot and save for structured population
read -r -d '' F2d_DESC <<'EOF' || true
Deliverable:
- Update TankSnapshot to expose per-stage counts (juvenile_count, adult_count, berried_count).
- Update API response to include stage breakdown.
- Update TUI display to show stage information where relevant.
- Register save migration for the new AnimalState structure (via A2c scaffolding).

Acceptance:
- snapshot shows stage breakdown, not just total count
- old saves load with reasonable defaults for the new stage structure
- TUI remains readable (don't overwhelm with too many numbers)
EOF

read -r -d '' F2d_COMMENT <<'EOF' || true
Display considerations:
- The TUI biology view should show something like: "Shrimp: 12 total (3 juvenile, 2 sub-adult, 5 adult, 2 berried)"
- The full stage detail is for the detailed view; the summary view can keep showing total count + overall condition.
- API response should include both the total and the breakdown for flexibility.
EOF

create_bead F2d 'Update snapshot, save, and TUI for stage-structured shrimp population' task 1 "$F2" scientific-core,phase-4,shrimp,snapshot,save,tui 60 "${F2d_DESC}" "${F2d_COMMENT}"

# ---------------------------------------------------------------------------
# F3 — Mineral budget and molt mechanics
# ---------------------------------------------------------------------------

read -r -d '' F3_DESC <<'EOF' || true
Context:
- Shrimp husbandry is heavily shaped by minerals and molt success, but the current model does not yet express that linkage explicitly.
- With richer water chemistry (D5 source water profiles) and clearer state structure (F2) in place, molt outcomes can now depend on something more meaningful than generic stress.

Deliverable:
- Introduce a simplified mineral-budget or availability check that affects molt success/failure.
- Tie the mechanic to the water-profile story established earlier without demanding perfect ionic physiology.
- Surface the outcome in a way that players can understand and act on.

Key mechanism:
  molt_success_probability = base_rate × mineral_modifier(GH, Ca, Mg) × condition_modifier(reserve)
  - Low GH (< 4°) → reduced molt success (insufficient Ca/Mg for exoskeleton)
  - Very low GH (< 2°) → high molt failure risk
  - High condition/reserve → normal molt success
  - Low condition (recent starvation or stress) → reduced molt success

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs
- crates/tank_core/src/types/water.rs (GH/Ca/Mg access)
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

Scientific notes:
- Neocaridina davidi molt approximately every 4-6 weeks as adults, more frequently as juveniles.
- Exoskeleton formation requires Ca2+ and Mg2+ ions. In very soft water (GH < 2°), shrimp often have visibly thin shells and higher molt mortality.
- The "white ring of death" (failed molt visible as a white band) is a commonly observed phenomenon in soft-water shrimp tanks.
- Recommended GH for Neocaridina: 6-8° (≈ 100-140 mg/L as CaCO3).
EOF

create_bead F3 'Add mineral budget and molt success/failure mechanics' task 1 "$F" scientific-core,phase-4,shrimp,molting,minerals,water-chemistry 240 "${F3_DESC}" "${F3_COMMENT}"

# ---------------------------------------------------------------------------
# F4 — Reproduction realism
# ---------------------------------------------------------------------------

read -r -d '' F4_DESC <<'EOF' || true
Context:
- Current shrimp reproduction is already directionally temperature-sensitive, but the next step is to tie breeding success/failure to a richer set of explicit mechanisms.
- This should enable better simulation of "tank looks okay but breeding still stalls" outcomes.

Deliverable:
- Extend reproduction logic so success depends on temperature envelope, environmental stability (rate of change), shrimp condition/reserve, density, and relevant water-chemistry stress proxies.
- Keep the first pass aggregated and interpretable.
- Update any reproduction-related species parameters or docs.

Factors to include:
  1. Temperature: existing curve (22-28°C optimal), sharply reduced above 30°C
  2. Stability: rapid temperature or chemistry changes suppress reproduction
  3. Condition: well-fed adults with high reserve breed more successfully
  4. Density: overcrowding reduces per-capita breeding rate
  5. Water chemistry: very low GH, high TAN, or high NO2 suppress breeding

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs
- crates/tank_core/src/types/process.rs
- crates/tank_data/data/shrimp/neocaridina_davidi.toml
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
- Density and stability should matter because mature tanks often succeed for systemic reasons, not just because they are "warm enough."

Scientific notes:
- Neocaridina davidi breeding behavior: females carry eggs for 21-30 days. Unfavorable conditions can cause egg dropping (loss of clutch).
- Egg dropping triggers: sudden temperature change (>2°C in 24h), very high or very low pH, molting stress, physical disturbance.
- Density-dependent effects: at high densities (>10/L), per-capita reproduction decreases due to competition and stress.
- A "stable, mature tank" breeds well because: temperature is consistent, chemistry is stable, food supply is reliable, and bioload is managed. The simulator should reward this through the interplay of all the above factors.
EOF

create_bead F4 'Model reproduction success as a function of temperature, stability, condition, density, and osmotic stress' task 1 "$F" scientific-core,phase-4,shrimp,reproduction,stress 240 "${F4_DESC}" "${F4_COMMENT}"

# ---------------------------------------------------------------------------
# F5 — Chloride-nitrite toxicity
# ---------------------------------------------------------------------------

read -r -d '' F5_DESC <<'EOF' || true
Context:
- The simulator already tracks chemistry that can support better stress logic (chloride_mg_total in water.rs:26, nitrite_mg_n_total in water.rs:13), but nitrite hazard is still missing an important modifier: chloride.
- Adding this makes stress outcomes less naive and better tied to water composition.

Deliverable:
- Introduce a chloride-aware nitrite hazard modifier suitable for the project's fidelity level.
- Integrate the result into shrimp stress/mortality logic without overcomplicating the full toxicology model.
- Document assumptions and parameter confidence clearly.

Mechanism:
  effective_nitrite_hazard = [NO2-] / (1 + chloride_protection_factor × [Cl-] / [NO2-])
  where chloride_protection_factor is a species-specific parameter.
  High chloride relative to nitrite → reduced effective hazard.

Likely touch points:
- crates/tank_core/src/systems/shrimp.rs
- crates/tank_core/src/types/water.rs
- source-water and process parameter files
- validation/provenance notes

Acceptance:
- equal nitrite does not imply equal hazard when chloride differs
- the toxicology pathway is transparent enough to explain in docs or debug output
- tests/scenarios cover at least one chloride-mitigation contrast
EOF

read -r -d '' F5_COMMENT <<'EOF' || true
Rationale:
- This bead is a good example of "small addition, big explanatory value."
- Keep the formulation modest and honest about uncertainty; the win is mechanistic directionality, not toxicology exhaustiveness.
- Treat this as a bridge between chemistry richness and animal outcomes.

Scientific background:
- Nitrite is toxic to freshwater crustaceans because NO2- competes with Cl- for uptake at the gills.
- In high-chloride water, nitrite uptake is inhibited, reducing toxicity. This is why salt (NaCl) is a common emergency treatment for nitrite poisoning in aquaria.
- The Cl-:NO2- ratio is the key modifier. A ratio > 10:1 is generally considered protective.
- Source: chloride reduces nitrite accumulation and associated stress in freshwater organisms (ResearchGate: Chloride uptake in freshwater teleosts).
EOF

create_bead F5 'Model chloride protection against nitrite hazard and integrate toxic stress accounting' task 2 "$F" scientific-core,phase-4,shrimp,toxicology,nitrite,chloride 180 "${F5_DESC}" "${F5_COMMENT}"

# ---------------------------------------------------------------------------
# F6 — Shrimp scenario tests
# ---------------------------------------------------------------------------

read -r -d '' F6_DESC <<'EOF' || true
Context:
- Once the shrimp model is richer, it needs scenario coverage that demonstrates both success and failure modes.
- These tests should support calibration and gameplay confidence, not just prevent panics.

Deliverable:
- Scenario probes for:
  1. Successful breeding: mature planted tank, stable conditions, adequate minerals → population grows
  2. Thermal suppression: temperature above 30°C → breeding stops, egg dropping
  3. Chemistry-induced stress: low GH + high NO2 → molt failures, increased mortality
  4. Chloride protection: same NO2 but high chloride → less mortality than low chloride
  5. Crash mode: overcrowding + overfeeding + no water changes → population collapse
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
- Prefer a handful of sharp scenarios over a single giant "everything at once" regression.
- These will become inputs to the calibration phase (G3, G4), so keep them readable.
EOF

create_bead F6 'Add shrimp-focused scenario tests for breeding, heat stress, molt failure, and crash modes' task 1 "$F" scientific-core,phase-4,testing,shrimp,scenarios 180 "${F6_DESC}" "${F6_COMMENT}"

# ===========================================================================
# PHASE 5 — PROVENANCE, CALIBRATION, VALIDATION, AND RELEASE NARRATIVE (G1–G6)
# ===========================================================================

read -r -d '' G1_DESC <<'EOF' || true
Context:
- The project goal is explicitly scientific, which means important parameters should eventually carry source and confidence metadata.
- The engine/data pipeline needs a minimal schema for provenance before parameter files can be enriched consistently.

Deliverable:
- Add a provenance-friendly parameter representation or adjacent metadata path that supports source, confidence, notes, and valid-range information.
- Ensure data loading remains ergonomic enough for everyday development.
- Decide how provenance is stored for code-defined defaults versus data-file parameters.

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

Acceptance:
- the project has a concrete place to store provenance/confidence metadata
- loaders and data structures remain maintainable
- the schema is flexible enough for both chemistry and biology parameters
EOF

read -r -d '' G1_COMMENT <<'EOF' || true
Rationale:
- Provenance is how the project grows from "plausible sim" into "scientifically grounded sim."
- Keep the first schema simple and useful; it does not need to solve every citation-management problem on day one.
- Think about developer ergonomics so provenance becomes a habit, not a burden.

Early-start note:
- G1 depends only on A1 (semantics inventory). It can start as soon as Phase 0 completes, well before the later science phases finish.
- This is intentional: having the provenance schema in place early means later phases (B6 retuning, D5 source water, F3 minerals) can attach provenance metadata as they go, rather than backfilling it all in Phase 5.
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

Acceptance:
- high-impact parameters have provenance entries
- confidence/uncertainty is explicit, not implied
- future maintainers can tell which parameters are strong anchors versus temporary approximations
EOF

read -r -d '' G2_COMMENT <<'EOF' || true
Future-self notes:
- Start with the load-bearing parameters, not every constant in the codebase.
- Be honest about uncertainty. A transparent provisional value is better than an unlabeled "scientific-looking" number.
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
- Clear separation between "validated directionally" and "still heuristic."

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
- spec.md
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

Acceptance:
- the upgraded simulator has a documented post-refactor tuning state
- the release narrative ties changes back to the project's overarching scientific goals
- obvious follow-on work is captured cleanly instead of being lost in memory
EOF

read -r -d '' G6_COMMENT <<'EOF' || true
Future-self notes:
- This is the "make it legible" bead, not just a tuning sweep.
- Use the calibration workflow (G4) and validation scenarios (G3) rather than intuition-only tweaks.
- Capture what was intentionally postponed so future roadmaps can pick it up without re-opening old debates.
EOF

create_bead G6 'Run end-to-end rebalance pass and capture release narrative for the scientific-core upgrade' task 2 "$G" scientific-core,phase-5,retuning,release,calibration 180 "${G6_DESC}" "${G6_COMMENT}"

# ===========================================================================
# DEPENDENCY GRAPH
# ===========================================================================
# The dependency graph encodes the execution ordering described in the phase
# structure. Dependencies are primarily between tasks (not epics), with
# subtask ordering handled internally.
#
# Key principles:
# - Epic-level deps are NOT added because they are fully captured by
#   task-level deps and would add noise to cycle detection.
# - Subtask internal ordering is added (fan-out/fan-in patterns).
# - Cross-task deps point to parent tasks, not subtasks, keeping the
#   inter-task graph at constant granularity.
# - Transitive dependencies (e.g., F4→C2 which is already covered by
#   F4→F2→F1→C2) are omitted to reduce graph weight. The only exception
#   is where removing a transitive dep would make the intent unclear.

# --- Phase 0 internal ordering ---
add_dep "$A2" "$A1"           # instrumentation needs semantics inventory
add_dep "$A3" "$A1"           # envelopes need inventory to know what to capture
add_dep "$A3" "$A2"           # envelopes use instrumentation to capture baselines

# --- A2 subtask ordering ---
add_dep "$A2b" "$A2a"         # debug hooks wrap budget tracking
add_dep "$A2c" "$A2a"         # save scaffolding can use budget types; parallel with A2b is fine

# --- Phase 1A internal ordering ---
add_dep "$B1" "$A1"           # unit taxonomy needs semantics inventory
add_dep "$B2" "$B1"           # helpers implement the unit policy
add_dep "$B2" "$A2"           # helpers may use instrumentation types
add_dep "$B3" "$B2"           # nitrogen kinetics use concentration helpers
add_dep "$B3" "$A3"           # normalization validates against baselines
add_dep "$B4" "$B2"           # plant normalization uses helpers
add_dep "$B5" "$B2"           # algae normalization uses helpers
add_dep "$B6" "$B3"           # retuning happens after nitrogen is normalized
add_dep "$B6" "$B4"           # retuning happens after plants are normalized
add_dep "$B6" "$B5"           # retuning happens after algae is normalized
add_dep "$B7" "$B1"           # field names need unit policy
add_dep "$B7" "$B2"           # display conversions use helpers
add_dep "$B8" "$B7"           # TDS/conductivity builds on the renamed fields

# --- B3 subtask ordering (fan-out from B3a, fan-in to B3e) ---
add_dep "$B3b" "$B3a"         # NOB follows AOB pattern
add_dep "$B3c" "$B3a"         # comammox follows AOB pattern
add_dep "$B3d" "$B3a"         # decomposer follows AOB pattern
add_dep "$B3e" "$B3b"         # regression test needs all guilds normalized
add_dep "$B3e" "$B3c"
add_dep "$B3e" "$B3d"

# --- Phase 1B internal ordering ---
add_dep "$C1" "$A1"           # matter routing needs semantics inventory
add_dep "$C2" "$C1"           # shrimp loop follows routing conventions
add_dep "$C2" "$B2"           # shrimp loop uses concentration helpers
add_dep "$C3" "$C1"           # microfauna follows routing conventions
add_dep "$C3" "$B2"           # microfauna uses concentration helpers
add_dep "$C4" "$C2"           # audit after shrimp loop is in place
add_dep "$C4" "$C3"           # audit after microfauna loop is in place
add_dep "$C4" "$B3"           # audit needs nitrogen kinetics correct
add_dep "$C5" "$C1"           # trim semantics follow routing conventions
add_dep "$C5" "$A2"           # trim needs migration scaffolding for action rename
add_dep "$C6" "$C4"           # conservation tests after bookkeeping audit
add_dep "$C6" "$C5"           # conservation tests after trim split
add_dep "$C6" "$A2"           # conservation tests use instrumentation

# --- C2 subtask ordering ---
add_dep "$C2b" "$C2a"         # excretion builds on assimilation
add_dep "$C2c" "$C2a"         # feces builds on assimilation (parallel with C2b)
add_dep "$C2d" "$C2b"         # respiration + test after excretion and feces
add_dep "$C2d" "$C2c"

# --- Phase 2 internal ordering ---
add_dep "$D1" "$B1"           # carbonate design needs unit policy
add_dep "$D1" "$A1"           # carbonate design needs semantics inventory
add_dep "$D2" "$D1"           # solver implements the design
add_dep "$D2" "$B2"           # solver uses concentration helpers
add_dep "$D2" "$A2"           # solver needs migration scaffolding
add_dep "$D3" "$D2"           # gas exchange needs carbonate state
add_dep "$D4" "$D2"           # DIC coupling needs carbonate solver
add_dep "$D4" "$B4"           # DIC coupling needs plant normalization
add_dep "$D4" "$B5"           # DIC coupling needs algae normalization
add_dep "$D4" "$C4"           # DIC coupling needs clean bookkeeping
add_dep "$D5" "$D2"           # source water needs carbonate model
add_dep "$D5" "$B6"           # source water needs retuned parameters
add_dep "$D6" "$D3"           # tests need gas exchange
add_dep "$D6" "$D4"           # tests need DIC coupling
add_dep "$D6" "$D5"           # tests need differentiated source water
add_dep "$D6" "$A3"           # tests compare against baselines

# --- D2 subtask ordering ---
add_dep "$D2b" "$D2a"         # solver needs state structure
add_dep "$D2c" "$D2b"         # replacement needs working solver
add_dep "$D2d" "$D2c"         # serialization after code change

# --- Phase 3 internal ordering ---
add_dep "$E1" "$B1"           # habitat types need unit policy
add_dep "$E1" "$A1"           # habitats need semantics inventory
add_dep "$E1" "$A2"           # habitats need save scaffolding
add_dep "$E2" "$E1"           # biofilter scaling needs habitats
add_dep "$E2" "$B3"           # biofilter scaling needs normalized N kinetics
add_dep "$E3" "$E1"           # periphyton split needs habitats
add_dep "$E3" "$C4"           # periphyton split needs clean bookkeeping
add_dep "$E3" "$B5"           # periphyton split needs normalized algae
add_dep "$E4" "$E1"           # substrate redox needs habitat framework
add_dep "$E4" "$E3"           # redox needs habitat-split biomass pools
add_dep "$E4" "$D3"           # redox follows gas exchange patterns
add_dep "$E4" "$B4"           # redox hooks need normalized plant model
add_dep "$E5" "$E1"           # light attenuation needs habitats
add_dep "$E5" "$B4"           # light affects plants (need normalized model)
add_dep "$E5" "$B5"           # light affects algae (need normalized model)
add_dep "$E6" "$E1"           # scaling needs habitat framework
add_dep "$E6" "$A3"           # scaling validates against baselines
add_dep "$E6" "$B6"           # scaling needs retuned parameters
add_dep "$E7" "$E2"           # tests need biofilter scaling
add_dep "$E7" "$E4"           # tests need substrate redox
add_dep "$E7" "$E5"           # tests need light attenuation
add_dep "$E7" "$E6"           # tests need geometry scaling
add_dep "$E7" "$A3"           # tests compare against baselines

# --- E1 subtask ordering ---
add_dep "$E1b" "$E1a"         # area computation needs types
add_dep "$E1c" "$E1b"         # modifiers build on area computation

# --- E4 subtask ordering ---
add_dep "$E4b" "$E4a"         # denitrification needs zone state
add_dep "$E4c" "$E4a"         # root hooks need zone state (parallel with E4b)

# --- Phase 4 internal ordering ---
add_dep "$F1" "$C2"           # shrimp design needs conservation loop done
add_dep "$F1" "$D5"           # shrimp design needs differentiated water profiles
add_dep "$F1" "$E3"           # shrimp design needs habitat-aware periphyton
add_dep "$F2" "$F1"           # implementation follows design
add_dep "$F3" "$F2"           # molting needs stage structure
add_dep "$F3" "$D5"           # molting needs water profile for minerals
add_dep "$F3" "$B8"           # molting needs TDS/ion tracking
add_dep "$F4" "$F2"           # reproduction needs stage structure
add_dep "$F4" "$F3"           # reproduction interacts with molt success
add_dep "$F5" "$F2"           # toxicity needs stage structure
add_dep "$F5" "$D5"           # toxicity needs water profiles
add_dep "$F5" "$B8"           # toxicity needs ion tracking
add_dep "$F6" "$F3"           # tests need molting mechanics
add_dep "$F6" "$F4"           # tests need reproduction mechanics
add_dep "$F6" "$F5"           # tests need toxicity mechanics
add_dep "$F6" "$A3"           # tests compare against baselines

# --- F2 subtask ordering ---
add_dep "$F2b" "$F2a"         # transitions need state structure
add_dep "$F2c" "$F2b"         # integration needs transitions
add_dep "$F2d" "$F2c"         # snapshot/save after integration

# --- Phase 5 internal ordering ---
add_dep "$G1" "$A1"           # provenance schema needs semantics inventory (EARLY START)
add_dep "$G2" "$G1"           # metadata needs schema
add_dep "$G2" "$B6"           # metadata needs retuned params
add_dep "$G2" "$D5"           # metadata needs carbonate/source water params
add_dep "$G2" "$E2"           # metadata needs biofilter params
add_dep "$G2" "$F4"           # metadata needs shrimp reproduction params
add_dep "$G3" "$A3"           # validation needs baselines
add_dep "$G3" "$B6"           # validation needs retuned scenarios
add_dep "$G3" "$C6"           # validation needs conservation tests
add_dep "$G3" "$D6"           # validation needs carbonate tests
add_dep "$G3" "$E7"           # validation needs habitat tests
add_dep "$G3" "$F6"           # validation needs shrimp tests
add_dep "$G3" "$G2"           # validation needs provenance metadata
add_dep "$G4" "$G3"           # calibration workflow needs validation scenarios
add_dep "$G5" "$B7"           # docs need API field names settled
add_dep "$G5" "$D6"           # docs need carbonate behavior described
add_dep "$G5" "$E7"           # docs need habitat behavior described
add_dep "$G5" "$F6"           # docs need shrimp behavior described
add_dep "$G5" "$G4"           # docs need calibration results
add_dep "$G6" "$G4"           # rebalance uses calibration workflow
add_dep "$G6" "$G5"           # rebalance needs docs updated

# ===========================================================================
# SANITY CHECKS AND FINAL FLUSH
# ===========================================================================

"${BR[@]}" dep cycles
"${BR[@]}" sync --flush-only

echo ''
echo '============================================================='
echo 'Backlog creation complete.'
echo ''
echo 'Summary:'
echo '  1 root epic'
echo '  7 phase epics (A-G)'
echo '  ~33 tasks'
echo '  ~26 subtasks'
echo '  Dependency edges: ~120'
echo ''
echo 'Review with:'
echo '  br epic status        — phase-level progress'
echo '  br ready              — tasks ready to start (all deps satisfied)'
echo '  br list --pretty      — full backlog overview'
echo '  br dep graph          — dependency visualization'
echo '============================================================='
