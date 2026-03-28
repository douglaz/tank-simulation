# Shrimp State Contract

This note is the implementation contract for `tanksim-6e5.6.1`. It fixes the shrimp state model for the stage-structured cohort approach so downstream implementation beads (especially `tanksim-6e5.6.2`) can code against a stable contract without reopening the state-model debate.

## Scope

- Target species: *Neocaridina davidi* (cherry shrimp), the only shrimp species currently modeled.
- Goal: restructure the flat `AnimalState` into a stage-structured cohort model with explicit reserve/condition, life stages, molt state, and reproduction state — while keeping the population-level (not individual-based) abstraction.
- Non-goal: individual-based simulation. This contract describes a stage-structured population model in the Leslie/Lefkovitch matrix family, where aggregate metrics are tracked per stage but individual shrimp are not.

## Chosen State Model

### Why stage-structured cohorts

The current `AnimalState` tracks `adults_count`, `juveniles_count`, and `berried_females_count` as flat fields with population-wide condition indices. This is adequate for phase-1 reproduction and mortality, but it cannot express:

- differential juvenile vs adult mortality under stress,
- stage-dependent feeding rates and reserve allocation,
- a sub-adult transition stage (needed for realistic maturation timing),
- per-stage molt readiness and failure rates.

A stage-structured cohort model solves these without the complexity of individual tracking. It is the same level of abstraction used in fisheries stock assessment (Leslie matrix, Lefkovitch matrix) and is well suited to the hourly discrete-time simulation this engine uses.

### Stage definitions

| Stage | Name | Description | Can reproduce? | Typical duration (*N. davidi* at 24 C) |
| --- | --- | --- | --- | --- |
| 0 | Juvenile | Post-hatch to first visible saddle or size threshold | No | ~30-45 days |
| 1 | Sub-adult | Visible saddle / approaching sexual maturity | No | ~15-30 days |
| 2 | Adult | Sexually mature, capable of molting into breeding condition | Yes | Indefinite |

Berried females remain a tracked subset of adults, as they are today. They are not a separate stage; `berried_females_count` stays as subset bookkeeping within the adult stage.

### Why three stages instead of two

The current juvenile-to-adult jump conflates size-dependent vulnerability with reproductive maturity. In reality, *Neocaridina* juveniles become visually distinguishable sub-adults (saddled females, colored-up males) well before their first breeding molt. The sub-adult stage:

- allows a separate maturation accumulator for each transition (juvenile -> sub-adult, sub-adult -> adult),
- gives the simulation a window where shrimp are past the fragile juvenile phase but not yet breeding,
- matches the observable life history that aquarists recognize.

## Per-Stage State Fields

### Stage cohort structure

Each of the three stages carries its own state block. The new struct layout replaces the current flat `AnimalState` fields:

| Field | Unit | Per-stage? | Description |
| --- | --- | --- | --- |
| `count` | integer | Yes | Number of shrimp in this stage |
| `reserve_g` | g organic matter | Yes | Assimilated organic reserve for this stage cohort |
| `condition_index` | 0..1 | Yes | Body condition derived from feeding, temperature, water quality |
| `maturation_accum` | fractional days | Yes (juvenile, sub-adult only) | Fractional accumulator toward next stage transition |

### Population-wide fields (not per-stage)

| Field | Unit | Description |
| --- | --- | --- |
| `berried_females_count` | integer | Subset of adult count; bookkeeping only, not extra biomass |
| `egg_cohorts` | `Vec<EggCohort>` | Canonical clutch tracking, unchanged from current model |
| `egg_progress_days` | days | Display mirror of most advanced cohort, unchanged |
| `molt_readiness` | 0..1 | Population-wide molt cycle readiness (see Molt State below) |
| `molt_stress_index` | 0..1 | Population-wide molt stress from GH, instability, condition |
| `reproductive_readiness_index` | 0..1 | Population-wide readiness to spawn, driven by temperature, condition, stability |
| `inter_molt_timer_days` | days | Days since last population-wide molt event |
| `last_molt_success` | bool | Whether the most recent molt cycle succeeded or failed |
| `failed_molt_accum` | 0..1 | Accumulated penalty from consecutive failed molts |
| `daily_food_consumed_g` | g/day | Daily feeding record, unchanged |
| Hourly stress accumulators | stress score | `hourly_nh3_stress_accum`, `hourly_nitrite_stress_accum`, `hourly_low_do_stress_accum`, `hourly_heat_stress_accum`, `hourly_instability_stress_accum` — unchanged |

### Rationale for per-stage vs population-wide split

**Per-stage reserve and condition**: juveniles should not fund adult reproduction, and adults should not starve because juveniles consumed the shared reserve pool. Splitting reserve per stage ensures that feeding, growth, and mortality draw from the correct pool. This is the most important change from the current flat model.

**Population-wide molt and reproduction indices**: molt cycle timing and reproductive readiness are driven primarily by water chemistry (GH, temperature, stability) which affects all stages equally. Tracking these per-stage would add complexity without meaningful behavioral difference in the current model. If future work needs stage-specific molt intervals (e.g., juveniles molt more frequently), these can be promoted to per-stage fields without changing the overall contract.

## Reserve and Condition

### Reserve pool

Each stage's `reserve_g` is the retained share of assimilated intake, as defined in [ROUTING.md](ROUTING.md). The consumer routing contract is unchanged:

```
ingested = retained + feces + dissolved_excretion + respiration_routed_share
```

The change is that `retained` now routes to the specific stage's `reserve_g` based on which stage consumed the food.

### Per-stage food allocation

Daily food intake is allocated across stages proportional to biomass demand:

- Juvenile weight: `0.05 g` (current constant)
- Sub-adult weight: `0.08 g` (new, interpolated between juvenile and adult)
- Adult weight: `0.12 g` (current constant)
- Feeding allocation per stage: `stage_count * stage_weight / total_population_weight`

The juvenile feeding weight of `0.3` relative to adults (current `shrimp.rs` convention) is preserved; sub-adults get `0.67` relative weight.

### Condition index

Each stage's `condition_index` is computed from the same factors as today (food satiation, DO, NH3, nitrite, temperature, GH, instability) but applied to the stage's own reserve level. A stage with depleted reserve will have lower condition regardless of population-wide feeding success.

Condition drives:
- mortality rate (lower condition = higher mortality)
- maturation rate (low condition slows stage transitions)
- reproductive readiness (adults only; low condition reduces spawning probability)

## Life Stage Transitions

### Transition criteria

| Transition | Accumulator | Days required (base) | Condition threshold | Additional requirements |
| --- | --- | --- | --- | --- |
| Juvenile -> Sub-adult | `juvenile.maturation_accum` | 30 days at 24 C | `condition_index >= 0.3` | None |
| Sub-adult -> Adult | `sub_adult.maturation_accum` | 20 days at 24 C | `condition_index >= 0.4` | `last_molt_success == true` |

### Temperature scaling

Maturation rate scales with temperature using the existing `temp_condition_factor()` curve. At optimal temperature (22-26 C), maturation proceeds at base rate. Below 18 C or above 30 C, maturation slows significantly. This is consistent with the known temperature dependence of crustacean development.

### Maturation accumulator mechanics

Each simulation day, each stage accumulates:

```
daily_maturation = temp_condition_factor * condition_factor * base_rate
```

where `base_rate = 1.0 / required_days` and `condition_factor` is `1.0` when condition is above threshold, tapering to `0.0` when condition falls to zero.

When the accumulator reaches `1.0`, the fractional part carries over (as in the current `maturation_accum` design) to prevent systematic rounding loss in small populations.

### Transition thresholds as configurable parameters

The base durations (30 days, 20 days) and condition thresholds (0.3, 0.4) should be added to `ShrimpRuntimeParams` so they can be tuned per species or scenario without code changes:

| New parameter | Default | Description |
| --- | --- | --- |
| `juvenile_to_subadult_days` | 30 | Base days for juvenile -> sub-adult at optimal conditions |
| `subadult_to_adult_days` | 20 | Base days for sub-adult -> adult at optimal conditions |
| `juvenile_maturation_condition_threshold` | 0.3 | Minimum condition for juvenile maturation to proceed |
| `subadult_maturation_condition_threshold` | 0.4 | Minimum condition for sub-adult maturation to proceed |
| `sub_adult_weight_g` | 0.08 | Body mass for sub-adult stage |

The existing `shrimp_juvenile_maturation_days` in `ProcessParams` becomes the sum of both transition periods for backward compatibility during migration.

## Molt State

### Explicit molt cycle

The current model has `molt_stress_index` but no explicit molt timer. The new contract adds:

| Field | Unit | Description |
| --- | --- | --- |
| `inter_molt_timer_days` | days | Days elapsed since the last population-wide molt cycle |
| `molt_readiness` | 0..1 | Derived readiness combining timer, GH, condition, and reserve |
| `last_molt_success` | bool | Whether the previous molt succeeded |
| `failed_molt_accum` | 0..1 | Penalty accumulator; consecutive failures increase mortality risk |

### Molt cycle period

*Neocaridina davidi* molts approximately every 4-6 weeks under good conditions, faster when young and well-fed, slower under stress. The base inter-molt interval is configured via `ShrimpRuntimeParams`:

| New parameter | Default | Description |
| --- | --- | --- |
| `base_molt_interval_days` | 28 | Base inter-molt period at optimal conditions |
| `molt_gh_min_d` | 5.0 | Minimum GH for successful molt (reuses existing `gh_min_d`) |
| `failed_molt_mortality_scale` | 0.15 | Additional daily mortality fraction per unit of `failed_molt_accum` |

### Molt readiness derivation

```
timer_factor = clamp(inter_molt_timer_days / base_molt_interval_days, 0, 1)
mineral_factor = gh_mineral_factor(gh_d)  // existing function
condition_factor = average_population_condition
molt_readiness = timer_factor * mineral_factor * condition_factor
```

When `molt_readiness >= 0.8`, the population enters a molt event:
- Success if `mineral_factor >= 0.5` and `condition_factor >= 0.3`: timer resets, `last_molt_success = true`, `failed_molt_accum` decays by `0.5`.
- Failure otherwise: timer resets, `last_molt_success = false`, `failed_molt_accum += 0.3` (clamped to 1.0).

### Molt stress continuity

The existing `molt_stress_index` is retained as the population-wide summary of molt-related stress visible to other systems (mortality, condition). It is now derived from `failed_molt_accum` and current mineral/condition factors rather than being independently integrated. This keeps the snapshot/API surface stable while grounding the index in explicit state.

## Reproduction

### Egg-carrying state

Berried females remain a tracked subset of adults. The reproduction flow is unchanged in structure:

1. Eligible adults (non-berried females, approximated as `(adults_count - berried_females_count) / 2`) attempt spawning based on `reproductive_readiness_index`.
2. Successful spawning creates new `EggCohort` entries and increments `berried_females_count`.
3. Egg cohorts advance `progress_days` each day.
4. Mature cohorts (>= `egg_duration_days`) resolve: hatching produces juveniles, berried count decrements.

### Clutch size

Current model: fixed 25 `juveniles_per_clutch`.

New contract: base clutch size with condition modifier.

```
effective_clutch_size = base_clutch_size * clutch_condition_modifier(adult_condition)
```

where `clutch_condition_modifier` is:
- `1.0` when `adult.condition_index >= 0.7`
- linear taper to `0.5` when `adult.condition_index == 0.3`
- `0.0` below `0.3` (spawning should not occur at very low condition, but this is a safety floor)

| New parameter | Default | Description |
| --- | --- | --- |
| `base_clutch_size` | 25 | Base juveniles per clutch at good condition |
| `min_clutch_condition` | 0.3 | Condition below which clutch size is zero |

### Hatch success

Hatch success factors are unchanged from the current model: base rate modified by temperature, DO, mineral stress, and instability. The hatched juveniles enter the juvenile stage cohort with initial reserve proportional to egg reserve (funded from adult reserve at spawning time, as currently implemented).

### Incubation progress

`EggCohort` is unchanged. `egg_progress_days` remains a display mirror. The clutch-resolution invariant (only fully matured cohorts resolve) is preserved.

## Population vs Individual Boundary

This is a **stage-structured population model**, not an individual-based model. The key design decisions:

| Aspect | This contract | Why |
| --- | --- | --- |
| Individual identity | Not tracked | Unnecessary for the behaviors the sim expresses (molt success, breeding, juvenile survival, chemistry stress) |
| Sex ratio | Implicit 50:50 | Adequate for population-level spawning probability; explicit sex ratio would require individual assignment |
| Size within stage | Not tracked | Each stage uses a fixed representative body mass; size variation within a stage is averaged out |
| Genetic variation | Not tracked | No heritable traits in the current model |
| Spatial position | Not tracked | Well-mixed tank assumption; no habitat selection by individuals |

This level of aggregation was chosen because it supports the planned mechanics (stage-dependent mortality, condition-dependent reproduction, molt cycle, and chemistry stress) while remaining tractable for hourly discrete-time stepping, parameter tuning, and deterministic save/load. The tradeoff is that within-stage variation is invisible, which means phenomena like selective mortality on the weakest individuals within a stage cannot emerge. That is acceptable for an aquarium simulator focused on husbandry outcomes rather than evolutionary dynamics.

## Intentional Omissions

The following are explicitly deferred beyond this contract:

| Omission | Reason for deferral | When it might matter |
| --- | --- | --- |
| Sex ratio tracking | Adds individual-assignment complexity with minimal husbandry-outcome impact for *Neocaridina* | If modeling species where male aggression or sex-ratio-dependent spawning is important |
| Individual identity | The stage-structured model is sufficient for all planned mechanics | If modeling individual genetic coloration or selective breeding |
| Size-within-stage variation | Fixed per-stage body mass is adequate; size distributions add complexity without clear simulation benefit | If modeling size-selective predation or competition |
| Predation | No predators in the current model | If adding fish or other predator species |
| Disease | No disease mechanics planned | If modeling bacterial or parasitic infections |
| Mineral budget for molting | Molt success currently depends on GH (mineral availability in water) but does not explicitly consume calcium/magnesium from the water column | When the carbonate/mineral phase adds explicit Ca/Mg budgets; this is an extension point, not a prerequisite |
| Chloride modulation of nitrite toxicity | `shrimp.rs` reads nitrite directly with no chloride modifier, even though chloride is tracked | Future shrimp toxicology phase |
| Density-dependent effects beyond feeding competition | No explicit crowding stress on shrimp | If modeling territorial behavior or waste-load feedback |

## Extension Points

These are interfaces the contract exposes for future work without requiring a state-model rework:

1. **Habitat-aware grazing**: the per-stage feeding allocation can be modified to prefer specific habitat surfaces (e.g., juveniles prefer biofilm on fine substrate) without changing the state layout.

2. **Mineral budget for molting**: when the carbonate/mineral phase adds explicit Ca/Mg consumption, the molt event can deduct minerals from the water column proportional to population biomass. The `molt_readiness` derivation already includes `gh_mineral_factor` as a readiness gate.

3. **Toxicity modifiers**: the hourly stress accumulators already separate NH3, nitrite, low DO, heat, and instability. Adding chloride-modulated nitrite toxicity or heavy-metal stress requires only new accumulator terms, not a state-model change.

4. **Per-stage mortality curves**: the current contract uses population-wide stress accumulators with a `juvenile_sensitivity` multiplier. Promoting stress accumulators to per-stage fields would enable stage-specific dose-response curves without restructuring.

5. **Variable clutch size from body size**: if size-within-stage tracking is added later, `effective_clutch_size` can incorporate body mass as a factor alongside condition.

## Migration Strategy

### Save-file compatibility

The current `AnimalState` is a flat struct with `#[serde(default)]` on newer fields. The stage-structured model changes the shape significantly (nested per-stage substruct), so simple `#[serde(default)]` may not suffice for all fields.

Recommended migration approach:

1. **Deserialize old saves into the current flat struct** using a versioned `AnimalStateV1` type alias or a `#[serde(untagged)]` enum.
2. **Convert at load time**: distribute `adults_count` into the adult stage, `juveniles_count` into the juvenile stage, zero sub-adults, and split `reserve_g` proportionally by biomass (`adult_reserve = reserve_g * adult_biomass / total_biomass`).
3. **New fields get sensible defaults**: `inter_molt_timer_days = 14` (mid-cycle), `last_molt_success = true`, `failed_molt_accum = 0.0`, `molt_readiness = 0.5`.
4. **One-way migration**: once loaded and re-saved, the new format is used. No backward compatibility to the flat format is required.

This follows the same pattern as the `egg_cohorts` migration, where old saves without cohort data are upgraded at runtime with serde defaults.

### Struct layout sketch

```rust
pub struct StageCohort {
    pub count: u32,
    pub reserve_g: f64,
    pub condition_index: f64,
    #[serde(default)]
    pub maturation_accum: f64,
}

pub struct AnimalState {
    pub juvenile: StageCohort,
    pub sub_adult: StageCohort,
    pub adult: StageCohort,

    // Subset bookkeeping (not extra biomass)
    pub berried_females_count: u32,

    // Reproduction
    pub egg_cohorts: Vec<EggCohort>,
    pub egg_progress_days: f64,
    pub reproductive_readiness_index: f64,

    // Molt state
    pub molt_stress_index: f64,
    pub molt_readiness: f64,
    pub inter_molt_timer_days: f64,
    pub last_molt_success: bool,
    pub failed_molt_accum: f64,

    // Feeding
    pub daily_food_consumed_g: f64,

    // Hourly stress accumulators (unchanged)
    pub hourly_nh3_stress_accum: f64,
    pub hourly_nitrite_stress_accum: f64,
    pub hourly_low_do_stress_accum: f64,
    pub hourly_heat_stress_accum: f64,
    pub hourly_instability_stress_accum: f64,
}
```

Accessors for backward compatibility with existing code:

```rust
impl AnimalState {
    pub fn adults_count(&self) -> u32 { self.adult.count }
    pub fn juveniles_count(&self) -> u32 { self.juvenile.count }
    pub fn total_count(&self) -> u32 {
        self.juvenile.count + self.sub_adult.count + self.adult.count
    }
    pub fn condition_index(&self) -> f64 {
        // Population-weighted average condition
    }
    pub fn reserve_g(&self) -> f64 {
        self.juvenile.reserve_g + self.sub_adult.reserve_g + self.adult.reserve_g
    }
}
```

## Downstream Bead Map

| Bead | What it consumes from this contract |
| --- | --- |
| `tanksim-6e5.6.2` (stage-structured population dynamics) | Full state layout, transition criteria, per-stage reserve, molt cycle, condition-dependent clutch size |
| `tanksim-6e5.3.2` (shrimp ingestion loop) | Per-stage food allocation and reserve routing |
| `tanksim-6e5.3.7` (death routing) | Per-stage mortality with reserve-to-detritus routing, `failed_molt_accum` mortality modifier |
| `F-phase` beads (shrimp life history) | Extension points for mineral budget, toxicity, habitat-aware grazing |
| `A2 / tanksim-6e5.1.2` (save schema) | Migration strategy, struct layout, versioned deserialization |

## Tests Affected By This Contract

Based on the test dependency map in [scientific_inventory.md](scientific_inventory.md):

| Test file | Impact | Required changes |
| --- | --- | --- |
| `shrimp_population.rs` | Direct: population counts, berried events, egg hatching, mortality, removal | Update to use stage accessors; spawning and hatching assertions stay envelope-based; validation/removal tests need updated field access |
| `thermal_reproduction_penalty.rs` | Direct: reproductive readiness comparison at 25 C vs 30 C | Update field access; the 20% readiness gap assertion should remain as an envelope check |
| `determinism.rs` | Indirect: snapshot JSON and state equality will change | Update expected snapshot fields; add sub-adult count to expected output |
| `end_to_end.rs` | Indirect: story outcomes may shift due to sub-adult stage delay | Loosen timing assertions if needed; population viability outcomes should be preserved |
| `save_load.rs` | Direct: save roundtrip schema changes | Add migration test for V1 -> V2 format; keep roundtrip exact for new format |
| `substrate_filter_integration.rs` | Indirect: shrimp condition and grazing surface assertions | Minor: update condition field access |

No test changes are made by this design spike. The table above is a map for the implementation beads.

## Verification Expectations

Implementation beads consuming this contract should test:

1. **Stage transition**: juveniles accumulate and promote to sub-adults, sub-adults to adults, with correct carry-over of fractional accumulators.
2. **Per-stage reserve isolation**: feeding a population where only adults have food access should not increase juvenile reserve.
3. **Molt cycle**: timer advances, readiness triggers molt events, failed molts accumulate penalty, successful molts reset.
4. **Condition-dependent clutch size**: well-conditioned adults produce larger clutches than stressed adults.
5. **Migration**: loading a V1 (flat) save produces a valid V2 (staged) state with no assertion failures.
6. **Conservation**: total shrimp biomass (sum of all stage counts * weights + reserves) is accounted for across feeding, transitions, reproduction, and mortality, consistent with [ROUTING.md](ROUTING.md).
