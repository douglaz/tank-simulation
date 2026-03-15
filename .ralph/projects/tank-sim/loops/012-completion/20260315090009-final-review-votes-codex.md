---
artifact: final-review-votes
loop: 12
project: tank-sim
backend: codex
role: final_reviewer
created_at: 2026-03-15T09:00:09Z
---

# Vote Results

## Amendment: TS-REV-001

### Vote
ACCEPT

### Rationale
`apply_action` already has access to `source_water_catalog`, so accepting a water-change action with a nonexistent or invalid source profile is avoidable. Deferring that failure until `step_hours` makes the queue report success for invalid input and can later block simulation. Keeping step-time validation as a guard for restored or externally constructed queues is still sensible, but normal API behavior should reject the bad action immediately.

## Amendment: TS-REV-002

### Vote
ACCEPT

### Rationale
The cited flow spends alkalinity during ammonia oxidation, then uses alkalinity again to limit nitrite oxidation, and finally subtracts alkalinity using a total that includes `nob_rate`. That is internally inconsistent for a single alkalinity-per-nitrified-nitrogen parameter and will over-deplete KH under low-alkalinity conditions. The amendment fixes a real stoichiometry error.

## Amendment: TS-REV-003

### Vote
ACCEPT

### Rationale
Respiration and photosynthesis should move carbon and therefore pH through DIC, but they should not independently create or destroy alkalinity. If `step_hourly_chemistry` changes alkalinity directly from the DIC swing, the model invents KH drift without a matching buffering process. Leaving alkalinity unchanged there is the more coherent behavior.

## Amendment: TS-REV-004

### Vote
ACCEPT

### Rationale
A maintenance mechanic that never emerges in normal runtime is effectively dead code. If `cleanliness_index` starts at `1.0`, cleaning resets it to `1.0`, and no production path lowers it, then the clogging formula gated by `(1.0 - cleanliness_index)` never activates except in seeded tests. The amendment addresses a real simulation-state bug.

## Amendment: TS-REV-005

### Vote
ACCEPT

### Rationale
A water change should exchange dissolved oxygen along with the other dissolved pools. Leaving `dissolved_oxygen_mg_total` untouched means even a 100% change preserves pre-change oxygen mass, which is inconsistent with source-water initialization and basic mass-balance expectations. Mixing in source-water oxygen is the correct fix.
