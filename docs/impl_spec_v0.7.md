# Implementation Spec: v0.7 "Scientific Integrity"

Companion to `docs/spec_v0.7_scientific_integrity.md` (rev 5). Defines *how*, at a level an
implementer can build against. Rev 4 after a fresh-eyes + codex FP review: behavior-neutral is
an FP tolerance (not byte-for-byte), plant/algae carbon decouples via an organism-owned N:C
*ratio* constant to stay bit-identical, W1a extends the consistency assertion to phosphorus, and
W2 spikes the O2 balance before enabling the guard by default. The governing insight:
**the only truly behavior-neutral Phase A work is accounting/representation. You cannot
enforce conservation of a half-tracked element, so Phase A hardens the guard for the fully
tracked N/C/O, makes phosphorus merely *visible* in the ledger, and defers all phosphorus
tracking/routing/closure — and any carbonate species reprojection — to Phase B.** Phase A
*exposes* the phosphorus and carbonate-boundary imbalances; Phase B *closes* them. There is
no reconciliation term in Phase A (it would be vacuous over water+substrate P alone).

Conventions: paths repo-relative; line anchors reflect branch `ralph/tank-sim` at spec time
and may drift. "Behavior-neutral" = the eight validation scenarios keep the same
pass/marginal/fail status and stay inside their envelopes, and the calibration report's observed
numeric values are unchanged **within a tight floating-point tolerance** (relative ~1e-9),
ignoring non-deterministic report metadata (e.g. `generated_at`, `calibration.rs:307`, which
already makes the report non-reproducible byte-for-byte). It is **not** byte-for-byte: W0's
stoichiometry decoupling and the organic-pool save migration both re-associate floating-point
arithmetic, so a fraction of low-order bits legitimately change (see W0). Bit-identity is
required only for A8's *new-code* save→load→replay determinism, never for an old-vs-new
comparison. The harness comparison is already status-only (`calibration.rs:315`), so this
tolerance framing matches how the report is actually diffed.

---

## W0 — Element-explicit organic pools + organism-owned N/C composition

### Goal
Introduce an `(N, C, P)` mass vector for the genuine independent organic pools and give
plants/algae/shrimp organism-owned **N and C** composition, replacing `feed_n_to_c_ratio`
projection. Behavior-neutral: **P is carried but zero in every organic pool** (detritus
carries no P today), and the derived gram view excludes P so pool mass is unchanged.

### `OrganicMass`
Add to `crates/tank_core/src/types/budget.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct OrganicMass { pub n_mg: f64, pub c_mg: f64, pub p_mg: f64 }
```
Ops: `add`, `sub`, `scale(frac∈[0,1])`, `split(removed_frac)->(kept,removed)`, `is_finite`,
`has_negative`. Critical (review fix): the derived gram view is **N+C only**, matching
today's semantics where `organic_nitrogen_mg + organic_carbon_mg` split `mass_g*1000`
(`budget.rs:648-656`):
```rust
fn dry_mass_g(&self) -> f64 { (self.n_mg + self.c_mg) / 1000.0 } // P intentionally excluded
```
`p_mg` is tracked but never part of `dry_mass_g`; W1b decides how P re-enters mass semantics.

### State changes (`types/biology.rs`) — ONLY genuine independent pools
Convert to `OrganicMass`:
- `fine_detritus_g_total` (~543) → `fine_detritus: OrganicMass`
- `particulate_organics_g_total` (~542) → `particulate_organics: OrganicMass`
- per-stage shrimp `reserve_g` (~85, ~94) → `reserve: OrganicMass`
- the microfauna `reserve_g` field (same anchor region) → `reserve: OrganicMass` (review fix:
  call it out explicitly; it is a distinct pool from the shrimp stage reserves)

**Do NOT convert `dissolved_feed_residue_g_total`** (review fix): it is a shadow counter for
DOC/DON-origin residue, explicitly excluded from budget deltas
(`water_change.rs:60-63`, asserted in `tests/budget_tracking.rs:461-465`). Leave it scalar.

Preserve every public gram reader as a derived accessor
(`fn fine_detritus_g_total(&self)->f64 { self.fine_detritus.dry_mass_g() }`). Snapshot fields
(`snapshot.rs:78-79`) stay `f64` via the accessor.

### Call-site sweep (review fix — accessors alone do not cover writes)
Every raw read/write of the converted scalars must move to `OrganicMass`:
- `ProcessAction::Feed` adds raw grams to `particulate_organics` (`engine.rs:884-886`) — build
  an `OrganicMass` (p=0 in W0).
- nitrogen-cycle detritus dissolution (`nitrogen_cycle.rs:~141-149`), microbial decay
  (`nitrogen_cycle.rs:725-738`), and `route_live_biomass_to_dissolved_organics`
  (`engine.rs:1184-1193`) — use guild composition, drop `feed_n_to_c_ratio`.
- plant/algae turnover (`plant_growth.rs:425-430`, `budget.rs:548-554` algae detrital),
  consumer feces/reserve (`shrimp.rs:353-412`, `microfauna.rs:98-156`), substrate O2 demand,
  filter clogging, tracing, snapshots, and existing tests that read the scalars.

### Organism-owned N/C composition (behavior-neutral)
Give plants/algae their own N:C **ratio** constant, **not** a pre-divided carbon-per-gram
constant. Decoupling from the feed parameter must preserve the exact arithmetic association so
the dominant carbon path stays bit-for-bit unchanged (review fix). Add
`PLANT_N_TO_C_RATIO = 0.16` and `ALGAE_N_TO_C_RATIO = 0.16` (organism-owned, `param_meta`) next
to the existing N constants (`budget.rs:16-18`), and keep the carbon projectors
(`plant_carbon_mg`, `algae_carbon_mg`, `live_biomass_carbon_mg`, `detritus_carbon_mg`,
`budget.rs:536-660`) computing the *same expression* —
`carbon_from_nitrogen_mg(plant_nitrogen_mg(biomass), PLANT_N_TO_C_RATIO)` — with the organism's
own ratio in place of `feed_n_to_c_ratio`. Since the value is identical (0.16) and the
expression is unchanged, plant/algae tissue carbon (which feeds budget totals **and** detrital
mass routing, `algae_detrital_mass_g` `budget.rs:548`, `live_biomass_detrital_mass_g`
`budget.rs:570`) is bit-identical, while feed's ratio no longer governs organism carbon (A5).
Feed keeps `feed_n_to_c_ratio` for *feed input only*.

**Do NOT substitute a pre-divided `biomass × 175.0` / `× 218.75` constant here** (review fix):
even though `28.0/0.16 == 175.0` exactly as a constant, `(biomass × 28.0) / 0.16` differs
bit-for-bit from `biomass × 175.0` in ~28% of inputs (double rounding), which would perturb
dynamics and the report for no benefit. The derived carbon-per-gram *equals* `175.0`/`218.75`;
W3b changes the **ratio** to a literature macrophyte value (that is where carbon is meant to
move). Fund shrimp **reserve** from body N/C composition
(`body_nitrogen/carbon_mg_per_g_wet_mass`) instead of `grams × 0.20 × feed ratio`, so reserve
and death use the same composition (fixes F3 integrity); this path genuinely re-associates
arithmetic (F3: the two "reconcile only by coincidence of defaults"), so its low-order bits move
within the tolerance defined above — that is the one W0 path that is not bit-identical, and it is
covered by the FP-tolerance bar, not byte-for-byte. **P composition constants are NOT introduced
here** — they arrive in W1b when P actually flows.

### Migration (review fix)
`save.rs` uses `SCHEMA_VERSION = 14` with a contiguous migration list (`save.rs:17,46-76`).
Add an explicit **v14→v15 raw-JSON migration** that maps each legacy scalar
(`fine_detritus_g_total`, `particulate_organics_g_total`, per-stage and microfauna
`reserve_g`) to `{n_mg, c_mg, p_mg}` via the legacy projection
(`organic_nitrogen_mg`/`organic_carbon_mg`, `p_mg = 0`) using the **saved** process
`feed_n_to_c_ratio` from that save file, not the current default (review fix: a legacy save
with a non-default ratio must migrate with its own ratio to stay behavior-neutral). Serde
defaults are insufficient; the migration is required. Bump to `SCHEMA_VERSION = 15`.

Note (review fix): the gram→`{n,c}`→gram round-trip is **not** bit-exact. A legacy scalar `G`
maps to `n = organic_nitrogen_mg(G, ratio)`, `c = organic_carbon_mg(G, ratio)`, and the derived
accessor recomputes `(n + c) / 1000`; the split, the addition, and the divide each round, so the
reconstructed gram value can differ from `G` in the last bits. This is expected and covered by
the FP-tolerance behavior-neutral bar — A8's legacy-migration clause means "observable state
preserved **within tolerance**", not bit-identical grams. New-code save→load→replay determinism
(same code both sides) remains exactly bit-identical.

### Tests (`crates/tank_core/tests/organic_mass_pools.rs`)
- `organic_mass_split_conserves` (N/C/P exact through split+reassemble).
- `w0_behavior_neutral` — eight scenarios keep identical pass/marginal/fail status and stay in
  envelope pre/post via `tank_harness`; observed values match within relative ~1e-9 (A7). Assert
  status + tolerance, **not** exact equality (the shrimp-reserve path and migration round-trip
  move low-order bits by design).
- `w0_legacy_save_migrates` — v14 fixture loads to identical observable state (A8).
- `w0_nondefault_shrimp_composition_conserves_n_and_c` — non-default body N/C summing ≠200;
  birth→death with `budget_ledger` on; N and C each close independently (A5).

---

## W1a — Phosphorus ledger plumbing + observability (no enforcement)

### Why observability, not enforcement (review fix)
Phosphate is already released by the model (`phosphate_mg = don_mg * FEED_P_TO_N_MASS_RATIO`,
`nitrogen_cycle.rs:141-149`) and taken up by plants/algae; the only **tracked** P pools are
`water.phosphate_mg_p_total` and substrate `nutrient_store_mg_p_total`. Biomass and organic P
are untracked. With only water+substrate P tracked, *every* P change is a boundary flow with
untracked biomass, so a reconciliation term would either mask all changes (guard vacuous) or
none (guard always fails). There is nothing to enforce yet. W1a therefore only makes
phosphorus **visible**; enforcement and closure are W1b (Phase B). **No reconciliation term.**

### W1a is pure plumbing — no state writes changed
- `Element` (`budget_helpers.rs:70`): add `Phosphorus`. `BudgetDelta` (`budget.rs:67`): add
  `phosphorus: ElementBudget`. `BudgetTotals` (`budget.rs:92`): add `phosphorus_mg`.
  `BudgetSnapshot`: add `phosphorus_components`.
- `total_phosphorus_mg(state)` = `water.phosphate_mg_p_total` + Σ substrate
  `nutrient_store_mg_p_total`. Surface it in budget snapshots so the imbalance is inspectable.
- The runtime guard does **not** check phosphorus in Phase A (added in W1b once biomass/organic
  P is tracked). No behavior change (accessors/sums only).
- **Extend the consistency assertion to phosphorus, then populate explicit budget producers**
  (review fix). `record_stage`'s `delta_net_matches_totals` (`budget.rs:698-704`) checks only
  N/C/O today; W1a extends it to phosphorus so the ledger's P accounting is actually asserted
  (otherwise a producer that leaves `phosphorus` at default while moving phosphate is a silent
  wrong-inspection, not a caught error). With the assertion extended, every explicit
  (non-snapshot) `BudgetDelta` producer must carry a P delta consistent with its phosphate move.
  Of the three production producers — `step_hourly_chemistry_with_budget` (`chemistry.rs:558`),
  `apply_water_change_with_budget` (`water_change.rs:99`), `step_dissolved_oxygen_with_budget`
  (`dissolved_oxygen.rs:64`) — only water-change moves phosphate (`water_change.rs:56,77`) and so
  must set a **non-zero** phosphorus field (it returns N/C/O only today, `water_change.rs:134`);
  chemistry and DO satisfy the extended assertion automatically with a zero P delta (their
  before/after `phosphorus_mg` are equal). This is bookkeeping only (the phosphate state moves are
  unchanged).

The per-route P closure test and guard enforcement live in **W1b**; W1a keeps only A2
(observability) and A7 (phosphate series unchanged).

### Tests (`crates/tank_core/tests/phosphorus_conservation.rs`)
- `phosphorus_structural_coverage` — `total_phosphorus_mg` = water + substrate P for a
  hand-built state; snapshot exposes it (A2).
- `w1a_phosphate_series_unchanged` — phosphate time series identical pre/post (A7).

---

## W2 — Enforce N/C/O in dev/CI; diagnostic clamp accounting

Phosphorus is intentionally **not** in the Phase A guard (it is not closable until W1b tracks
biomass/organic P). W2 hardens the three fully-tracked elements.

### Guard extension (`engine.rs:1158`)
- **Oxygen** (review fix — O2 is flux-balanced, not flat): do **not** use `net − open_flux`.
  Promote the existing gross in/out check (`assert_o2_balanced`, `budget_helpers.rs:315-325`)
  into the runtime guard: assert the recorded O2 in/out entries (photosynthesis, reaeration,
  respiration, nitrification, shrimp, microfauna — `dissolved_oxygen.rs:121-132`,
  `nitrogen_cycle.rs:374-377,436-437,480-482`, `shrimp.rs:397-400`, `microfauna.rs:142-145`)
  account for the net DO change to tolerance.
- Extend `enforce_tracked_tick_budget_guard` with an oxygen branch →
  `SimError::BudgetImbalance { element: "oxygen", .. }`. (The phosphorus branch is added in
  W1b.)

### On-by-default in dev/CI
Under `cfg!(debug_assertions)` and a `TANK_BUDGET_GUARD=1` harness flag, construct the engine
with `budget_ledger` enabled (default stays `None`, `engine.rs:~219`, so release is zero-cost)
and run the guard each tick. Wire `tank_harness` to set the flag on all eight scenarios.

**Spike the O2 balance before wiring guard-by-default** (review fix). The guard has been opt-in
(`budget_ledger: None`) and N/C-only, so it has **never** run across the eight scenarios with
oxygen included, and the DO `.max(0.0)` floor (`budget.rs:109`) plus invariants DO normalization
have been masking any negative-DO / imbalance events. Before adding the on-by-default wiring, run
`assert_o2_balanced` (`budget_helpers.rs:322`) across all eight scenarios as a throwaway spike. If
they all close to tolerance, proceed — W2 is the behavior-neutral hardening it claims to be. If
any scenario does **not** close, that is a pre-existing conservation defect the guard has newly
surfaced: treat it as a **separate, tracked defect** with its own envelope decision (it may be
behavior-changing and belong in Phase B), **not** a W2 tolerance-widening exercise. Do not make
the guard pass by loosening tolerance.

### Clamp accounting (review fix — diagnostic, NOT a ledger delta)
Adding a budget delta for the DO floor would violate `record_stage`'s delta-vs-floored-totals
assertion (`budget.rs:698-704`) because `BudgetTotals::from_state` already floors oxygen
(`budget.rs:109`) and invariants normalize DO (`invariants.rs:86-90`). So record clamps as a
**separate signed-debt diagnostic** (a `BudgetMetric`/tracing counter `clamp:do_floor_debt`
carrying the discarded negative magnitude), not a conserved-ledger entry. A4 asserts the
diagnostic fires with the right magnitude.

### Tests (`crates/tank_core/tests/conservation_guard.rs`)
- `guard_trips_on_injected_{n,c,o}_leak` (A3). (The P injection test lands with W1b.)
- `do_floor_records_debt_diagnostic` — force DO negative; assert `clamp:do_floor_debt`
  magnitude; assert it does **not** appear as a conserved-ledger delta (A4).

---

## W6a — Carbonate boundary residual DIAGNOSTIC (behavior-neutral)

### Corrected scope (review fix)
The clamp path deliberately preserves DIC by recomputing species from `(DIC, clamped pH)`
(`chemistry.rs:431-437`); tests assert DIC-preserving species at the clamp
(`tests/chemistry.rs:285-347`) and the contract uses simplified alkalinity `HCO3 + 2·CO3`
omitting `OH−H` (`carbonate_state_contract.md:63-70`). Reprojecting from alkalinity would
fabricate non-DIC species and break both — that is a behavior change, so it moves to **W6b**.

W6a is behavior-neutral: **surface** the boundary inconsistency instead of hiding it. When the
pH clamp engages, compute the implied alkalinity from the cached species and emit the
difference vs `water.alkalinity_meq_total` as an explicit diagnostic
(`BudgetMetric` `carbonate:boundary_alk_residual_meq`). No species/DIC/pH values change.
**Emit from a single site (review fix):** `resolve_carbonate_state`/`solve_carbonate_equilibrium`
is called from many paths (chemistry, water change, daily plant/algae, daily resolve, shrimp,
save/load). Emit the residual only from the budgeted hourly-chemistry resolve stage, not from
every call site, to avoid duplicate/ambiguous diagnostics.

### Tests
- `carbonate_boundary_residual_reported` — force a clamp; assert the residual diagnostic
  equals `implied_alk(cached species) − tracked_alk` and that DIC/species are unchanged vs
  today (A6, behavior-neutral form).

---

## Phase A gate
After W0→W1a→W2→W6a: `cargo test && cargo clippy -- -D warnings && cargo fmt --check` green;
`cargo run -p tank_harness --bin calibration_report` shows unchanged status/envelopes with
observed values within the A7 FP tolerance (not byte-for-byte); new-code save→load→replay
bit-identical (A8). Do not start Phase B until this holds.

---

## Phase B (behavior; each lands alone + re-calibrates)

### W1b — Track, route, close, and enforce phosphorus (owns all P beyond plumbing)
This item does everything W1a deliberately deferred. Introduce P composition constants
(`PLANT_P_MG_PER_G_BIOMASS = 4.0`, `ALGAE_P_MG_PER_G_BIOMASS = 5.0`, shrimp
`body_phosphorus_mg_per_g_wet_mass` new param) and populate `OrganicMass.p_mg` on every organic
route: **feed input**, plant/algae turnover, consumer feces/reserve/excretion, microbial decay,
and decomposition. **Feed-P is load-bearing (review fix):** W0 builds the feed `OrganicMass`
with `p=0`, and today feed phosphate is created by the `don × FEED_P_TO_N_MASS_RATIO` shortcut
(`nitrogen_cycle.rs:145`). W1b must give feed a real P content (feed's organic P per gram) so
that when the shortcut is removed, feed-derived P enters the organic P pool and mineralizes
from tracked detritus P instead of vanishing — otherwise P is silently deleted (or, if the
shortcut is kept, B1 never closes). Add **derived biomass P and organic-pool P to
`total_phosphorus_mg`** (now water + substrate + biomass + organic). **Unify** detritus-P
release with the phosphate mechanism (`nitrogen_cycle.rs:141-149`) so P mineralization is
driven by tracked detritus P, replacing the N-derived shortcut. **Extend the guard to phosphorus**
(`phosphorus_guard_delta_mg = total P net − open-action P flux`, mirroring
`carbon_guard_delta_mg`) and add the `guard_trips_on_injected_p_leak` test. Add microbial-
biomass P (currently N/C only in the budget, `budget.rs:335-356,447-468`). Ripple shrimp
`body_phosphorus` into presets (`tank_data/src/presets.rs:529-534,770-778,1275-1287,
1505-1512`) and scenario materialization (`tank_scenarios/src/lib.rs:772-777`), which currently
expose N/C only. Optionally add `water.dissolved_organic_phosphorus_mg_p_total` if intermediate
DOP is needed. A per-route N/C/P matrix test now closes P independently (B1). Re-run
calibration; phosphate dynamics shift — document envelope deltas. Tests:
`phosphorus_route_matrix_closes` (per-route N/C/P independent closure), `guard_enforces_p`
(injected P leak trips the guard).

### W3b — Realistic macrophyte C:N
Lower `PLANT_N_TO_C_RATIO` from 0.16 to a literature macrophyte value (molar C:N 15–30 → N:C
mass ratio ≈ 0.039–0.078; with N=28 mg/g this is C≈360–720 mg/g; pick a `param_meta`-cited mid
value). This is the deliberate carbon-moving change the W0 ratio-constant seam exists for. Re-run
calibration; document deltas; update `PROVENANCE_STATUS.md`.

### W4 — Hypoxia response
In `shrimp.rs:408-410` and `microfauna.rs:153-155`, stop crediting the un-respired
`(target_respired − respired)` C/N to reserve; instead reduce assimilation by the O2-limited
fraction and route the unassimilated food to feces/`fine_detritus` (W0 `OrganicMass`),
conserving mass. Net: low DO lowers reserve and growth. Test `hypoxia_depresses_growth`.

### W5 — Reproduction aggregation
Add `fn combine_stressors(&[f64]) -> f64` (Liebig minimum or geometric mean); replace the raw
products at `shrimp.rs:455-463` (condition), `:940-948` (hatch), `:1586-1592` (readiness).
Count adult condition once (keep in readiness; drop from hatch and clutch, or a single
condition gate). Re-tune coefficients to hold envelopes; document. Tests:
`reproduction_plausible_under_optimal` + update existing.

### W6b — Alkalinity-consistent boundary + raise clamp
Implement `carbonate_species_for_alkalinity_ph(alk, ph, ka1, ka2)` and reproject cached
species from alkalinity at the clamp (deliberately updating `tests/chemistry.rs:285-347` and
`carbonate_state_contract.md`); raise `CARBONATE_PH_MAX` (`chemistry.rs:111`) from 8.5 to ~9.5;
add solver-stability tests at pH 9.0–9.5 across 15–35 °C. Re-run calibration (B5 pH>8.5).

### W7 — Documentation refresh
Update `PROVENANCE_STATUS.md`, `scientific_specs.md`, `README.md` (phosphate/potassium already
tracked; live DIC rates; N/C/O/P conservation guarantee; per-guild stoichiometry). Regenerate
the calibration report.

---

## Cross-cutting checklist (every PR)
- `cargo test && cargo clippy -- -D warnings && cargo fmt --check`.
- No wall clock, no unseeded RNG (C2).
- New/changed parameters carry `param_meta` (confidence + source).
- Phase A PRs assert A7 (no envelope change); Phase B PRs attach before/after calibration and
  justify each envelope delta.
