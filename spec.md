
# Aquarium / Planted Tank Ecosystem Simulator — Implementation Spec (Rust)

Version: 0.1  
Target system: freshwater planted shrimp tank (`Neocaridina davidi`)  
Primary UI: terminal / text UI  
Core requirement: scientifically grounded, deterministic, agent-implementable

---

## 1. Objective

Build a Rust simulation engine that models a living freshwater planted aquarium as an ecosystem rather than a scripted set of events.

The simulator must let a player:

- configure tanks of different sizes and shapes
- stock and maintain a tank over time
- change husbandry variables, including **ambient temperature**
- observe chemistry, plant growth, algae, biofilm, microfauna, shrimp condition, and long-term stabilization
- encounter realistic emergent problems such as:
  - cycling delays
  - ammonia or nitrite stress
  - algae blooms
  - oxygen dips
  - breeding success/failure
  - molt issues
  - detritus accumulation
  - instability in small tanks compared with larger tanks

The design must be specific enough that an implementation agent can build the system incrementally without re-architecting the codebase.

---

## 2. Scope

## 2.1 In scope for v1

A single freshwater tank containing:

- water column
- tank geometry and water volume
- substrate
- filter / biomedia compartment
- heater (optional but supported)
- light
- rooted plants
- periphyton / biofilm
- free algae
- microbial guilds
- microfauna guilds
- shrimp population
- detritus / dissolved organics
- player maintenance actions

## 2.2 Out of scope for v1

- photorealistic graphics
- saltwater / reef chemistry
- disease-specific pathology models
- species-level microbial taxonomic simulation
- exact CFD, hydrodynamics, or molecular chemistry
- multiplayer
- online features

## 2.3 Planned later

- fish populations
- snails and other macro-grazers
- additional plant archetypes
- scenario editor
- richer rendering frontend
- more detailed carbonate chemistry and ion pairing
- external research calibration tools

---

## 3. Scientific modeling stance

The simulation is **mechanistic and guild-based**.

It must not attempt molecule-level or species-complete ecological simulation. Instead, it must model the main causal pathways that drive real aquarium outcomes:

- waste generation -> ammonia -> nitrite -> nitrate
- pH and temperature effects on unionized ammonia risk
- oxygen and carbon cycling
- plant vs algae competition
- substrate and biofilm effects
- microfauna grazing and predation loops
- shrimp feeding, stress, molting, and reproduction
- tank-size-dependent stability

Important scientific anchors for the model:

- Unionized ammonia fraction increases with pH, and temperature affects ammonia toxicity.
- Home aquarium biofilters show real microbial succession and variable cycle times rather than one fixed “recipe.”
- Aquarium biofilters commonly include AOA and comammox `Nitrospira`, so nitrification should not be represented as instant or purely magical conversion.
- Submerged macrophytes can take nutrients from sediment and/or the water column.
- Rising temperature interacts with nutrient availability and can shift the plant/algae balance.
- Protozoan grazing affects bacterial communities and nutrient mineralization.
- `Neocaridina davidi` reproduction is temperature-sensitive; high temperatures can suppress ovarian maturation and spawning.

---

## 4. Design principles

1. **Deterministic core**
   - same seed + same actions + same version = same result

2. **Mass-balance first**
   - store masses/amounts internally where possible
   - derive concentrations for display and toxicity calculations

3. **Tank size matters**
   - volume, footprint, depth, surface area, and wall area must affect the dynamics

4. **Engine/UI separation**
   - core simulation crate must not depend on terminal or graphics code

5. **Data-driven parameters**
   - species and process parameters loaded from data files
   - each important parameter carries provenance metadata

6. **Progressive fidelity**
   - start with robust abstractions
   - refine later where science or gameplay demands it

7. **Visible causality**
   - user should be able to inspect why the tank is improving or failing

---

## 5. Functional requirements

The system MUST support:

- different tank dimensions and fill heights
- different substrate types and depths
- at least one shrimp species pack
- at least two plant guilds
- at least two algae/biofilm guilds
- nutrient and oxygen cycling
- water temperature dynamics influenced by ambient temperature
- partial water changes with configurable source water
- feeding and detritus accumulation
- reproduction and juvenile survival for shrimp
- time acceleration (hours, days, weeks)
- logging of events and causes
- deterministic save/load

The system SHOULD support:

- seasonal ambient temperature schedules
- heater thermostat behavior
- filter cleaning side effects
- top-off water
- fertilizer dosing
- photoperiod editing
- different light strengths
- simple denitrification in oxygen-poor substrate microzones

The system MAY support later:

- fish
- snails
- disease
- graphical rendering
- multiplayer or shared scenarios

---

## 6. Tank size and geometry model

Tank size is a first-class simulation input, not cosmetic metadata.

## 6.1 Required geometry inputs

```rust
pub struct TankGeometry {
    pub length_cm: f32,
    pub width_cm: f32,
    pub height_cm: f32,
    pub fill_height_cm: f32,
    pub glass_thickness_mm: f32,
    pub open_top: bool,
}
```

## 6.2 Derived geometry quantities

The engine MUST derive:

- `gross_water_volume_l`
- `water_volume_l_with_substrate_depth(...)`
- `surface_area_cm2` = length * width
- `footprint_area_cm2`
- `wall_area_cm2`
- `substrate_plan_area_cm2`
- `mean_depth_cm`
- `surface_area_to_gross_volume_ratio`
- `wall_area_to_gross_volume_ratio`

## 6.3 Why geometry matters

These derived values must affect simulation outcomes:

- **Dilution / concentration**
  - same mass of feed in a 10 L tank causes a larger concentration spike than in 100 L

- **Thermal inertia**
  - smaller tanks change temperature faster

- **Heat exchange**
  - surface area and wall area affect passive exchange with ambient air

- **Gas exchange**
  - open water surface area affects oxygenation / CO2 exchange

- **Light distribution**
  - deeper tanks attenuate light more strongly with depth

- **Plant carrying capacity**
  - footprint and substrate depth affect rooted plant biomass capacity

- **Periphyton / biofilm area**
  - hardscape, glass area, and substrate expose colonizable surfaces

- **Bioload stability**
  - small tanks are less forgiving; large tanks buffer fixed disturbances better

## 6.4 Tank size classes

The engine must not hardcode classes, but example presets should exist:

- nano: 5–20 L
- small: 20–60 L
- medium: 60–200 L
- large: 200+ L

The same scenario logic must run at any size.

---

## 7. Time model

## 7.1 Base tick

The engine MUST use a fixed-step simulation.

Recommended v1 base tick:
- **1 hour per simulation tick**

Optional implementation detail:
- allow internal substeps (`substeps_per_tick`) for future numerical stability without changing public behavior.

## 7.2 Multi-scale update layers

Use three logical clocks:

### Hourly layer
- temperature
- gas exchange
- ammonia speciation
- oxygen changes
- decomposition
- nitrification
- feeding pulses
- excretion
- light exposure accumulation

### Daily layer
- plant growth
- algae growth
- biofilm maturation
- shrimp condition/stress
- molting progress
- egg development
- juvenile recruitment
- mortality checks

### Weekly/monthly layer
- ecological succession indicators
- substrate enrichment/depletion
- biofilter maturity trends
- long-term stability metrics
- seasonal ambient patterns

## 7.3 Determinism

All stochastic events must use an explicit RNG object seeded from scenario state.

```rust
pub struct SimSeed(pub u64);
```

No direct calls to thread-local RNG are permitted inside the core engine.

---

## 8. Simulation state model

## 8.1 Core state container

```rust
pub struct TankState {
    pub meta: SimMeta,
    pub geometry: TankGeometry,
    pub environment: EnvironmentState,
    pub hardware: HardwareState,
    pub water: WaterState,
    pub substrate: Vec<SubstrateLayerState>,
    pub filter: FilterState,
    pub plants: Vec<PlantGuildState>,
    pub algae: AlgaeState,
    pub microbes: MicrobeState,
    pub microfauna: MicrofaunaState,
    pub animals: AnimalState,
    pub detritus: DetritusState,
    pub schedules: ScheduleState,
    pub observables: ObservableState,
    pub events: Vec<SimEvent>,
    pub rng: SimRng,
}
```

## 8.2 Environment state

```rust
pub struct EnvironmentState {
    pub ambient_temp_c: f32,
    pub ambient_temp_schedule: AmbientSchedule,
    pub room_light_leak: f32, // 0..1, optional future use
    pub season_day: u16,
}
```

### Ambient temperature action
Ambient temperature must be editable by the player at runtime. This represents room heating/cooling, seasonal weather, AC failure, or relocation of the tank.

This action must affect:
- water temperature trends
- heater load
- dissolved oxygen saturation
- plant/algae rates
- shrimp stress and reproduction
- ammonia risk indirectly through temperature and pH-sensitive chemistry

## 8.3 Hardware state

```rust
pub struct HardwareState {
    pub light: LightState,
    pub heater: Option<HeaterState>,
    pub filter_pump: FilterPumpState,
    pub aeration: AerationState,
}
```

### Light
```rust
pub struct LightState {
    pub enabled: bool,
    pub photoperiod_hours: f32,
    pub intensity_index: f32,      // relative scalar for v1
    pub par_surface_umol: f32,     // optional if known
    pub schedule_start_hour: u8,
}
```

### Heater
```rust
pub struct HeaterState {
    pub enabled: bool,
    pub setpoint_c: f32,
    pub max_watts: f32,
    pub deadband_c: f32,
    pub efficiency: f32,
}
```

### Filter / aeration
```rust
pub struct FilterPumpState {
    pub flow_lph: f32,
    pub media_surface_area_m2: f32,
    pub cleanliness: f32,  // 0..1
}

pub struct AerationState {
    pub enabled: bool,
    pub intensity: f32,    // relative scalar
}
```

## 8.4 Water state

Internal water chemistry should primarily store **amounts** plus some derived state. Concentrations are derived from amount / volume.

```rust
pub struct WaterState {
    pub volume_l: f32,
    pub temp_c: f32,

    // Nitrogen
    pub ammonia_total_mg_n: f32,   // TAN = NH3 + NH4+, as N
    pub nitrite_mg_n: f32,
    pub nitrate_mg_n: f32,

    // Phosphorus
    pub phosphate_mg_p: f32,

    // Carbon / oxygen
    pub dissolved_oxygen_mg: f32,
    pub dissolved_inorganic_carbon_mg_c: f32,

    // Organic pools
    pub dissolved_organic_carbon_mg_c: f32,
    pub dissolved_organic_nitrogen_mg_n: f32,

    // Major ions (minimum useful set)
    pub calcium_mg: f32,
    pub magnesium_mg: f32,
    pub sodium_mg: f32,
    pub potassium_mg: f32,
    pub bicarbonate_mg: f32,
    pub chloride_mg: f32,
    pub sulfate_mg: f32,

    // Solver-facing water chemistry
    pub alkalinity_meq: f32,
    pub ph: f32,
    pub conductivity_us_cm: f32,
    pub tds_mg_l_display: f32,
}
```

### Important rule
`TDS` must be a **derived display metric**, not the main hidden state. Internal modeling should use ions and alkalinity where possible.

## 8.5 Substrate state

Substrate must be layered.

```rust
pub struct SubstrateLayerState {
    pub kind: SubstrateKind,
    pub depth_cm: f32,
    pub porosity: f32,
    pub cec_index: f32,               // cation exchange capacity proxy
    pub organic_matter_g: f32,
    pub trapped_detritus_g: f32,
    pub nutrient_store_n_mg: f32,
    pub nutrient_store_p_mg: f32,
    pub oxygen_penetration_index: f32,
    pub colonizable_area_m2: f32,
}
```

### Required v1 substrate kinds
- inert sand
- inert gravel
- nutrient-rich planted substrate
- porous rubble / biomedia-like coarse layer

Substrate must affect:
- rooted nutrient access
- detritus trapping
- microbial habitat
- potential low-oxygen zones
- shrimp grazing surface

## 8.6 Filter state

```rust
pub struct FilterState {
    pub maturity: f32,                // 0..1 summary metric
    pub ammonia_oxidizer_biomass: f32,
    pub nitrite_oxidizer_biomass: f32,
    pub comammox_biomass: f32,
    pub heterotroph_biomass: f32,
    pub clogging: f32,
}
```

## 8.7 Plant guild state

Plants are modeled as guilds, not per-leaf geometry.

```rust
pub enum PlantGuildKind {
    FastStem,
    RootFeedingRosette,
    MossEpiphyte,
    FloatingPlant,
}

pub struct PlantGuildState {
    pub kind: PlantGuildKind,
    pub name: String,
    pub biomass_g_dry: f32,
    pub health: f32,                  // 0..1
    pub nutrient_reserve_index: f32,  // 0..1
    pub shade_factor: f32,            // 0..1
    pub root_fraction: f32,
    pub growth_potential: f32,
}
```

At minimum, v1 must include:
- one fast-growing water-column user
- one rooted/substrate-responsive plant guild

## 8.8 Algae state

```rust
pub struct AlgaeState {
    pub suspended_algae_biomass: f32,   // green water
    pub surface_film_biomass: f32,      // glass/film algae
    pub periphyton_biomass: f32,        // attached biofilm + algae complex
    pub nuisance_index: f32,            // 0..1 summary for UI
}
```

## 8.9 Microbe state

```rust
pub struct MicrobeState {
    pub decomposer_biomass: f32,
    pub ammonia_oxidizer_biomass: f32,
    pub nitrite_oxidizer_biomass: f32,
    pub comammox_biomass: f32,
    pub mineralization_capacity: f32,
}
```

## 8.10 Microfauna state

```rust
pub struct MicrofaunaState {
    pub bacterivorous_protists: f32,
    pub rotifers: f32,
    pub nematodes: f32,
    pub microcrustaceans: f32,
    pub visibility_index: f32,
}
```

Microfauna are mostly hidden from the player in v1, but their effects are not optional.

## 8.11 Animal state (v1 shrimp focus)

```rust
pub struct AnimalState {
    pub shrimp: ShrimpPopulationState,
}

pub struct ShrimpPopulationState {
    pub adults: u32,
    pub juveniles: u32,
    pub berried_females: u32,
    pub average_condition: f32,        // 0..1
    pub molt_stress_index: f32,        // 0..1
    pub reproductive_readiness: f32,   // 0..1
    pub egg_progress_days: f32,
}
```

## 8.12 Detritus state

```rust
pub struct DetritusState {
    pub particulate_organic_matter_g: f32,
    pub fine_detritus_g: f32,
    pub bioavailable_food_g: f32,
}
```

---

## 9. Observables vs hidden state

The engine must distinguish:

### Hidden internal state
Used for causality and simulation:
- microbial biomasses
- nutrient stores in substrate
- microfauna pools
- exact detrital pools
- internal maturity indices

### Observable state
Shown to user:
- temperature
- pH
- GH / KH
- TDS
- conductivity
- DO
- TAN / nitrite / nitrate / phosphate
- plant health and growth direction
- algae risk
- shrimp population and condition
- biofilter maturity
- event log
- warnings/explanations

This separation allows scientific depth without drowning the player in hidden variables.

---

## 10. Core simulation rules

## 10.1 Temperature model

Water temperature must be dynamic.

Required equation family:

```text
dT/dt = (Q_ambient + Q_heater + Q_light - Q_evap) / heat_capacity
```

Where:

- `heat_capacity` is proportional to tank water mass
- `Q_ambient` is based on temperature difference between ambient air and water and a geometry-dependent transfer coefficient
- `Q_heater` depends on heater power and thermostat logic
- `Q_light` is optional heating from lights
- `Q_evap` may be omitted in v1 and added later

### Minimum v1 implementation
Use:

```text
Q_ambient = UA_total * (T_ambient - T_water)
UA_total = k_surface * surface_area + k_walls * wall_area
heat_capacity = water_volume_l * 4186 J/K approximately
```

### Acceptance behavior
- small open tanks respond faster to ambient changes than large tanks
- heaters partially buffer ambient swings
- if ambient exceeds heater setpoint, heater cannot cool the tank
- high ambient temperature reduces oxygen saturation and can stress shrimp

## 10.2 Nitrogen cycle

The minimum nitrogen pathway:

```text
feed -> organic matter -> mineralization -> TAN -> nitrite -> nitrate
```

### Inputs to TAN
- animal excretion
- decomposition/mineralization of detritus
- optional fertilizer dose

### Outputs from TAN
- nitrification
- plant uptake
- water changes

### Outputs from nitrite
- nitrite oxidation
- water changes

### Outputs from nitrate
- plant uptake
- water changes
- optional denitrification in later phase

### Required implementation rule
Nitrification must be biomass-limited, oxygen-limited, temperature-sensitive, and pH-sensitive.

Do not implement nitrification as instant or as a fixed percentage independent of biology.

Recommended generic rate form:

```text
rate = Vmax * guild_biomass
     * f_temp(T)
     * f_pH(pH)
     * f_DO(DO)
     * substrate / (K + substrate)
```

### Guild structure
v1 must support:
- ammonia oxidizers
- nitrite oxidizers
- comammox guild

The agent may simplify by allowing comammox to oxidize TAN directly into nitrate with its own slower but efficient rate curve.

## 10.3 Ammonia speciation and risk

Total ammonia is not enough. The engine must estimate the unionized fraction.

Required approach:
- store total ammonia as TAN
- compute NH3 fraction from pH and temperature each tick
- use NH3-derived risk for toxicity/stress events

Implementation note:
- use the freshwater equilibrium relation based on the Emerson et al. formulation commonly cited by EPA:
  - `pKa = 0.09018 + 2729.92 / (273.2 + T_celsius)`
  - `fraction_NH3 = 1 / (1 + 10^(pKa - pH))`

Then:
```text
NH3_mg_n_per_l = TAN_mg_n_per_l * fraction_NH3
NH4_mg_n_per_l = TAN_mg_n_per_l - NH3_mg_n_per_l
```

NH3 should drive acute risk more strongly than TAN.

## 10.4 Oxygen and gas exchange

The engine must track dissolved oxygen explicitly.

### Sources
- plant photosynthesis (light-dependent)
- periphyton photosynthesis
- atmospheric reaeration
- aeration equipment

### Sinks
- animal respiration
- plant respiration
- microbial respiration
- decomposition
- nitrification oxygen demand

Recommended v1 form:

```text
dDO/dt = photosynthesis
       + kLa_DO * (DO_sat(T) - DO)
       - respiration_total
       - nitrification_O2_cost
```

### Required behaviors
- DO saturation decreases as temperature rises
- night-time oxygen can dip in planted tanks
- strong aeration increases gas exchange
- overfeeding increases oxygen demand via decomposition

CO2/DIC may be modeled at lower fidelity in v1, but DO is mandatory.

## 10.5 pH / alkalinity

The engine must not treat pH as completely arbitrary.

### Minimum acceptable v1
Track:
- alkalinity / KH
- dissolved inorganic carbon
- acidifying effect of nitrification
- degassing and photosynthesis effects on DIC

Then solve or approximate pH each tick from alkalinity + DIC + temperature.

### If full carbonate solver is too heavy for initial milestone
Temporary fallback:
- maintain pH as a derived approximation from KH and DIC
- document this as a temporary simplification
- do not hardcode pH as constant if plants, nitrification, and water changes occur

## 10.6 Major ions, GH, KH, TDS

v1 must track a minimal ion set sufficient to derive:
- GH from Ca + Mg
- KH / alkalinity from bicarbonate/alkalinity
- TDS display from dissolved ions
- conductivity display from ion content or empirical conversion

This is required because the project explicitly cares about TDS and mineral conditions.

### Required rule
Water changes and top-offs must mix source water chemistry into current tank water; they must not simply “reset numbers.”

## 10.7 Plant growth

Plants should not just be decorative biomass.

Plant growth must depend on:
- available light
- water temperature
- nutrient availability
- access to nutrients from water and/or substrate depending on guild
- self-shading
- algae/periphyton competition
- stress from poor chemistry

Recommended rate structure:

```text
plant_growth = biomass
             * f_light
             * f_temp
             * min(f_nitrogen, f_phosphorus, f_carbon)
             * habitat_factor
             - respiration
             - senescence
```

### Guild differences
- fast stem plants: stronger water-column nutrient uptake, faster growth, stronger shading feedback
- root-feeding plants: benefit more from nutrient-rich sediment and adequate substrate depth
- moss/epiphytes: more surface-attached, low root dependence
- floating plants: strong light access, strong nutrient uptake, high shading effect

### Required behavior
Nutrient-rich substrate must benefit rooted plants more than inert substrate.

## 10.8 Algae and periphyton

Separate at least:
- suspended algae
- attached film/periphyton

Growth drivers:
- light
- excess nutrients
- temperature
- low grazing pressure
- weak plant competition
- surface availability (for attached forms)

Recommended structure:

```text
algae_growth = biomass
             * f_light
             * f_temp
             * min(f_nitrogen, f_phosphorus)
             * competition_modifier
             - grazing
             - sloughing
```

### Required behavior
The system must support:
- green-water style blooms when nutrients/light are favorable
- film/periphyton increase on surfaces under imbalance
- attached biofilm as both nuisance and shrimp food source

## 10.9 Biofilm / periphyton role

Periphyton is not merely “algae.” It is a mixed guild that includes autotrophs and heterotrophs and should be treated as an ecological intermediary.

Periphyton must:
- provide shrimp grazing resource
- contribute to oxygen and nutrient dynamics
- increase with available surface area and light/nutrients
- support microfauna habitat
- become nuisance at high biomass

## 10.10 Decomposition and mineralization

Detritus must move through at least these pools:

```text
particulate food/waste -> fine detritus -> dissolved organics -> mineralized nutrients
```

Rates depend on:
- temperature
- decomposer biomass
- oxygen
- detritus quality
- substrate trapping

Overfeeding must increase:
- detritus
- microbial oxygen demand
- TAN generation
- algae risk

## 10.11 Microfauna

Microfauna should be abstracted as guilds with real effects.

Minimum effects:
- bacterivorous protists reduce bacterial standing biomass but enhance nutrient turnover
- rotifers/nematodes/microcrustaceans compete for biofilm resources
- shrimp grazing/predation suppresses some microfauna pools

Player does not need exact counts, but the engine must use these pools in ecology.

## 10.12 Shrimp physiology and population

`Neocaridina davidi` is the target reference species for v1.

The shrimp model must include:
- adults
- juveniles
- berried females
- condition
- molt stress
- reproduction readiness
- juvenile survival

### Inputs affecting shrimp condition
- food availability
- biofilm availability
- temperature
- DO
- NH3 risk
- nitrite risk
- mineral status (GH / ion adequacy proxy)
- instability / sudden swings

### Reproduction
A simple but meaningful model:

```text
spawn_probability =
    f_temperature
  * f_condition
  * f_stability
  * f_mineral_status
  * sex_ratio_proxy
```

When spawning succeeds:
- berried female count increases
- egg development progresses over days
- hatch success depends on stress during egg period

### Molting
Molting stress rises when:
- mineral conditions are poor
- temperature is stressful
- rapid chemistry swings occur
- condition is poor

### Mortality
Mortality should be probabilistic, with risk increased by:
- NH3
- severe nitrite
- low oxygen
- prolonged thermal stress
- failed molt stress
- starvation / collapse of biofilm and food web

---

## 11. Player actions

Actions are part of the simulation API.

## 11.1 Setup actions

- choose tank dimensions and fill height
- choose substrate type(s) and depth
- choose filter and media
- choose heater and setpoint
- choose light intensity and photoperiod
- define source water chemistry
- add plants
- add shrimp
- optionally seed filter/media

## 11.2 Runtime husbandry actions

The engine MUST support at least:

```rust
pub enum PlayerAction {
    Feed { grams: f32 },
    WaterChangePercent { percent: f32, source: SourceWaterProfile },
    TopOffEvaporation { liters: f32, source: SourceWaterProfile },
    DoseFertilizer { n_mg: f32, p_mg: f32, k_mg: f32 },
    TrimPlants { fraction: f32 },
    SiphonDetritus { fraction: f32 },
    CleanFilter { intensity: f32 },
    AddShrimp { count: u32 },
    RemoveShrimp { count: u32 },
    ChangePhotoperiod { hours: f32 },
    ChangeLightIntensity { intensity_index: f32 },
    ChangeHeaterSetpoint { setpoint_c: f32 },
    ChangeAmbientTemperature { target_c: f32 },
    ChangeAeration { enabled: bool, intensity: f32 },
}
```

### Important requirement
`ChangeAmbientTemperature` is mandatory in the public API.

## 11.3 Action semantics

### Feed
- adds particulate organics
- increases later detritus and excretion
- increases food availability
- may improve reproduction if not excessive

### Water change
- removes dissolved and suspended constituents proportionally
- mixes in source water temperature and chemistry
- can cause stress if parameters differ strongly

### Top-off
- restores volume but usually does not remove dissolved waste
- depending on source chemistry, can alter TDS/KH/GH

### Clean filter
- reduces clogging
- may remove nitrifier biomass if too aggressive

### Change ambient temperature
- changes room environment, not instant water temperature
- effect propagates through heat transfer over time

---

## 12. Update order per tick

The order must be explicit and stable.

Recommended hourly update pipeline:

1. apply scheduled environment updates
2. apply queued player actions
3. update water volume (evaporation/top-off/water change)
4. update temperature from ambient + heater + equipment
5. update light state for current hour
6. compute gas solubility targets from temperature
7. process feeding/excretion pulses
8. move detritus through decomposition/mineralization
9. update nitrification
10. update carbon/oxygen exchange
11. update plant photosynthesis/respiration
12. update algae/periphyton photosynthesis/respiration
13. update pH / alkalinity / NH3 fraction
14. update shrimp feeding/stress
15. accumulate daily summaries
16. emit events / warnings / observations
17. recompute observables

Daily update pipeline:

1. plant biomass growth/senescence
2. algae net biomass changes
3. microfauna turnover
4. shrimp reproduction and juvenile recruitment
5. mortality checks
6. biofilter maturity trend updates
7. weekly/monthly counters if needed

---

## 13. Event and explanation system

The simulation must produce interpretable events.

```rust
pub struct SimEvent {
    pub day: u32,
    pub severity: EventSeverity,
    pub kind: EventKind,
    pub summary: String,
    pub causes: Vec<String>,
}
```

Required event kinds:
- cycle progressing
- ammonia warning
- nitrite warning
- oxygen dip
- algae bloom risk rising
- algae bloom underway
- biofilm maturity increase
- shrimp berried
- egg drop / reproductive failure
- molt stress warning
- filter cleaning setback
- substrate fouling warning
- stability improving

Example:
```text
Day 41: Moderate warning — night oxygen dip
Causes:
- high detritus after heavy feeding
- strong plant biomass increased night respiration
- low aeration
- warm ambient temperature reduced oxygen saturation
```

This layer is essential for learning and gameplay.

---

## 14. Persistence and data-driven content

## 14.1 Config files

Use serialized config files for:
- tank presets
- source water profiles
- substrate types
- plant guild packs
- shrimp species packs
- process parameter sets
- scenarios

## 14.2 Parameter provenance

Every important parameter should support metadata:

```rust
pub struct Parameter<T> {
    pub value: T,
    pub unit: String,
    pub source_title: String,
    pub source_year: u16,
    pub confidence: f32,      // 0..1
    pub notes: String,
}
```

This is required for scientific credibility and later calibration.

## 14.3 Save files

A save file must include:
- full `TankState`
- simulation version
- RNG state
- schema version

A save should be resumable with bitwise-identical deterministic results if code version is unchanged.

---

## 15. Rust workspace structure

Recommended workspace:

```text
aquarium-sim/
  Cargo.toml
  crates/
    tank_core/
    tank_data/
    tank_tui/
    tank_scenarios/
    tank_cli/        # optional launcher
```

## 15.1 `tank_core`

Contains:
- state types
- update systems
- equations
- action application
- save/load schema
- tests
- deterministic RNG wrapper

No UI dependencies allowed.

## 15.2 `tank_data`

Contains:
- TOML/JSON/RON assets
- parameter loading
- validation of configs
- preset species and scenario packs

## 15.3 `tank_tui`

Contains:
- terminal dashboard
- input mapping
- time controls
- screens/tabs
- charts / sparkline widgets

## 15.4 `tank_scenarios`

Contains:
- sample scenarios
- regression validation scenarios
- benchmark presets (nano tank, medium planted tank, warm-room stress case, etc.)

---

## 16. Suggested crate/tool choices

These are recommendations, not hard requirements.

- `serde` for serialization/deserialization
- `thiserror` for typed errors
- `rand_chacha` or another deterministic RNG
- `ratatui` for terminal UI
- `tracing` for instrumentation
- `uom` or a lightweight custom units layer if desired
- `approx` for float comparisons in tests

Do **not** couple the core to a graphics engine in v1.

---

## 17. Public engine API

```rust
pub trait SimulationEngine {
    fn apply_action(&mut self, action: PlayerAction) -> Result<(), SimError>;
    fn step_hours(&mut self, hours: u32) -> Result<(), SimError>;
    fn snapshot(&self) -> TankSnapshot;
    fn full_state(&self) -> &TankState;
}
```

### Snapshot
`TankSnapshot` should expose UI-friendly derived values without requiring the UI to know the low-level chemistry model.

```rust
pub struct TankSnapshot {
    pub day: u32,
    pub temp_c: f32,
    pub ph: f32,
    pub do_mg_l: f32,
    pub tan_mg_l: f32,
    pub nh3_mg_l: f32,
    pub nitrite_mg_l: f32,
    pub nitrate_mg_l: f32,
    pub phosphate_mg_l: f32,
    pub gh_d: f32,
    pub kh_d: f32,
    pub conductivity_us_cm: f32,
    pub tds_mg_l: f32,
    pub plant_status: Vec<PlantStatusView>,
    pub shrimp_status: ShrimpStatusView,
    pub algae_risk: f32,
    pub biofilter_maturity: f32,
    pub recent_events: Vec<SimEvent>,
}
```

---

## 18. Numerical and modeling requirements

## 18.1 Internal representation rules

Prefer:
- masses in mg or g
- volumes in liters
- temperatures in °C
- times in hours/days
- derived concentrations in mg/L
- conductivity in µS/cm
- GH/KH in degree equivalents only for display

## 18.2 Clamping and invariants

All update systems must enforce:
- no negative masses
- no negative populations
- no temperature below freezing in normal freshwater scenarios unless explicitly allowed
- `0.0 <= health <= 1.0`
- `0.0 <= maturity <= 1.0`

## 18.3 Stability

Systems must avoid:
- uncontrolled exponential blow-ups from naive multiplicative rates
- order dependence caused by mutable global state without explicit pipeline
- floating-point drift causing negative pools

Where necessary:
- clamp rates to available mass
- use saturating subtraction
- use bounded logistic or Monod-like functions

---

## 19. TUI requirements

The terminal UI is part of the MVP.

## 19.1 Minimum screens

1. **Overview**
   - day/time
   - chemistry summary
   - plant summary
   - shrimp summary
   - event warnings

2. **Chemistry**
   - temperature
   - pH
   - DO
   - TAN / NH3 / nitrite / nitrate / phosphate
   - GH/KH/TDS/conductivity
   - trends over recent days

3. **Biology**
   - plants
   - algae/periphyton
   - shrimp population
   - reproduction
   - hidden-state summaries like biofilter maturity and microfauna health

4. **Actions**
   - feed
   - water change
   - trim
   - siphon
   - change ambient temp
   - change light / heater settings

5. **Event log**
   - chronological causal log

## 19.2 Time controls

- advance 1 hour
- advance 1 day
- advance 1 week
- pause/autoadvance

---

## 20. Acceptance tests and regression scenarios

These are mandatory.

## 20.1 Determinism test
Given identical seed and action sequence, snapshots must match exactly or within fixed float tolerance.

## 20.2 Mass balance test
Water change removes the expected fraction of dissolved constituents.

## 20.3 Tank-size stability test
With identical feed mass and livestock addition:
- 10 L tank must show larger concentration swings than 100 L tank
- 10 L tank must change temperature faster after ambient temp jump

## 20.4 Ambient temperature test
Scenario:
- stable tank at 24 °C
- ambient raised to 31 °C
Expected:
- water warms over time, not instantly
- DO saturation target falls
- shrimp stress rises if sustained
- heater usage drops to zero if heater present

## 20.5 Cycling test
Brand-new tanks with identical setup but different seeded states should show different time-to-stability.

Expected:
- TAN and nitrite do not disappear instantly
- nitrifier biomass grows over time
- filter cleaning early in the cycle can delay maturation

## 20.6 Overfeeding test
Repeated overfeeding must produce:
- rising detritus
- increased oxygen demand
- TAN increase
- elevated algae risk

## 20.7 Rooted substrate test
A rooted plant guild in nutrient-rich substrate must outperform the same guild in inert sand under otherwise equal conditions.

## 20.8 Reproduction thermal stress test
At high sustained temperature, shrimp reproduction readiness and success must fall relative to moderate temperature conditions.

## 20.9 Water change chemistry shock test
A large water change with very different GH/KH/TDS must improve some waste metrics while potentially increasing stress.

---

## 21. Milestone plan for an implementation agent

## Milestone 0 — repository skeleton
Deliver:
- cargo workspace
- core types
- build/test CI
- sample config loading
- save/load roundtrip test

## Milestone 1 — tank geometry and temperature
Deliver:
- `TankGeometry`
- `EnvironmentState`
- `HardwareState`
- temperature update system
- ambient temperature action
- heater logic
- tank-size comparison tests

## Milestone 2 — dissolved pools and water changes
Deliver:
- `WaterState`
- source water profiles
- water change/top-off actions
- TDS / GH / KH / conductivity derivation
- chemistry snapshot rendering

## Milestone 3 — nitrogen + oxygen cycle
Deliver:
- detritus pools
- decomposition
- nitrification guilds
- oxygen dynamics
- NH3 fraction calculation
- event warnings

## Milestone 4 — substrate + plants + algae
Deliver:
- substrate layers
- plant guilds
- algae/periphyton
- light schedule
- rooted vs water-column nutrient access
- trim action

## Milestone 5 — shrimp + microfauna
Deliver:
- shrimp population model
- food-web linkage to biofilm/periphyton
- reproduction / molt stress
- high-temperature reproduction penalty
- microfauna abstraction

## Milestone 6 — TUI MVP
Deliver:
- overview/chemistry/biology/actions/log screens
- time controls
- scenario selection
- save/load support

## Milestone 7 — validation + balancing
Deliver:
- regression scenarios
- parameter audit with provenance
- balancing pass for plausible trajectories
- documentation of known simplifications

---

## 22. Initial parameter packs required

The first data pack must include:

### Source water profile presets
- soft acidic
- moderate community-tank water
- hard alkaline shrimp water
- RO-like low-mineral water

### Substrate presets
- inert sand
- inert gravel
- active planted substrate
- coarse porous media mix

### Plant guild presets
- fast stem
- rooted rosette
- moss/epiphyte
- floating plant (optional v1.1)

### Shrimp pack
- `Neocaridina davidi`
  - moderate temperature optimum
  - high-temp reproductive penalty
  - biofilm grazing preference
  - GH dependence proxy
  - moderate juvenile sensitivity

---

## 23. Known simplifications allowed in v1

These simplifications are acceptable if documented:

- aggregated microfauna guilds instead of species
- approximate carbonate chemistry rather than full equilibrium across all acid/base systems
- no explicit bacterial taxonomy beyond guilds
- no spatial CFD
- one well-mixed water column compartment
- substrate represented as 1–3 layers rather than continuous depth
- simple sex-ratio proxy for shrimp rather than individual mating model

These are acceptable because the target is realistic aquarium dynamics, not full ecological omniscience.

---

## 24. Explicit non-negotiables

The following must not be cut from the MVP:

1. **different tank sizes**
2. **ambient temperature as a real action**
3. **temperature-dependent water behavior**
4. **pH/temperature-dependent ammonia risk**
5. **non-instant biological cycling**
6. **substrate effects**
7. **plant vs algae competition**
8. **shrimp reproduction and failure states**
9. **deterministic save/load**
10. **text UI**

If any simplification is necessary, simplify detail, not these core loops.

---

## 25. Research anchors for calibration

These are not exhaustive; they are the initial anchor points the simulation should respect during calibration.

1. Ammonia speciation and toxicity depend strongly on pH and temperature.
2. Home aquarium biofilters can cycle at different speeds; in one recent study, ammonia and nitrite fell to undetectable levels by week 3 in two aquariums and by week 8 in another.
3. Freshwater aquarium biofilters commonly contain AOA and comammox `Nitrospira`.
4. Rooted submerged macrophytes can draw nutrients from sediment and/or the water column.
5. Rising temperature interacts with nutrient regime and can shift aquatic plant performance and algal competition.
6. Protozoan grazing changes bacterial biomass/community dynamics and nutrient turnover.
7. `Neocaridina davidi` predates on meiofauna and can reshape those communities.
8. `Neocaridina davidi` reproduction is inhibited at sufficiently high sustained temperature.

These anchors should inform calibration and regression scenarios.

---

## 26. Example first sprint ticket list

1. Define all core structs and enums.
2. Implement geometry derivation and unit tests.
3. Implement source water profile loader.
4. Implement water change mixing.
5. Implement ambient temperature and heater tick.
6. Implement DO saturation utility.
7. Implement TAN/NH3 helper.
8. Implement detritus -> TAN pipeline.
9. Implement nitrifier guild growth and oxidation.
10. Implement daily plant update.
11. Implement algae/periphyton update.
12. Implement shrimp status model and daily reproduction check.
13. Implement event emission layer.
14. Build TUI overview screen.
15. Add regression scenarios for nano vs medium tank.

---

## 27. Definition of done for v1

v1 is complete when a user can:

- start a shrimp-planted tank at one of several sizes
- watch it cycle and mature over weeks
- feed it, change water, and alter ambient temperature
- see chemistry and biological state evolve over time
- cause both healthy stabilization and realistic failure modes
- observe shrimp breeding success or failure under changing conditions
- inspect a log that explains why those outcomes happened

That is the minimum successful product.


---

## 28. Reference set used for scientific anchoring

These are the initial sources used to shape the first spec and calibration targets.

- EPA CADDIS: **Ammonia** — ammonia occurs as NH3 + NH4+, higher pH favors the more toxic unionized NH3, and temperature also affects toxicity.
- McKnight et al. 2025, **Microbial community succession of home aquarium biofilters associated with early establishment of comammox Nitrospira** — reported variable early-cycle trajectories in home aquarium biofilters, with undetectable ammonia/nitrite by week 3 in two aquariums and by week 8 in another.
- McKnight et al. 2024, **Comammox Nitrospira among dominant ammonia oxidizers within aquarium biofilter microbial communities** — supports explicit treatment of comammox in freshwater aquarium biofilters.
- Clarke 2001, **Sediment nutrient characteristics and aquatic macrophytes in lowland English rivers** — rooted submerged macrophytes can use nutrients from sediment and/or water column.
- Zhang et al. 2020, **Interactive Effects of Rising Temperature and Nutrient Enrichment on Aquatic Plant Growth, Stoichiometry, and Palatability** — temperature and nutrient regime interact to change plant growth; external nutrient loading can suppress plants through algal competition.
- Rønn et al. 2002, **Impact of Protozoan Grazing on Bacterial Community Structure** — protozoan grazing alters bacterial biomass/community structure and nutrient turnover.
- Weber & Traunspurger 2016, **Influence of the ornamental red cherry shrimp Neocaridina davidi on freshwater meiofaunal assemblages** — shrimp predation can depress meiofaunal abundance, biomass, and secondary production.
- Baliña et al. 2018, **Losing Reproduction: Effect of High Temperature on Female Biochemical Composition and Egg Quality in Neocaridina davidi** — high sustained temperature inhibited ovarian maturation and spawning at 33 °C relative to 28 °C.
- USGS 2023, **Salinity and total dissolved solids measurements for natural waters** — supports treating TDS as a derived aggregate metric rather than the deepest internal state.
