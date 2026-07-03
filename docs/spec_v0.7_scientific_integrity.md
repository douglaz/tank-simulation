# Spec: v0.7 "Scientific Integrity" Release

Status: DRAFT rev 3 (revised after two codex xhigh reviews)
Author: scientific-core review, 2026-07-03
Scope owner: `tank_core`
Predecessor: v0.2–v0.6 scientific-core overhaul (complete; backlog drained)

---

## 1. Motivation

The v0.2–v0.6 overhaul gave the simulator broad, mechanistic coverage: concentration-based
Monod kinetics, AOB/NOB/comammox nitrification, denitrification, a closed-form carbonate
equilibrium solver, a five-zone habitat registry, stage-structured shrimp, and parameter
provenance. The breadth work is done. A code-level audit of the shipped model found that the
project's central claim — **"deterministic mass-balance chemistry"** (README.md:47) — is
only partially enforced (opt-in, N/C only, clamp-masked) and is silently *violated* for
phosphorus, which is taken up into biomass but never returned as phosphorus.

This release makes the existing model *provably conservative and internally consistent*,
in two clearly separated phases:

- **Phase A — Integrity (behavior-neutral).** Accounting, enforcement, and representation
  changes that must NOT move any validation envelope. Where the current model has a real
  imbalance (phosphorus), Phase A does **not** silently fix it — it makes the imbalance an
  **explicit, logged reconciliation term** so behavior is unchanged and the conservation
  guard passes with the leak visible instead of hidden.
- **Phase B — Behavior corrections (envelope-moving, individually justified).** A small set
  of deliberate biology/chemistry corrections that necessarily change output — including
  actually *closing* the phosphorus leak — each requiring re-calibration and written
  envelope justification.

The "no new science" framing from rev 1 was inaccurate and has been dropped. Phase B changes
behavior on purpose; the split isolates that risk and keeps the guard meaningful.

### Guiding principle

Every element the model tracks as a pool must be conserved across every transformation, or
the transformation must be an **explicitly accounted** source/sink. A discovered imbalance is
first made explicit (Phase A), then corrected as a deliberate, re-calibrated behavior change
(Phase B). Silent non-conservation is never acceptable.

### Chosen representation (decided here to unblock implementation specs)

Every organic pool (fine/coarse detritus, feces, per-stage shrimp reserve, trim residue,
suspended vs attached biomass where already split) is represented as an **explicit element
vector** carrying `mg N`, `mg C`, and `mg P`. Gram-mass views are *derived* from the vector,
not the source of truth. Mixing pools adds vectors; removing a fraction scales them; a
transfer moves the same vector out of source and into sink. This makes every organic route
conservative by construction and is the concrete target for the W0 implementation spec. The
alternative (keep scalar grams + per-source projection) is rejected: it reintroduces exactly
the shared-ratio coupling this release exists to remove.

---

## 2. Findings this release resolves

Verified against code on branch `ralph/tank-sim` and re-checked by two independent xhigh
reviews. File/line references are anchors, not exact contracts.

### F1 — Phosphorus is taken up but never returned as phosphorus (HIGH)

Phosphate is first-class state: dissolved `water.phosphate_mg_p_total` (`types/water.rs:56`)
and per-layer substrate `nutrient_store_mg_p_total` (`types/substrate.rs:80`). The **uptake**
side is real: feed input (`systems/nitrogen_cycle.rs:145,149`), plant uptake at 4.0 mg P/g
(`systems/plant_growth.rs:6,105`), algae uptake at 5.0 mg P/g (`systems/algae_growth.rs:112`),
substrate-store drawdown weighted by the CEC index (`systems/plant_growth.rs:342`; note: this
is drawdown only — there is **no** substrate P *release* path in the cited code), refund of
unused uptake (`systems/plant_growth.rs:271`), and water changes (`systems/water_change.rs`).

The **loss** side does not carry phosphorus at all: when biomass turns over, is grazed,
trimmed, decomposed, or exported, organic mass routes as *grams projected to N and C only*.
The conserved ledger — `BudgetTotals`/`BudgetDelta` (`types/budget.rs:67,92`) and the
`Element` enum (`budget_helpers.rs:70`) — has no phosphorus. Net effect: P assimilated into
biomass (4.0/5.0 mg P/g) has no element-preserving return, and any P the detrital projection
*does* imply (≈ N × feed P:N ≈ 2.8/3.5 mg P/g) does not match uptake. Phosphorus is silently
non-conserved, and the mismatch means true closure is a **behavior change**, not accounting.

### F2 — Conservation enforcement is opt-in, partial, and clamp-masked (MED-HIGH)

A runtime guard exists — `enforce_tracked_tick_budget_guard()` → `SimError::BudgetImbalance`
(`engine.rs:1158`) — but: (1) it is **off by default** (`budget_ledger: None`,
`engine.rs:~219,284`); (2) it checks **N and C only**, not O or P; (3) it is **clamp-masked**
— `BudgetTotals::from_state` floors oxygen with `.max(0.0)` (`types/budget.rs:109`) and
`enforce_invariants` normalizes negative DO before checking (`invariants.rs:13`), hiding a
negative-oxygen event from the total instead of surfacing it.

### F3 — Organism stoichiometry is tied to the fish-food parameter (HIGH, mixed)

Plant/algae tissue carbon is computed as `nitrogen / feed_n_to_c_ratio`
(`types/budget.rs:536,544`; `feed_n_to_c_ratio = 0.16` at
`crates/tank_data/data/process/default.toml:30`).
- **(integrity)** A *feed* parameter sets *organism* carbon content; must be organism-owned.
  Decoupling while preserving current effective values is behavior-neutral.
- **(behavior)** `0.16` implies plant molar C:N ~7 (algae-like); real submersed macrophytes
  are ~15–30. Correcting the value changes carbon drawdown and detrital return.

Shrimp sibling defect **(integrity)**: reserves are funded via `grams × 0.20`
(`LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G`, `types/budget.rs:24`, used
`systems/shrimp.rs:1363,1385`) and projected to N/C by the generic reserve projection, while
death returns body mass via per-species `body_nitrogen/carbon_mg_per_g` (`shrimp.rs:1330`).
These reconcile element-wise only when *both* the reserve's organic fraction *and* its
implied N:C equal the body composition — a coincidence of the defaults, not an identity.

### F4 — Hypoxia inverts the biology (MED, behavior)

Under O2-limited respiration the *un-respired* C and N are added to reserve
(`systems/shrimp.rs:408`, `systems/microfauna.rs:153`). Mass conserves (not an integrity bug)
but the sign is backwards: low DO grows reserve instead of depressing growth.

### F5 — Reproduction multiplies too many sub-unity factors (MED, behavior)

Condition = product of 7 factors (`shrimp.rs:455`); hatch = base × 7 (`:940`); readiness =
8 (`:1586`). 0.8^8 ≈ 0.17. Adult condition is counted three times — readiness (`:1564`),
hatch (`:928`), clutch (`:951`). Behavior/calibration.

### F6 — pH clamp: boundary charge inconsistency + truncated swing (MED, mixed)

`CARBONATE_PH_MAX = 8.5` (`systems/chemistry.rs:111`). At the boundary the solver reprojects
species at the clamped pH and its own comment notes the cached speciation "no longer imply[s]
the original alkalinity" (`chemistry.rs:431`).
- **(integrity)** Cached carbonate state silently mis-states charge at the boundary.
- **(behavior)** Raising the clamp so a lit tank reaches pH 9+ changes output.

---

## 3. Scope

### Phase A — Integrity (behavior-neutral; must not move envelopes)

**W0. Element-explicit organic pools + organism-owned composition (FOUNDATION).**
Replace scalar gram organic pools with the element-vector representation defined in §1. Give
plants, algae, detritus, and shrimp their own C:N:P composition constants (with `param_meta`),
replacing the `feed_n_to_c_ratio`-derived carbon; feed keeps its own ratio for feed only.
Fund shrimp reserves from the same body composition used at death. **Preserve current
effective N and C values** so N/C behavior is unchanged. Every organic route — plant loss →
`fine_detritus_g_total` (`plant_growth.rs:402`), consumer feces/reserve (`shrimp.rs:366`,
`microfauna.rs:110`), trim-leave, decomposition, and every export/import in the A1 route list
— moves the full element vector. Define save/load migration for the changed state shape
(legacy gram fields become derived views or are migrated) and preserve replay determinism.
This is the linchpin; W1 and W3b are unsound without it. Absorbs the integrity half of F3.

**W1a. Phosphorus in the ledger + explicit reconciliation (behavior-neutral).**
Add phosphorus as a first-class conserved element in `BudgetTotals`, `BudgetDelta`,
`BudgetSnapshot`, `Element`, and the biomass→element projection. Wire every P transfer
(uptake, refund, substrate drawdown, feed-derived organic P, and the W0 loss routes) into the
ledger. Because current uptake (4.0/5.0 mg P/g) does not match current return (≈2.8/3.5
mg P/g), introduce an **explicit, logged `phosphorus_reconciliation` source/sink budget term**
that absorbs exactly today's asymmetry so total-P *with the reconciliation term* closes and
observable phosphate behavior is unchanged (A7 holds). The reconciliation term is the
measured size of the leak; it is not a fudge to be left forever — W1b removes it. Resolves the
integrity half of F1.

**W2. Enforce N/C/O/P closure by default in dev/CI; account for clamps.**
Extend the guard to cover oxygen and phosphorus, and make it run without an opt-in ledger in
`debug_assert` builds and in the validation harness (release builds stay free of the cost).
Convert **mass-affecting** clamps from silent `.max(0.0)` into explicit logged budget
source/sinks: in scope are the DO floor (`budget.rs:109`, `invariants.rs:13`) and any pool
`.max(0.0)` that discards tracked N/C/O/P mass; explicitly **exempt** non-mass index clamps
(exposure modifiers, condition/maturity indices, clogging index). Resolves F2.

**W6a. Carbonate boundary charge consistency (clamp unchanged).**
At the pH clamp boundary, **reproject the cached carbonate speciation to preserve the tracked
alkalinity** (the definite contract — not "record a residual"). Do not change the clamp value
in Phase A. Update `docs/carbonate_state_contract.md`. Resolves the integrity half of F6.

### Phase B — Behavior corrections (envelope-moving; each individually justified)

Land only after Phase A is green and the guard is active, so every envelope shift is
attributable to a deliberate correction and re-calibrated.

**W1b. Close the phosphorus leak.** Route P through loss paths at true tissue P content and
**remove the W1a reconciliation term**; phosphate concentrations now change. Re-run
calibration; document envelope deltas (algae/plant dynamics are P-sensitive). Resolves the
behavior half of F1.

**W3b. Realistic macrophyte C:N.** Set plant tissue C:N to a literature macrophyte value
(molar ~15–30). Re-run calibration; justify envelopes. (behavior half of F3)

**W4. Hypoxia response.** Under O2 limitation, suppress assimilation/growth instead of
crediting un-respired C/N to reserve; un-assimilated food routes to feces/detritus (via W0),
never phantom reserve; total mass still closes. Resolves F4.

**W5. Reproduction aggregation.** Replace the product of sub-unity factors with a defensible
aggregation (Liebig minimum or geometric mean of independent stressors) and count adult
condition once. Re-tune only to keep envelopes sane. Resolves F5.

**W6b. Raise pH clamp.** Raise `CARBONATE_PH_MAX` to admit realistic daytime highs (target up
to ~pH 9.5) on top of W6a; verify solver stability across 15–35 °C. Resolves the behavior half
of F6.

**W7. Documentation refresh.** Update `PROVENANCE_STATUS.md`, `scientific_specs.md`, README to
reflect already-shipped phosphate/potassium tracking and live DIC rates, and record the new
conservation guarantees and per-guild stoichiometry.

### Explicitly out of scope (deferred to v0.8+ realism release)

Activity-coefficient (Davies) correction; free-CO2 vs bicarbonate split; humic/organic-acid
buffering; iron/potassium co-limitation; algae functional types; CEC GH/KH buffering and
aquasoil ammonia leaching; molt-death-syndrome specifics and water-change osmotic
shock/acclimation; the algae NH4-uptake 60% cap vs plant full-NH4 inconsistency
(`systems/algae_growth.rs:375`).

---

## 4. Acceptance criteria

Phase A (behavior-neutral):
A1. **Element-explicit route matrix.** A test exercises **every** organic import/export and
    asserts N, C, and P each close independently: plant/algae turnover, grazing, feces,
    trim-leave, trim-and-remove (export), decomposition, `siphon_detritus`, filter-cleaning
    biomass routing, shrimp add/remove (body + reserve), and water-change export of suspended
    algae. (W0, W1a)
A2. **Phosphorus structural + dynamic closure (with reconciliation).** P has structural budget
    coverage mirroring N/C, and a multi-day run conserves total P **including the explicit
    reconciliation term** to fp tolerance; the reconciliation term is reported and non-growing
    in steady state. (W1a)
A3. **Guard covers N/C/O/P, on in dev/CI.** Injecting a create/destroy of any of the four
    elements trips the guard/`debug_assert` or a harness failure, with the ledger no longer
    opt-in for those builds. (W2)
A4. **Clamp accounting.** A forced negative-DO condition (and any other in-scope
    mass-discarding clamp) records an explicit clamp source/sink budget metric rather than a
    silent floor. (W2)
A5. **Stoichiometry organism-owned and value-preserving.** Changing `feed_n_to_c_ratio` no
    longer changes plant/algae tissue carbon; a birth→death cycle with a **non-default** shrimp
    body composition conserves N **and** C **independently** (asserted separately, not summed).
    (W0)
A6. **Carbonate boundary consistency.** A boundary test recomputes alkalinity from the cached
    species and matches the tracked alkalinity within tolerance. (W6a)
A7. **Phase A moves no envelope.** The eight validation scenarios and the calibration report
    are unchanged after Phase A (P observable behavior held constant by the reconciliation
    term). (all Phase A)
A8. **Determinism + migration.** Seeded save→load→replay is bit-identical after the W0 state
    reshape; a legacy save loads via migration with observable state preserved. (W0)

Phase B (behavior; changes justified):
B1. Phosphate dynamics change in a documented direction after the reconciliation term is
    removed; envelopes re-justified. (W1b)
B2. Plant tissue molar C:N sits in the literature macrophyte band; deltas documented. (W3b)
B3. Low-DO scenario: shrimp/microfauna reserve and growth **decrease** vs normoxia; N/C still
    close. (W4)
B4. Near-optimal conditions yield reproductive output in a sane range; condition not
    triple-counted. (W5)
B5. A brightly lit CO2-drawdown scenario reaches pH > 8.5 with the carbonate contract holding
    at the new boundary. (W6b)

Cross-cutting:
C1. `cargo test && cargo clippy -- -D warnings && cargo fmt --check` pass. (all)
C2. No wall clock, no unseeded RNG introduced. (all)

---

## 5. Risks and mitigations

- **W0 is invasive** (touches many systems + persisted state). Mitigation: first item; land
  behavior-neutral with migration; prove A7 + A8 before anything else.
- **Reconciliation term misused.** Risk it becomes a permanent fudge. Mitigation: it is
  reported every tick (A2), and W1b's definition of done is its removal.
- **Guard failing "correctly" mid-change.** If stoichiometry/P closure changes before W0/W1a
  land, the active guard legitimately fails. Mitigation: strict order W0 → W1a → W2.
- **Phase B re-tuning cascade.** Mitigation: one Phase B item at a time, re-run calibration,
  change envelopes only with written justification.
- **Debug-assert cost.** Mitigation: gate under `debug_assert`/harness flag.
- **Boundary math (W6).** Mitigation: verify against carbonate contract vectors 15–35 °C.

---

## 6. Deliverables

1. Code in `tank_core`: Phase A (W0 → W1a → W2 → W6a) then Phase B (W1b, W3b, W4, W5, W6b),
   plus W7 docs.
2. Tests: route-matrix N/C/P, P structural + dynamic (with reconciliation), N/C/O/P
   guard-injection, clamp accounting, non-default shrimp composition (N and C separate),
   carbonate boundary alkalinity recomputation, save-migration/replay determinism; updated
   hypoxia, reproduction, and pH tests.
3. Refreshed docs (W7) and updated calibration report.
4. Detailed, buildable per-work-item implementation specs (W0 first), reviewed before coding.

---

## 7. Sequencing

1. **W0** element-explicit pools + organism-owned composition + migration → prove A5, A7, A8.
2. **W1a** P ledger + explicit reconciliation term → A1, A2, A7.
3. **W2** N/C/O/P guard on-by-default-in-dev + clamp accounting → A3, A4.
4. **W6a** carbonate boundary consistency → A6.
5. **Phase A gate:** full suite + calibration report unchanged (A7, A8).
6. **W1b → W3b → W4 → W5 → W6b**, one at a time, each re-running calibration (B1–B5).
7. **W7** docs → final suite + calibration report.

Implementation does not begin until the per-item implementation specs (Deliverable 4, W0
first) are written and have passed their own review.
