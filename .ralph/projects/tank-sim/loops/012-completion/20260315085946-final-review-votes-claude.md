---
artifact: final-review-votes
loop: 12
project: tank-sim
backend: claude
role: final_reviewer
created_at: 2026-03-15T08:59:46Z
---

# Vote Results

## Amendment: TS-REV-001

### Vote
ACCEPT

### Rationale
The code confirms the asymmetry: `apply_action` at `engine.rs:243-265` already performs state-aware validation for `RemoveShrimp` (checking available count against queued actions), but `WaterChangePercent` only gets syntactic validation via `action.validate()` at `actions.rs:86-96` (percent range, non-empty string). Source profile existence is deferred to `validate_water_changes` at `engine.rs:47`, which runs inside `step_one_hour`. This means a bogus profile silently enters the queue and only errors on the next `step_hours` call — an inconsistent API surface. The fix is straightforward: check `state.source_water_catalog` at enqueue time, matching the `RemoveShrimp` precedent. Retaining step-time validation as a defensive backstop for deserialized queues is also correct.

## Amendment: TS-REV-002

### Vote
ACCEPT

### Rationale
Tracing the alkalinity flow confirms the double-charge:

1. **Budget limiting**: AOB is capped by `alk_budget / alk_per_mg_n` (line 152), budget debited (line 160-162). Comammox similarly capped and debited (lines 186-196). NOB is *also* capped by remaining `alk_budget / alk_per_mg_n` (line 228-232).
2. **State subtraction**: Line 271 subtracts `(aob_rate + comammox_rate + nob_rate) * alk_per_mg_n` from actual state.

In real aquarium chemistry, the alkalinity cost of nitrification (~7.14 mg CaCO3/mg N, or the `alkalinity_meq_per_mg_n_nitrified` constant) is entirely consumed during ammonia oxidation (AOB step). The NOB step (NO2- → NO3-) consumes O2 but does *not* consume additional alkalinity. Including `nob_rate` in both the budget limiter and the final subtraction is stoichiometrically wrong — it artificially stalls nitrite oxidation under low-KH conditions and over-depletes alkalinity. The parameter name itself ("per mg N nitrified") implies the full-pathway cost, which should be charged once when ammonia is oxidized, not again when nitrite is oxidized.

## Amendment: TS-REV-003

### Vote
ACCEPT

### Rationale
At `chemistry.rs:52-54`, alkalinity is modified by `net_dic_delta_mg * ALKALINITY_FEEDBACK_MEQ_PER_MG_C` during every hourly chemistry tick. The spec at lines 758-761 clearly separates the alkalinity drivers (nitrification, water changes) from DIC drivers (photosynthesis, respiration, degassing). The `compute_ph_from_totals` function at `chemistry.rs:7-21` already properly derives pH from both alkalinity and DIC independently — changing DIC alone will shift pH through that formula without needing to also mutate alkalinity. The current feedback term means that a dark period (net respiration → DIC increase) spuriously *destroys* alkalinity, and a light period spuriously *creates* it, which has no real-world basis and introduces phantom KH drift unrelated to the spec's intended alkalinity sinks/sources.

## Amendment: TS-REV-004

### Vote
ACCEPT

### Rationale
The dead-state defect is unambiguous. `cleanliness_index` defaults to `1.0` (`hardware.rs:78`), `CleanFilter` resets it to `1.0` (`engine.rs:179`), and the daily clogging growth at `nitrogen_cycle.rs:376` multiplies by `(1.0 - cleanliness_index)` which is `0.0` — so `clogging_delta` is always zero in normal play. No code path between construction and cleaning ever decreases `cleanliness_index`. The only way to get a dirty filter is to manually seed one in test setup (as `substrate_filter_integration.rs:63` and `cycling.rs:353` do). This makes the entire filter-maintenance gameplay loop inert: players can never experience clogging emergence, and cleaning is a no-op since the filter is always clean. The proposed fix — decaying `cleanliness_index` from detritus load or driving clogging directly from detritus — is the minimal change needed to make this mechanic functional.

## Amendment: TS-REV-005

### Vote
ACCEPT

### Rationale
`apply_water_change` at `water_change.rs:45-77` explicitly mixes 15 dissolved pools (ammonia, nitrite, nitrate, phosphate, DIC, DOC, DON, alkalinity, Ca, Mg, Na, K, bicarbonate, chloride, sulfate) plus temperature, but `dissolved_oxygen_mg_total` is conspicuously absent from both the removal and replacement sections. Meanwhile, `WaterState::from_source_profile` at `water.rs:67` derives initial DO from saturation at source temperature (`do_sat_mg_l(profile.temperature_c)`). This means a 100% water change preserves the old oxygen mass entirely, while an initial fill would set it to saturation — a direct inconsistency. The spec's "removes dissolved and suspended constituents proportionally" language (line 1002) clearly includes dissolved oxygen. The fix is straightforward: apply `retention` to `dissolved_oxygen_mg_total` and add replacement DO from source-water saturation at the source temperature, reusing the existing `do_sat_mg_l` helper.
