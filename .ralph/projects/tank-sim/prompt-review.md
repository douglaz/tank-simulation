---
artifact: prompt-review
project: tank-sim
backend: codex(gpt-5.4-xhigh)
role: prompt_reviewer
created_at: 2026-03-15T02:47:53Z
---

I’m reviewing the spec for implementation ambiguity, then I’ll rewrite it into a tighter build-ready prompt with explicit scope, acceptance criteria, and deferred items.
I’ve isolated the main problems: the spec mixes MVP requirements with research-grade ambitions, and several “must” behaviors still lack concrete pass/fail thresholds. I’m tightening the rewrite around one implementable slice with explicit contracts for determinism, formulas, config shape, and regression tests.
The rewrite is narrowing the contract in three ways: one authoritative MVP scope, exact storage/step semantics, and numeric acceptance thresholds. I’m also converting free-form areas like events and provenance into smaller machine-testable requirements.
# Prompt Review

## Issues Found
- The MVP scope is overloaded. It combines a deterministic engine, TUI, scientific provenance, calibration, multiple ecological subsystems, and large content packs without a single enforced cut line, which makes delivery order ambiguous.
- Priority is unclear. The spec mixes `MUST`, `SHOULD`, `MAY`, “planned later,” and “non-negotiables” without one authoritative v1 list, so an implementation agent could spend time on stretch work before core loops are stable.
- The unit model is inconsistent. Some water values are totals, some are concentrations, and some are display metrics, which makes mass balance and save/load correctness easy to get wrong.
- Determinism is underspecified. “Bitwise-identical” results are hard to guarantee with floating point unless the scope is limited to the same binary/target/toolchain and the RNG/state contract is explicit.
- Several core systems are only directionally specified. pH approximation, DO saturation, plant/algae competition, microfauna effects, and shrimp reproduction/mortality still leave too much room for incompatible implementations.
- Action timing is unclear. The spec does not fully define when `apply_action` takes effect relative to the tick pipeline or how invalid actions should be handled.
- The event system is hard to test as written. Free-form `causes: Vec<String>` will produce inconsistent output unless machine-readable cause codes are also required.
- Acceptance tests are mostly qualitative. Phrases like “stress rises,” “outperform,” and “warming over time” need numeric assertions or relative thresholds.
- Parameter provenance is too broad for MVP. Requiring metadata on “every important parameter” adds large content overhead without defining the minimum viable granularity.
- Optional features are entangled with the public API. Top-off, fertilizer, seasonal schedules, denitrification, and richer plant sets expand the surface area before core chemistry and population loops are validated.
- Hidden versus observable state is described conceptually, but the snapshot contract, event retention policy, and UI data boundary are not strict enough.
- The spec lacks a concrete performance and scenario contract, so an implementation agent cannot tell how much engineering effort should go into optimization versus correctness.

## Refined Prompt
### Aquarium / Planted Tank Ecosystem Simulator (Rust) — Implementation Prompt v1

Build a deterministic Rust workspace that simulates a single freshwater planted shrimp tank as a causal ecosystem. The target species is `Neocaridina davidi`. The primary interface is a terminal UI. The MVP must be scientifically plausible, testable, and incremental to implement.

Optimize for a complete, working v1. Do not pursue research-grade completeness. Use simplified, guild-based models that preserve the main causal relationships.

#### 1. Deliverable

Deliver a Cargo workspace with these crates:

- `tank_core`: deterministic simulation engine, domain types, update systems, save/load, tests
- `tank_data`: TOML preset loading and validation
- `tank_tui`: terminal UI that consumes `TankSnapshot` and dispatches `PlayerAction`
- `tank_scenarios`: scenario fixtures used by regression tests and manual play

The MVP must allow a user to:

- choose a tank size and fill height
- choose substrate, source water, plants, filter, light, heater, and shrimp count
- run the simulation forward in hours/days/weeks
- feed the tank, change water, trim plants, clean the filter, siphon detritus, and change ambient temperature
- observe chemistry, plant status, algae/periphyton pressure, shrimp condition, reproduction, and event explanations
- save and load deterministically

#### 2. Scope

##### Mandatory v1 scope

Implement all of the following:

- one tank only
- one well-mixed water column compartment
- 1-hour fixed simulation tick
- layered substrate with 1 to 3 layers
- geometry-dependent volume, wall area, and surface area
- ambient temperature as a runtime player action
- water temperature dynamics with optional heater
- dissolved oxygen tracking
- nitrogen cycle: detritus -> mineralization -> TAN -> nitrite -> nitrate
- NH3 fraction derived from TAN, pH, and temperature
- minimal ions sufficient to derive GH, KH, TDS, and conductivity
- 2 plant guilds: `FastStem`, `RootFeedingRosette`
- 2 algae/biofilm pools: suspended algae and periphyton
- decomposer microbes plus nitrifier guilds: ammonia oxidizers, nitrite oxidizers, comammox
- one aggregate microfauna pool with ecological effects
- shrimp population with adults, juveniles, berried females, condition, molt stress, and reproduction readiness
- event log with machine-readable kinds and cause codes
- deterministic save/load
- terminal UI with overview, chemistry, biology, actions, and log screens

##### Stretch only after all mandatory tests pass

These may be added later, but are not required for v1 acceptance:

- seasonal ambient schedules
- fertilizer dosing
- top-off and evaporation
- floating plants
- denitrification
- richer plant packs
- more detailed carbonate chemistry

##### Out of scope for v1

Do not implement:

- fish
- snails
- disease-specific pathology
- saltwater chemistry
- CFD or spatial flow
- photorealistic graphics
- multiplayer
- online features

#### 3. Modeling stance

Use a mechanistic, guild-based model. Preserve these causal truths:

- tank size changes dilution, thermal inertia, gas exchange, and stability
- nitrification is biological and takes time to mature
- ammonia risk depends on pH and temperature, not TAN alone
- rising temperature lowers oxygen saturation
- rooted plants benefit from nutrient-rich substrate more than inert substrate
- plants and algae compete for light and nutrients
- periphyton is both a nuisance and a shrimp food resource
- overfeeding raises detritus, oxygen demand, TAN, and algae pressure
- `Neocaridina davidi` reproduction is worse at sustained high temperature

Use heuristics anchored by the supplied references. No new literature search is required for v1.

#### 4. Determinism and numeric contract

- Use a fixed 1-hour step.
- Use `f64` internally for all continuous values.
- Use an explicit RNG stored in state, for example `ChaCha8Rng`.
- Never call thread-local RNG inside `tank_core`.
- Determinism is required for the same seed, action schedule, binary, target triple, and toolchain.
- Cross-platform bitwise identity is not required.
- Save files must include full state, queued actions, RNG state, app version, and schema version.
- Invalid actions must return a typed validation error. Do not silently clamp user inputs except for tiny floating-point cleanup.
- Enforce invariants after every update:
  - no negative mass pools
  - no negative populations
  - `0.0 <= index <= 1.0` for health/maturity/stress-style values
  - temperature stays above `0.0 C` unless a scenario explicitly allows freezing

#### 5. Units and state rules

Use these conventions consistently:

- store dissolved and particulate substances as total tank amounts, not concentrations
- use `_mg_total`, `_g_total`, `_meq_total`, `_count`, `_c`, `_l`, `_cm`, `_m2`
- derive all `mg/L` values from total amount divided by current water volume
- treat TDS and conductivity as derived display metrics, not hidden source-of-truth state

Minimum tracked dissolved pools:

- TAN as `ammonia_total_mg_n_total`
- `nitrite_mg_n_total`
- `nitrate_mg_n_total`
- `phosphate_mg_p_total`
- `dissolved_oxygen_mg_total`
- `dissolved_inorganic_carbon_mg_c_total`
- `dissolved_organic_carbon_mg_c_total`
- `dissolved_organic_nitrogen_mg_n_total`
- `alkalinity_meq_total`
- ions: Ca, Mg, Na, K, HCO3, Cl, SO4 as total mg

Derived display values:

- `tan_mg_l`
- `nh3_mg_l`
- `nitrite_mg_l`
- `nitrate_mg_l`
- `phosphate_mg_l`
- `do_mg_l`
- `gh_d`
- `kh_d`
- `tds_mg_l`
- `conductivity_us_cm`
- `ph`

Use these display formulas:

- `gh_d = ((2.497 * ca_mg_l) + (4.118 * mg_mg_l)) / 17.848`
- `kh_d = (alkalinity_meq_l * 50.0) / 17.848`
- `tds_mg_l = total_tracked_ions_mg / volume_l`
- `conductivity_us_cm = tds_mg_l / 0.65`

#### 6. Core geometry model

Use:

```rust
pub struct TankGeometry {
    pub length_cm: f64,
    pub width_cm: f64,
    pub height_cm: f64,
    pub fill_height_cm: f64,
    pub glass_thickness_mm: f64,
    pub open_top: bool,
}
```

Derive exactly:

- `water_volume_l = length_cm * width_cm * fill_height_cm / 1000.0`
- `surface_area_cm2 = length_cm * width_cm`
- `footprint_area_cm2 = length_cm * width_cm`
- `wall_area_cm2 = 2.0 * fill_height_cm * (length_cm + width_cm)`
- `mean_depth_cm = fill_height_cm`
- `surface_area_to_volume_ratio = surface_area_cm2 / water_volume_l`
- `wall_area_to_volume_ratio = wall_area_cm2 / water_volume_l`

`open_top = false` must reduce surface heat/gas exchange by a configurable multiplier less than `1.0`. Default that multiplier to `0.25`.

#### 7. Time model

- One simulation tick equals one hour.
- Every 24 hourly ticks, run a daily update layer.
- Weekly or monthly bookkeeping can be simple counters updated from daily state.
- `apply_action` queues an action for the start of the next hourly tick.
- `step_hours(n)` processes exactly `n` hourly ticks in order.
- If multiple actions are queued for the same tick, process them in insertion order.

#### 8. Public API

These public types are required:

```rust
pub struct SimSeed(pub u64);

pub enum PlayerAction {
    Feed { grams: f64 },
    WaterChangePercent { percent: f64, source_profile_id: String },
    TrimPlants { fraction: f64 },
    SiphonDetritus { fraction: f64 },
    CleanFilter { intensity: f64 },
    AddShrimp { count: u32 },
    RemoveShrimp { count: u32 },
    ChangePhotoperiod { hours: f64 },
    ChangeLightIntensity { intensity_index: f64 },
    ChangeHeaterSetpoint { setpoint_c: f64 },
    ChangeAmbientTemperature { target_c: f64 },
    ChangeAeration { enabled: bool, intensity: f64 },
}

pub trait SimulationEngine {
    fn apply_action(&mut self, action: PlayerAction) -> Result<(), SimError>;
    fn step_hours(&mut self, hours: u32) -> Result<(), SimError>;
    fn snapshot(&self) -> TankSnapshot;
    fn full_state(&self) -> &TankState;
}
```

Validation rules:

- percentages and fractions must be in `0.0..=1.0` unless explicitly documented as percent values
- water change percent must be `0.0..=100.0`
- counts and grams must be non-negative
- `ChangeAmbientTemperature` is mandatory and must affect water temperature only through the heat model, not by instant water temperature assignment

#### 9. Required simulation systems

##### Temperature

Use this hourly discrete model:

- `dt_s = 3600.0`
- `heat_capacity_j_per_k = volume_l * 4186.0`
- `ua_total_w_per_k = (k_surface * surface_area_m2 * top_factor) + (k_wall * wall_area_m2)`
- `q_ambient_w = ua_total_w_per_k * (ambient_temp_c - water_temp_c)`
- `q_heater_w = max_watts * efficiency` only when heater is enabled and `water_temp_c < setpoint_c - deadband_c / 2.0`
- the heater must never cool the tank
- light heating may be ignored in v1

Then:

- `delta_temp_c = (q_ambient_w + q_heater_w) * dt_s / heat_capacity_j_per_k`
- `water_temp_c += delta_temp_c`

Required outcomes:

- smaller tanks respond faster than larger tanks
- higher ambient temperature lowers DO saturation
- if ambient exceeds the heater setpoint, heater power eventually goes to zero

##### Nitrogen cycle

Required pathway:

- feed -> particulate organics -> dissolved organics -> TAN -> nitrite -> nitrate

Implement:

- a particulate detritus pool
- a fine detritus pool
- dissolved organic carbon and nitrogen pools
- a decomposer biomass or capacity term
- nitrifier guild biomasses for ammonia oxidizers, nitrite oxidizers, and comammox

Use Monod-style bounded rates:

- `rate = vmax * biomass * f_temp * f_pH * f_do * substrate / (k_substrate + substrate)`

Rules:

- nitrification must be biomass-limited
- nitrification must be oxygen-limited
- nitrification must be slower in immature filters
- comammox may convert TAN directly to nitrate with a lower `vmax`
- rate clamps must prevent consuming more substrate than exists

Use `4.57 mg O2` demand per `1 mg N` nitrified to nitrate.

##### Ammonia speciation

Store TAN only. Derive NH3 each tick using:

- `pKa = 0.09018 + 2729.92 / (273.2 + temp_c)`
- `fraction_nh3 = 1.0 / (1.0 + 10.0_f64.powf(pKa - ph))`
- `nh3_mg_l = tan_mg_l * fraction_nh3`

NH3-driven stress must be stronger than TAN-driven stress.

##### Dissolved oxygen

Track dissolved oxygen explicitly.

Use:

- `do_sat_mg_l` from linear interpolation over this table:
  - `0 C -> 14.6`
  - `10 C -> 11.3`
  - `20 C -> 9.1`
  - `30 C -> 7.6`

Then update hourly:

- `reaeration = kLa * (do_sat_mg_l - do_mg_l)`
- `photosynthesis` from plants and periphyton during lit hours only
- `respiration` from shrimp, plants, microbes, algae, and decomposition
- `nitrification_o2_cost` from the nitrification step

Required outcomes:

- night oxygen dips are possible
- aeration increases gas exchange
- overfeeding can push DO down through decomposition demand

##### pH and alkalinity

Do not keep pH constant.

Use a simplified approximation for v1:

- nitrification consumes alkalinity
- photosynthesis lowers DIC during lit hours
- respiration and decomposition raise DIC
- water changes mix alkalinity and DIC from source water

Approximate pH from alkalinity and DIC each tick with a bounded monotonic function. Use this default formula unless replaced by a documented equivalent:

- `alkalinity_meq_l = alkalinity_meq_total / volume_l`
- `dic_mmol_l = (dissolved_inorganic_carbon_mg_c_total / 12.0) / volume_l`
- `ph = clamp(6.3 + log10(max(alkalinity_meq_l, 0.05)) - log10(max(dic_mmol_l, 0.02)), 5.5, 8.5)`

Alkalinity consumption from nitrification must be modeled. Use `0.1428 meq` alkalinity consumed per `1 mg N` nitrified.

##### Substrate

Support 1 to 3 layers. Required kinds:

- inert sand
- inert gravel
- active planted substrate
- coarse porous media mix

Each layer must affect:

- rooted nutrient access
- detritus trapping
- colonizable area
- low-oxygen tendency
- shrimp grazing surface proxy

Use a substrate nutrient store for rooted plants. Active substrate must start with higher nutrient stores and higher CEC proxy than inert substrate.

##### Plants

Mandatory plant guilds:

- `FastStem`
- `RootFeedingRosette`

Daily net plant growth must depend on:

- light
- temperature
- nitrogen availability
- phosphorus availability
- carbon availability proxy from DIC
- habitat factor
- crowding/self-shading

Use a bounded form such as:

- `net_growth = biomass * max_rate * f_light * f_temp * min(f_n, f_p, f_c) * habitat_factor * (1 - crowding) - respiration - senescence`

Rules:

- `FastStem` takes most nutrients from the water column
- `RootFeedingRosette` preferentially draws from substrate nutrient stores
- active substrate and adequate depth must improve `RootFeedingRosette` growth versus inert sand
- if plant biomass declines, a fraction must enter detritus

##### Algae and periphyton

Track:

- suspended algae biomass
- periphyton biomass

Daily algae growth must depend on:

- light
- temperature
- dissolved nutrients
- weak plant competition
- available colonizable area for periphyton

Periphyton must:

- provide shrimp food
- contribute to nuisance risk
- scale with surfaces and light
- support event generation when high

##### Microbes and microfauna

Use simplified hidden state:

- decomposer biomass or capacity
- nitrifier guild biomasses
- one aggregate microfauna grazing index

Microfauna effects in v1:

- modestly increase mineralization efficiency
- consume some periphyton/bacterial resource
- be reduced by strong shrimp grazing pressure

Do not model species-level microfauna.

##### Shrimp population

Use aggregate state for:

- adults
- juveniles
- berried females
- average condition `0..1`
- molt stress `0..1`
- reproductive readiness `0..1`
- egg progress in days

Daily condition must depend on:

- food and periphyton availability
- dissolved oxygen
- NH3
- nitrite
- temperature
- mineral status proxy from GH
- recent instability from large chemistry swings

Reproduction rules:

- use an eligible female proxy of `0.5 * adults`, minus current berried females
- `p_spawn = base_spawn_rate * f_temp * f_condition * f_stability * f_mineral`
- sample new berried females stochastically from eligible females
- egg development progresses daily
- hatch success depends on condition, oxygen, temperature, and stability

Temperature anchor for reproduction:

- best around `22 C` to `26 C`
- clearly worse by `30 C`
- near-zero by `33 C`

Molt stress must rise under:

- low GH
- abrupt water chemistry changes
- poor condition
- thermal stress

Mortality must be probabilistic and increase with:

- NH3
- severe nitrite
- low DO
- sustained heat stress
- severe molt stress
- starvation

#### 10. Update order

Use this stable hourly pipeline:

1. apply queued actions
2. update current water volume if a water change occurred
3. mix source water chemistry and temperature for water changes
4. update water temperature from ambient and heater
5. compute light on/off state for the current hour
6. process feed inputs and excretion
7. move detritus through decomposition and mineralization
8. update nitrification and nitrifier growth/decay
9. update DIC, alkalinity, and pH
10. update dissolved oxygen
11. update hourly plant and periphyton photosynthesis/respiration effects
12. update shrimp hourly stress accumulators
13. emit hourly events and warnings
14. refresh derived observables

After every 24 hourly ticks, run this daily pipeline:

1. net plant biomass change
2. net algae/periphyton biomass change
3. microfauna turnover
4. shrimp condition, molt stress, reproduction, hatching, and juvenile recruitment
5. mortality checks
6. biofilter maturity summary update
7. long-term stability metrics

#### 11. Events and observability

Use machine-readable events.

```rust
pub struct SimEvent {
    pub day: u32,
    pub hour: u8,
    pub severity: EventSeverity,
    pub kind: EventKind,
    pub cause_codes: Vec<EventCause>,
    pub summary: String,
}
```

Required `EventKind` values:

- `CycleProgressing`
- `AmmoniaWarning`
- `NitriteWarning`
- `OxygenDip`
- `AlgaeRiskRising`
- `AlgaeBloom`
- `BiofilmMaturityIncrease`
- `ShrimpBerried`
- `EggFailure`
- `MoltStressWarning`
- `FilterCleaningSetback`
- `SubstrateFoulingWarning`
- `StabilityImproving`

Tests must assert on `kind` and `cause_codes`, not exact summary text.

Keep at least the last `200` events in state. `TankSnapshot` must expose the last `20`.

#### 12. Data files and saves

Use TOML for presets.

Required preset sets:

- source water: `soft_acidic`, `moderate`, `hard_shrimp`, `ro_like`
- substrate: `inert_sand`, `inert_gravel`, `active_planted`, `coarse_porous`
- plants: `fast_stem`, `root_rosette`
- shrimp: `neocaridina_davidi`
- process parameters: one default pack
- scenarios: `nano_cycle`, `medium_planted`, `warm_room`

Each preset file must contain:

- `id`
- `name`
- parameter values
- optional `[provenance]` block with:
  - `source_title`
  - `source_year`
  - `confidence`
  - `notes`

Per-field provenance is not required for v1.

Save files must be JSON and contain:

- schema version
- app version
- full `TankState`
- queued actions
- RNG state

#### 13. TUI MVP

Provide these screens:

- `Overview`: day/hour, temperature, pH, DO, TAN, NH3, nitrite, nitrate, plant status, shrimp status, warnings
- `Chemistry`: chemistry details and recent trends
- `Biology`: plants, algae/periphyton, shrimp, biofilter maturity, hidden-state summaries
- `Actions`: forms for feed, water change, trim, siphon, clean filter, add/remove shrimp, change light, change heater, change ambient temperature
- `Log`: chronological event list

Minimum controls:

- step `1 hour`
- step `1 day`
- step `1 week`
- pause/autoadvance
- save/load
- quit

The TUI must only use `TankSnapshot` plus action dispatch. It must not directly mutate low-level state.

#### 14. Acceptance tests

All of the following tests are mandatory.

1. `determinism_same_seed`
   - same seed, same scenario, same action schedule, same binary/target
   - after `720` hours, serialized snapshots must be identical

2. `save_load_resume`
   - run `240` hours, save, load, run `480` more hours
   - final snapshot must match an uninterrupted `720`-hour run exactly

3. `water_change_mass_balance`
   - perform a `50%` water change using a zero-nutrient source profile
   - dissolved total masses for TAN, nitrite, nitrate, phosphate, DIC, and tracked ions must drop by `50% +/- 0.5%`

4. `tank_size_response`
   - identical stocked scenarios in `10 L` and `100 L`
   - after the same feed pulse, peak TAN in `mg/L` in `10 L` must be at least `5x` the `100 L` peak
   - after ambient rises from `24 C` to `31 C`, the `10 L` tank must reach half of its eventual temperature delta in fewer than half as many ticks as the `100 L` tank

5. `ambient_temperature_action`
   - start at stable `24 C`
   - change ambient to `31 C`
   - water temperature must rise gradually and not instantly jump to `31 C`
   - DO saturation target must decrease
   - heater output must be zero once water temperature is at or above setpoint

6. `cycling_seeded_vs_unseeded`
   - identical new tanks, one seeded and one unseeded
   - the seeded tank must reach `TAN < 0.1 mg/L` and `nitrite < 0.1 mg/L` earlier
   - the unseeded tank must show non-zero TAN and nitrite peaks

7. `overfeeding_effects`
   - compare control feeding versus repeated overfeeding for `14` days
   - overfed tank must end with higher detritus, lower nightly minimum DO, and higher algae nuisance index

8. `rooted_substrate_advantage`
   - same `RootFeedingRosette` plant in active substrate versus inert sand
   - after `30` days, biomass in active substrate must be at least `15%` higher

9. `thermal_reproduction_penalty`
   - identical shrimp scenarios at `25 C` and `30 C` for `60` days
   - the `30 C` tank must end with at least `20%` lower reproductive readiness and fewer juveniles

10. `event_generation`
    - a warm, overfed, weak-aeration scenario must emit at least one of:
      - `AmmoniaWarning`
      - `OxygenDip`
      - `AlgaeRiskRising`
    - emitted events must contain non-empty `cause_codes`

#### 15. Milestones

Implement in this order:

1. workspace skeleton, core types, config loading, RNG wrapper, save/load roundtrip
2. geometry, source water, water changes, temperature model, ambient temperature action
3. dissolved pools, NH3 calculation, DO model, event framework
4. detritus, decomposition, nitrifier guilds, cycling behavior
5. substrate, plants, algae/periphyton, rooted substrate advantage
6. shrimp population, reproduction, molt stress, mortality
7. TUI MVP, scenarios, full regression suite

Do not start milestone `N+1` until milestone `N` has passing tests.

#### 16. Definition of done

v1 is done when:

- all mandatory acceptance tests pass
- a user can start a preset shrimp-planted tank at multiple sizes
- the tank can cycle and stabilize over simulated weeks
- the user can feed, change water, and change ambient temperature
- chemistry and biology change in causal, inspectable ways
- shrimp can both succeed and fail reproductively depending on conditions
- save/load preserves deterministic continuation
- the TUI exposes the required views and actions
