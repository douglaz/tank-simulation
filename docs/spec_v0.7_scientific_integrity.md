# Spec: v0.7 "Scientific Integrity" Release

Status: DRAFT rev 4 (revised after three codex xhigh reviews)
Author: scientific-core review, 2026-07-03
Scope owner: `tank_core`
Predecessor: v0.2–v0.6 scientific-core overhaul (complete; backlog drained)

---

## 1. Motivation

The v0.2–v0.6 overhaul gave the simulator broad, mechanistic coverage: concentration-based
Monod kinetics, AOB/NOB/comammox nitrification, denitrification, a closed-form carbonate
equilibrium solver, a five-zone habitat registry, stage-structured shrimp, and parameter
provenance. The breadth work is done. A code-level audit found that the project's central
claim — **"deterministic mass-balance chemistry"** (README.md:47) — is only partially
enforced (opt-in, N/C only, clamp-masked) and cannot be enforced for phosphorus at all
because phosphorus is taken up into biomass that the model does not track.

This release makes the model *provably conservative and internally consistent*, in two phases:

- **Phase A — Integrity (behavior-neutral).** Accounting, enforcement, and representation
  changes that must NOT move any validation envelope. Phase A hardens conservation for the
  elements that are *fully tracked and closable today* — nitrogen, carbon, oxygen — by
  extending the runtime guard to all three, enabling it by default in dev/CI, and accounting
  for clamps. It also introduces the element-vector representation and organism-owned N/C
  composition that later work depends on, and it makes phosphorus **visible** in the ledger
  and snapshots (observability) so its imbalance can be measured. Phase A does **not** claim
  to enforce phosphorus conservation, because you cannot enforce conservation of a
  half-tracked element.
- **Phase B — Behavior corrections (envelope-moving, individually justified).** The changes
  that necessarily move output: actually *tracking and closing* phosphorus (biomass + organic
  P, routed through every pathway, decomposition P unified with the detritus P content, guard
  extended to P), plus hypoxia response, reproduction aggregation, macrophyte C:N, and the
  pH-clamp / carbonate-boundary correction. Each re-runs calibration with written envelope
  justification.

The "no new science" framing from rev 1 was inaccurate and has been dropped; Phase B changes
behavior on purpose. The split isolates that risk and keeps the guard meaningful.

### Guiding principle

Every element the model *fully tracks* must be conserved across every transformation, or the
transformation must be an explicitly accounted source/sink. A partially-tracked element
(phosphorus, today) is first made **visible** in the ledger (Phase A) and then made **fully
tracked and enforced** as a deliberate, re-calibrated change (Phase B). Silent
non-conservation is never acceptable; neither is a guard that pretends to enforce an element
it cannot yet close.

### Chosen representation (decided here to unblock implementation specs)

Every genuine independent organic pool (fine/coarse detritus, per-stage shrimp reserve, trim
residue) is an **explicit element vector** carrying `mg N`, `mg C`, and `mg P`. Gram-mass
views are *derived from N+C only* (matching today's semantics, where a gram of detritus is
1000 mg of N+C), so introducing the vector with `p_mg = 0` is behavior-neutral. Phosphorus
enters those `p_mg` slots only in Phase B (W1b), when it becomes fully tracked. Mixing pools
adds vectors; removing a fraction scales them; a transfer moves the same vector out of source
and into sink — conservative by construction. The shadow DOC/DON feed-residue counter is
**not** an independent pool and stays scalar.

---

## 2. Findings this release resolves

Verified against code on branch `ralph/tank-sim` and re-checked by three independent xhigh
reviews. File/line references are anchors, not exact contracts.

### F1 — Phosphorus is taken up but never conserved (HIGH)

Phosphate is first-class state: dissolved `water.phosphate_mg_p_total` (`types/water.rs:56`)
and per-layer substrate `nutrient_store_mg_p_total` (`types/substrate.rs:80`). Uptake is real
(plants 4.0 mg P/g `plant_growth.rs:6,105`; algae 5.0 `algae_growth.rs:112`; substrate
drawdown weighted by CEC `plant_growth.rs:342`; refund `plant_growth.rs:271`). Phosphate is
also *released* by detritus dissolution — `phosphate_mg = don_mg * FEED_P_TO_N_MASS_RATIO`
(`nitrogen_cycle.rs:141-149`). But P assimilated into biomass and stored in detritus is **not
a tracked pool**, and the ledger — `BudgetTotals`/`BudgetDelta` (`types/budget.rs:67,92`),
`Element` (`budget_helpers.rs:70`) — has no phosphorus at all. The uptake (4.0/5.0 mg P/g) and
the N-proportional release (`don × ratio`) do not match, so phosphorus is silently
non-conserved, and *closing* it necessarily changes phosphate dynamics (a behavior change).

### F2 — Conservation enforcement is opt-in, partial, and clamp-masked (MED-HIGH)

The guard `enforce_tracked_tick_budget_guard()` (`engine.rs:1158`) exists but: (1) is off by
default (`budget_ledger: None`, `engine.rs:~219`); (2) checks N and C only — not O; (3) is
clamp-masked — `BudgetTotals::from_state` floors oxygen with `.max(0.0)` (`budget.rs:109`) and
invariants normalize negative DO (`invariants.rs`), hiding negative-oxygen events.

### F3 — Organism stoichiometry is tied to the fish-food parameter (HIGH, mixed)

Plant/algae tissue carbon = `nitrogen / feed_n_to_c_ratio` (`budget.rs:536,544`; ratio 0.16 at
`crates/tank_data/data/process/default.toml:30`). **(integrity)** a *feed* parameter sets
*organism* carbon; decoupling while preserving values is behavior-neutral. **(behavior)** 0.16
implies plant molar C:N ~7 (algae-like) vs real macrophyte ~15–30; correcting the value moves
carbon drawdown. Shrimp sibling **(integrity)**: reserves funded via `grams × 0.20`
(`LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G`, `budget.rs:24`, `shrimp.rs:1363,1385`) vs death via
`body_nitrogen/carbon_mg_per_g` (`shrimp.rs:1330`) reconcile only by coincidence of defaults.

### F4 — Hypoxia inverts the biology (MED, behavior)
Un-respired C/N added to reserve under O2 limitation (`shrimp.rs:408`, `microfauna.rs:153`):
mass conserves but low DO grows reserve instead of depressing growth.

### F5 — Reproduction multiplies too many sub-unity factors (MED, behavior)
Condition = 7-factor product (`shrimp.rs:455`), hatch = base × 7 (`:940`), readiness = 8
(`:1586`); adult condition counted three times (`:1564,:928,:951`). 0.8^8 ≈ 0.17.

### F6 — pH clamp: boundary charge inconsistency + truncated swing (MED, mixed)
`CARBONATE_PH_MAX = 8.5` (`chemistry.rs:111`). At the clamp, species are reprojected from DIC
and the cached `HCO3-/CO3--` "no longer imply the original alkalinity" (`chemistry.rs:431`).
**(integrity)** the residual is currently silent. **(behavior)** raising the clamp and making
species alkalinity-consistent changes output and the DIC-preserving carbonate tests/contract
(`tests/chemistry.rs:285-347`, `carbonate_state_contract.md:63-70`).

---

## 3. Scope

### Phase A — Integrity (behavior-neutral; must not move envelopes)

**W0. Element-explicit organic pools + organism-owned N/C composition (FOUNDATION).**
Replace the scalar-gram independent organic pools (fine detritus, particulate organics,
per-stage shrimp reserve, and the equivalent microfauna reserve) with the `(N,C,P)`
element-vector of §1, gram view = N+C only, `p_mg = 0` for now. Give plants/algae/shrimp their
own N and C composition constants (with `param_meta`), replacing `feed_n_to_c_ratio`
projection (feed keeps its ratio for feed only); fund shrimp reserve from the same body N/C
composition used at death. Sweep every raw gram read/write to the vector. Define a save
migration for the changed state shape and preserve replay determinism. Absorbs the integrity
half of F3. The `p_mg` slots are the seam W1b fills.

**W1a. Phosphorus ledger plumbing + observability (behavior-neutral, no enforcement).**
Add `Element::Phosphorus`, `BudgetDelta.phosphorus`, `BudgetTotals.phosphorus_mg`, a snapshot
`phosphorus_components`, and `total_phosphorus_mg(state)` over the only currently-tracked P
state (`water.phosphate_mg_p_total` + substrate `nutrient_store_mg_p_total`). This makes the
phosphorus imbalance *visible* in budget inspection without changing any value. The runtime
guard does **not** enforce phosphorus in Phase A (it cannot close while biomass/organic P is
untracked). No reconciliation term in Phase A. Resolves the observability half of F1.

**W2. Enforce N/C/O by default in dev/CI; account for clamps.**
Extend the guard to oxygen using the existing gross in/out flux-balance semantics
(`assert_o2_balanced`, `budget_helpers.rs:315-325`) — O2 is flux-balanced, not flat-conserved.
Make the guard run without an opt-in ledger under `debug_assertions` and a harness flag
(release builds stay `None`, zero cost). Record mass-discarding clamps (the DO floor) as an
explicit signed-debt **diagnostic** (not a conserved-ledger delta, which would violate
`record_stage`'s delta assertion), and exempt non-mass index clamps. Resolves F2.

**W6a. Carbonate boundary residual diagnostic (behavior-neutral).**
When the pH clamp engages, compute the alkalinity implied by the cached species and emit the
difference vs `alkalinity_meq_total` as an explicit diagnostic. Do **not** change species, DIC,
or pH (that is W6b). Resolves the observability half of F6.

### Phase B — Behavior corrections (envelope-moving; each individually justified)

**W1b. Track and close phosphorus.** Introduce P composition constants (plant/algae tissue P,
shrimp `body_phosphorus_mg_per_g_wet_mass`) and populate the `p_mg` slots on every organic
route (feed input, turnover, feces, reserve, microbial decay, decomposition). Feed-derived P
must be pinned to a real feed P content so that replacing the `don × ratio` phosphate shortcut
does not silently delete feed phosphorus. Add derived biomass P and organic P to
`total_phosphorus_mg`; unify decomposition P release with the tracked detritus P content; extend
the guard to phosphorus. Ripple shrimp
body-P into presets and scenario materialization (currently N/C only). Re-run calibration;
phosphate dynamics shift — document deltas. Closes F1.

**W3b. Realistic macrophyte C:N.** Raise plant tissue C to a literature macrophyte value
(molar C:N 15–30). Re-calibrate; justify envelopes. (behavior half of F3)

**W4. Hypoxia response.** Under O2 limitation, suppress assimilation/growth instead of
crediting un-respired C/N to reserve; unassimilated food routes to feces/detritus (via W0);
mass still closes. Resolves F4.

**W5. Reproduction aggregation.** Replace the product of sub-unity factors with a Liebig
minimum or geometric mean and count adult condition once. Re-tune to hold envelopes. Resolves
F5.

**W6b. Alkalinity-consistent boundary + raise pH clamp.** Reproject cached species from
alkalinity at the clamp (deliberately updating the DIC-preserving tests and the carbonate
contract), raise `CARBONATE_PH_MAX` to ~pH 9.5, verify solver stability 15–35 °C. Resolves the
behavior half of F6.

**W7. Documentation refresh.** Update `PROVENANCE_STATUS.md`, `scientific_specs.md`, README to
reflect already-shipped phosphate/potassium tracking and live DIC rates, and record the new
N/C/O (then N/C/O/P) conservation guarantees and per-guild stoichiometry.

### Explicitly out of scope (deferred to v0.8+ realism release)

Activity-coefficient (Davies) correction; free-CO2 vs bicarbonate split; humic/organic-acid
buffering; iron/potassium co-limitation; algae functional types; CEC GH/KH buffering and
aquasoil ammonia leaching; molt-death-syndrome specifics and water-change osmotic
shock/acclimation; the algae NH4-uptake 60% cap vs plant full-NH4 inconsistency
(`systems/algae_growth.rs:375`).

---

## 4. Acceptance criteria

Phase A (behavior-neutral):
A1. **N/C route matrix.** A test exercises every organic import/export (plant/algae turnover,
    grazing, feces, trim-leave, trim-and-remove, decomposition, `siphon_detritus`,
    filter-cleaning, shrimp add/remove, water-change export) and asserts N and C each close
    independently through the new element vectors. (W0)
A2. **Phosphorus observability.** `total_phosphorus_mg` sums water + substrate P and appears in
    budget snapshots; the phosphorus imbalance is inspectable. No enforcement claim. (W1a)
A3. **Guard covers N/C/O, on in dev/CI.** Injecting a create/destroy of N, C, or O trips the
    guard/`debug_assert` or a harness failure with the ledger no longer opt-in for those
    builds. (W2)
A4. **Clamp accounting.** A forced negative-DO condition records an explicit signed-debt
    diagnostic (not a conserved-ledger delta). (W2)
A5. **Stoichiometry organism-owned, value-preserving.** Changing `feed_n_to_c_ratio` no longer
    changes plant/algae tissue carbon; a birth→death cycle with a non-default shrimp body
    composition conserves N and C independently (asserted separately). (W0)
A6. **Carbonate boundary residual reported.** A clamp-boundary test asserts the residual
    diagnostic equals `implied_alk(cached species) − tracked_alk`, with DIC/species/pH
    unchanged vs today. (W6a)
A7. **Phase A moves no envelope.** The eight validation scenarios and the calibration report
    are unchanged after Phase A. (all)
A8. **Determinism + migration.** Seeded save→load→replay is bit-identical after the W0 reshape;
    a legacy save loads via migration with observable state preserved. (W0)

Phase B (behavior; changes justified):
B1. **Phosphorus closes and is enforced.** After W1b, a per-route N/C/P matrix closes P
    independently, the guard enforces phosphorus, and phosphate dynamics change in a documented
    direction. (W1b)
B2. Plant tissue molar C:N in the literature macrophyte band; deltas documented. (W3b)
B3. Low-DO scenario: shrimp/microfauna reserve and growth decrease vs normoxia; N/C close. (W4)
B4. Near-optimal conditions yield reproductive output in a sane range; condition counted once.
    (W5)
B5. A brightly lit CO2-drawdown scenario reaches pH > 8.5 with the carbonate contract holding
    at the new boundary. (W6b)

Cross-cutting:
C1. `cargo test && cargo clippy -- -D warnings && cargo fmt --check` pass. (all)
C2. No wall clock, no unseeded RNG introduced. (all)

---

## 5. Risks and mitigations

- **W0 is invasive** (many systems + persisted state). Mitigation: first item; land
  behavior-neutral with migration; prove A5, A7, A8 before anything else.
- **Half-tracked P looks "done" after Phase A.** Mitigation: A2 explicitly makes no enforcement
  claim; W1b's definition of done is guard-enforced P closure (B1).
- **Guard failing "correctly" mid-change.** Mitigation: strict order W0 → W1a → W2; P
  enforcement only after W1b tracks biomass/organic P.
- **Phase B re-tuning cascade.** Mitigation: one item at a time, re-run calibration, change
  envelopes only with written justification.
- **Debug-assert cost.** Mitigation: gate under `debug_assertions`/harness flag.
- **Boundary math (W6b).** Mitigation: verify against carbonate contract vectors 15–35 °C and
  update them deliberately.

---

## 6. Deliverables

1. Code in `tank_core` (+`tank_data`/`tank_scenarios` for W1b): Phase A (W0 → W1a → W2 → W6a)
   then Phase B (W1b, W3b, W4, W5, W6b), plus W7 docs.
2. Tests: N/C route matrix, P observability, N/C/O guard-injection, clamp diagnostic,
   non-default shrimp composition (N and C separate), carbonate boundary residual,
   save-migration/replay determinism; then W1b P route matrix + guard, updated hypoxia,
   reproduction, and pH/carbonate tests.
3. Refreshed docs (W7) and updated calibration report.
4. Detailed, buildable per-work-item implementation specs (W0 first), reviewed before coding —
   see `docs/impl_spec_v0.7.md`.

---

## 7. Sequencing

1. **W0** element-explicit pools + organism-owned composition + migration → prove A5, A7, A8.
2. **W1a** phosphorus ledger plumbing + observability → A2, A7.
3. **W2** N/C/O guard on-by-default-in-dev + clamp diagnostic → A3, A4.
4. **W6a** carbonate boundary residual diagnostic → A6.
5. **Phase A gate:** full suite + calibration report unchanged (A7, A8).
6. **W1b → W3b → W4 → W5 → W6b**, one at a time, each re-running calibration (B1–B5).
7. **W7** docs → final suite + calibration report.

Implementation does not begin until the per-item implementation specs (Deliverable 4, W0
first) are written and have passed their own review.
