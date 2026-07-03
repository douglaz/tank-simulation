# Spec: v0.7 "Scientific Integrity" Release

Status: DRAFT rev 2 (revised after codex xhigh review)
Author: scientific-core review, 2026-07-03
Scope owner: `tank_core`
Predecessor: v0.2–v0.6 scientific-core overhaul (complete; backlog drained)

---

## 1. Motivation

The v0.2–v0.6 overhaul gave the simulator broad, mechanistic coverage: concentration-based
Monod kinetics, AOB/NOB/comammox nitrification, denitrification, a closed-form carbonate
equilibrium solver, a five-zone habitat registry, stage-structured shrimp, and parameter
provenance. The breadth work is done. A code-level audit of the shipped model found that
the project's central claim — **"deterministic mass-balance chemistry"** (README.md:47) —
is only partially enforced, is enforced only when opted in, covers only two of the tracked
conserved elements, and is silently violated for a third (phosphorus).

This release makes the existing model *provably conservative and internally consistent*.
It is split into two clearly separated phases:

- **Phase A — Integrity (behavior-neutral).** Accounting and enforcement changes that must
  NOT move any validation envelope. The conservation guard proves this: if Phase A changed
  behavior, the guard or the calibration report would flag it.
- **Phase B — Behavior corrections (envelope-moving, individually justified).** A small set
  of biology/chemistry corrections that necessarily change output and require re-calibration
  and written envelope justification. These are honestly labeled as behavior changes, not
  "just accounting."

The earlier framing of "no new science" was inaccurate and has been dropped: several of the
fixes below (realistic plant C:N, hypoxia response, reproduction aggregation, pH clamp) do
change behavior. The split above makes that explicit and keeps the risky part isolated.

### Guiding principle

Every element the model tracks as a pool must be conserved across every transformation, or
the transformation must be an explicitly accounted source/sink. Approximations are allowed;
silent non-conservation is not.

---

## 2. Findings this release resolves

Verified against the current code on branch `ralph/tank-sim` and re-checked by an
independent xhigh review. File/line references are anchors, not exact contracts.

### F1 — Phosphorus is tracked but never conserved (HIGH, integrity)

Phosphate is a first-class state variable: dissolved `water.phosphate_mg_p_total`
(`types/water.rs:56`) and per-layer substrate `nutrient_store_mg_p_total`
(`types/substrate.rs:80`). It moves through the system on the **uptake side**: feed input
(`systems/nitrogen_cycle.rs:145,149`), plant/algae uptake (`systems/plant_growth.rs:105`,
`systems/algae_growth.rs:112`), substrate-store drawdown weighted by the CEC index
(`systems/plant_growth.rs:342`), refund of unused uptake (`systems/plant_growth.rs:271`),
and water changes (`systems/water_change.rs:56,77`).

But the conserved element ledger — `BudgetTotals`/`BudgetDelta` (`types/budget.rs:67,92`)
and the `Element` enum (`budget_helpers.rs:70`) — covers only **nitrogen, carbon, oxygen**.
Phosphorus is absent from every check. Worse, the **loss side** of the P cycle is not even
represented as phosphorus: when plant/algae biomass turns over, is grazed, is trimmed, or
suspended algae is exported in a water change, the organic mass routes as *grams projected
to N and C only* (see F3/W0). Phosphorus assimilated into biomass has no element-preserving
return path at all. P can be silently created or destroyed and no test would detect it.

Correction vs rev 1: there is no substrate P *release* path in the cited code (only
CEC-weighted drawdown into plants), so "CEC storage/release" in rev 1 was imprecise.

### F2 — Conservation enforcement is opt-in, partial, and clamp-masked (MED-HIGH, integrity)

Correction vs rev 1: a runtime guard **does** exist. `enforce_tracked_tick_budget_guard()`
returns `SimError::BudgetImbalance` on drift (`engine.rs:1158`). But it is inadequate as a
conservation law for three reasons:
1. **Off by default.** It runs only when `budget_ledger` is enabled; default engines set
   `budget_ledger: None` (`engine.rs:215,284`), so production and most tests never run it.
2. **Partial element coverage.** It checks nitrogen and carbon only — not oxygen, not
   phosphorus.
3. **Clamp-masked.** `BudgetTotals::from_state` floors oxygen with `.max(0.0)`
   (`types/budget.rs:109`) and `enforce_invariants` normalizes negative DO before checking
   (`invariants.rs:13`), so a negative-oxygen event is hidden from the total instead of
   surfaced as a violation. Clamps are unaccounted sources/sinks.

### F3 — Organism stoichiometry is tied to the fish-food parameter (HIGH, mixed)

Plant and algae tissue carbon is computed as `nitrogen / feed_n_to_c_ratio`
(`types/budget.rs:536,544`; `feed_n_to_c_ratio = 0.16` at
`crates/tank_data/data/process/default.toml:30`). Two distinct problems:

- **(integrity)** A *feed* parameter determines *plant and algae* carbon content. Organism
  composition must be organism-owned. Decoupling it (keeping current effective values) is
  behavior-neutral.
- **(behavior)** `0.16` gives plants molar C:N ~7, which is algae-like; real submersed
  macrophytes run molar C:N ~15–30. Correcting the *value* changes carbon drawdown and
  detrital return, so it is a behavior change, not accounting.

Shrimp sibling defect **(integrity)**: reserves are funded via `grams ×
LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G` (= 0.20; `types/budget.rs:24`, used at
`systems/shrimp.rs:1363,1385`) and projected to N/C by the generic reserve projection,
while death returns body mass via the per-species `body_nitrogen/carbon_mg_per_g` fields
(`systems/shrimp.rs:1330`). Element-wise conservation across birth→death holds only when
*both* the reserve's organic fraction *and* its implied N:C match the body composition — a
fragile coincidence of the shipped defaults, not a guaranteed identity. Any non-default body
composition preset silently creates or destroys N and/or C each cycle.

### F4 — Hypoxia inverts the biology (MED, behavior)

Under O2-limited respiration, the *un-respired* carbon and nitrogen are added to the
organism's reserve (`systems/shrimp.rs:408`, `systems/microfauna.rs:153`). Mass is
conserved (so this is not an integrity bug), but the sign is biologically backwards: low DO
makes shrimp and microfauna *accumulate more reserve* instead of depressing growth.

### F5 — Reproduction multiplies too many sub-unity factors (MED, behavior)

Condition is a product of seven factors (`systems/shrimp.rs:455`); hatch rate multiplies a
base by seven factors (`:940`); readiness multiplies eight (`:1586`). Independent factors
near 0.8 collapse (0.8^8 ≈ 0.17). Adult condition is counted three times — readiness
(`:1564`), hatch (`:928`), and clutch size (`:951`) — compounding one variable. Behavior /
calibration, not conservation.

### F6 — pH clamp: boundary charge inconsistency + truncated swing (MED, mixed)

`CARBONATE_PH_MAX = 8.5` (`systems/chemistry.rs:111`). At the boundary the solver reprojects
species at the clamped pH and, by its own comment, the cached speciation "no longer imply[s]
the original alkalinity" (`chemistry.rs:431`).
- **(integrity)** The cached carbonate state silently mis-states charge at the boundary.
  Making cached species consistent with alkalinity is an accounting fix.
- **(behavior)** Raising the clamp so a brightly lit tank can reach pH 9+ changes output and
  requires re-calibration.

---

## 3. Scope

### Phase A — Integrity (behavior-neutral; must not move envelopes)

**W0. Element-explicit organic pools (FOUNDATION — precedes W1 and W3).**
Today N/C conservation works only because every organic pool (detritus, feces, reserve,
trim residue) is a scalar gram pool projected to N and C through one shared
`feed_n_to_c_ratio`. That representation cannot conserve N, C, and P separately once
different guilds have different C:N:P. Redesign the organic-pool representation so each
pool carries (or is projected through a stored, source-specific) N, C, and P content, and
every transfer route — plant loss → `fine_detritus_g_total` (`systems/plant_growth.rs:402`),
consumer feces/reserve (`systems/shrimp.rs:366`, `systems/microfauna.rs:110`), trim-leave,
decomposition, water-change export of suspended algae — moves all three elements
consistently. This is the linchpin: W1 and W3 are unsound without it. Landed with current
effective compositions, it is behavior-neutral.

**W1. Phosphorus in the conserved ledger with real loss routes.**
Add phosphorus as a first-class conserved element in `BudgetTotals`, `BudgetDelta`,
`BudgetSnapshot`, the `Element` enum, and the biomass→element projection. Wire **every** P
transfer — uptake, refund, substrate drawdown, feed-derived organic P, and the loss routes
enabled by W0 (turnover, grazing, trim-leave, decomposition, water-change export) — into the
ledger. Resolves F1.

**W2. Enforce N/C/O/P closure by default in dev/CI; account for clamps.**
Extend `enforce_tracked_tick_budget_guard()` to cover oxygen and phosphorus in addition to
N and C, and make it run without requiring an opt-in ledger in `debug_assert` builds and in
the validation harness (release builds stay free of the cost). Convert the O2 (and any
other) clamp from a silent `.max(0.0)` into an explicit, logged budget source/sink so clamp
events are accounted, not hidden. Resolves F2.

**W3a. Organism-owned stoichiometry (values preserved).**
Give plants, algae, detritus, and shrimp their own C:N:P composition constants (with
`param_meta`), replacing the `feed_n_to_c_ratio`-derived carbon. Feed keeps its own ratio
for feed only. Fund shrimp reserves from the same body composition used at death.
**Preserve current effective numeric values** so this item is behavior-neutral and provable
against the guard. Resolves the integrity half of F3.

**W6a. Carbonate boundary charge consistency (clamp unchanged).**
At the pH clamp boundary, make the cached carbonate speciation consistent with the tracked
alkalinity (reproject to preserve alkalinity, or record the residual as an explicit
accounted deviation). Do **not** change the clamp value in Phase A. Resolves the integrity
half of F6. Update `docs/carbonate_state_contract.md`.

### Phase B — Behavior corrections (envelope-moving; each individually justified)

Land only after Phase A is green and the guard is active, so every envelope shift is
attributable to a deliberate correction and re-calibrated.

**W3b. Realistic macrophyte C:N.** Set plant tissue C:N to a literature macrophyte value
(molar ~15–30). Re-run calibration; justify envelope changes. (behavior half of F3)

**W4. Hypoxia response.** Under O2 limitation, suppress assimilation/growth instead of
crediting un-respired C/N to reserve; un-assimilated food routes to feces/detritus (via W0),
never to phantom reserve; total mass still closes. Resolves F4.

**W5. Reproduction aggregation.** Replace the product of sub-unity factors with a defensible
aggregation (Liebig minimum or geometric mean of independent stressors) and count adult
condition once. Re-tune only to keep envelopes sane. Resolves F5.

**W6b. Raise pH clamp.** Raise `CARBONATE_PH_MAX` to admit realistic daytime highs (target
up to ~pH 9.5) on top of the W6a boundary fix; verify solver stability across 15–35 °C.
Resolves the behavior half of F6.

**W7. Documentation refresh.** Update `PROVENANCE_STATUS.md`, `scientific_specs.md`, and
README to reflect already-shipped phosphate/potassium tracking and live DIC rates, and to
record the new conservation guarantees and per-guild stoichiometry.

### Explicitly out of scope (deferred to v0.8+ realism release)

Activity-coefficient (Davies) correction; free-CO2 vs bicarbonate split; humic/organic-acid
buffering; iron/potassium co-limitation; algae functional types; CEC GH/KH buffering and
aquasoil ammonia leaching; molt-death-syndrome specifics and water-change osmotic
shock/acclimation; the algae NH4-uptake 60% cap vs plant full-NH4 inconsistency
(`systems/algae_growth.rs:375`).

---

## 4. Acceptance criteria

Phase A (behavior-neutral):
A1. **Element-explicit routes.** A route-matrix test exercises every organic transfer
    (plant/algae turnover, grazing, feces, trim-leave, decomposition, water-change export)
    and asserts N, C, and P each close independently. (W0, W1)
A2. **Phosphorus structural + dynamic closure.** P has structural budget coverage mirroring
    the existing N/C coverage, and a multi-day run (feed → uptake → drawdown → refund →
    turnover → water change) conserves total P to fp tolerance. (W1)
A3. **Guard covers N/C/O/P and is on in dev/CI.** Deliberately injecting a create/destroy of
    any of the four elements trips the guard/`debug_assert` or a harness failure with the
    ledger no longer opt-in for those builds. (W2)
A4. **Clamp accounting.** A forced negative-DO condition records an explicit clamp
    source/sink budget metric rather than a silent `.max(0.0)`. (W2)
A5. **Stoichiometry is organism-owned and value-preserving.** Changing `feed_n_to_c_ratio`
    no longer changes plant/algae tissue carbon; with values preserved, the full validation
    suite envelopes are unchanged. A birth→death cycle with a **non-default** shrimp body
    composition conserves N **and** C **independently** (asserted separately, not as a sum).
    (W3a)
A6. **Carbonate boundary consistency.** A test at the clamp boundary recomputes alkalinity
    from the cached species and matches the tracked alkalinity within tolerance. (W6a)
A7. **Phase A moves no envelope.** The eight validation scenarios and the calibration report
    are unchanged after Phase A. (all Phase A)

Phase B (behavior; changes justified):
B1. Plant tissue molar C:N sits in the literature macrophyte band; envelope deltas
    documented. (W3b)
B2. In a low-DO scenario, shrimp/microfauna reserve and growth **decrease** vs normoxia, and
    total N/C still close. (W4)
B3. Near-optimal conditions yield reproductive output in a sane range; condition is not
    triple-counted. (W5)
B4. A brightly lit CO2-drawdown scenario reaches pH > 8.5 with the carbonate contract holding
    at the new boundary. (W6b)

Cross-cutting:
C1. `cargo test && cargo clippy -- -D warnings && cargo fmt --check` pass. (all)
C2. **Determinism preserved:** seeded save/load and replay remain bit-identical; no wall
    clock, no unseeded RNG. (all)

---

## 5. Risks and mitigations

- **W0 is invasive.** Redesigning organic-pool representation touches many systems.
  Mitigation: it is the first item; land it behavior-neutral (current compositions) and
  prove no-envelope-change (A7) before anything else.
- **Guard failing "correctly" during W3.** If stoichiometry changes before W0 lands, the
  now-active guard will legitimately fail. Mitigation: strict ordering W0 → W1/W2 → W3a.
- **Re-tuning cascade in Phase B.** F3b/F5 may move envelopes. Mitigation: land each Phase B
  item alone, re-run the calibration report, adjust envelopes only with written
  justification.
- **Debug-assert cost.** Per-tick full-ledger checks could slow long runs. Mitigation: gate
  under `debug_assert`/harness flag; release builds unaffected.
- **Boundary math (W6).** Raising the clamp must not destabilize the quadratic solver.
  Mitigation: verify against `carbonate_state_contract.md` vectors across 15–35 °C.

---

## 6. Deliverables

1. Code changes in `tank_core` implementing W0, W1, W2, W3a, W6a (Phase A) then W3b, W4, W5,
   W6b (Phase B), plus W7 docs.
2. New tests: route-matrix N/C/P closure, P structural + dynamic, N/C/O/P guard-injection,
   clamp accounting, non-default shrimp composition (N and C asserted separately), carbonate
   boundary alkalinity recomputation; updated hypoxia, reproduction, and pH tests.
3. Refreshed docs (W7) and an updated calibration report.
4. A follow-on set of **detailed, buildable per-work-item implementation specs** (one per
   W-item, W0 first) authored and reviewed before implementation begins.

---

## 7. Sequencing

1. **W0** element-explicit organic pools (behavior-neutral) → prove A7 (no envelope change).
2. **W1 + W2** P ledger + N/C/O/P guard on-by-default-in-dev + clamp accounting → A1–A4.
3. **W3a** organism-owned stoichiometry, values preserved → A5, re-confirm A7.
4. **W6a** carbonate boundary consistency → A6.
5. Phase A gate: full suite + calibration report unchanged (A7).
6. **W3b → W4 → W5 → W6b**, one at a time, each re-running calibration (B1–B4).
7. **W7** docs → final suite + calibration report.

Implementation does not begin until the per-item implementation specs (Deliverable 4, W0
first) are written and have passed their own review.
