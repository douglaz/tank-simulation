# Scientific Inventory

This note inventories what the simulator means today, as inspected on 2026-03-27. It is intentionally descriptive, not prescriptive: it records current storage semantics, units, invariants, shortcuts, and test locks so later beads can cite one repo-local reference instead of rediscovering them in code.

Quick orientation:

- `WaterState` stores almost everything as absolute totals for the whole tank. Only `temperature_c` and `ph` are not totals.
- Nitrogen pools are stored as elemental nitrogen (`mg N total` internally, `mg N/L` after dividing by volume), not ion-mass equivalents.
- Source-water presets are per-liter inputs. `TankState`, snapshots, and save files store the post-materialization totals.
- The backlog comment mentions 19 integration tests, but the repository currently has 21 test files on disk: 19 under `crates/tank_core/tests/` and 2 under `crates/tank_scenarios/tests/`.

## 1. Pool Catalog

### 1.1 Water-state storage

| Field name | Location | Unit | Meaning | Compartment | Known shortcuts |
| --- | --- | --- | --- | --- | --- |
| `temperature_c` | `crates/tank_core/src/types/water.rs:11`, `crates/tank_core/src/systems/temperature.rs` | `deg C` | Bulk mixed-water temperature used by thermal, dissolved-oxygen, NH3, chemistry, and shrimp logic. | Water column | Single well-mixed temperature; no stratification, substrate thermal inertia, or glass-thickness effect despite `geometry.glass_thickness_mm`. |
| `ammonia_total_mg_n_total` | `crates/tank_core/src/types/water.rs:12`, consumed in `nitrogen_cycle.rs`, `plant_growth.rs`, `algae_growth.rs`, `shrimp.rs`, `events.rs` | `mg N total` | Whole-tank TAN store. | Water column | Stored as elemental N total, not `NH4+`/`NH3` ion mass and not concentration. |
| `nitrite_mg_n_total` | `crates/tank_core/src/types/water.rs:13` | `mg N total` | Whole-tank nitrite-as-nitrogen store. | Water column | Snapshot/API names read like `NO2- mg/L`, but storage is `mg N total`. |
| `nitrate_mg_n_total` | `crates/tank_core/src/types/water.rs:14` | `mg N total` | Whole-tank nitrate-as-nitrogen store. | Water column | Snapshot/API names read like `NO3- mg/L`, but storage is `mg N total`. |
| `phosphate_mg_p_total` | `crates/tank_core/src/types/water.rs:15` | `mg P total` | Whole-tank orthophosphate proxy used by plants and algae. | Water column | Stored as elemental P total, not `PO4` ion mass. |
| `dissolved_oxygen_mg_total` | `crates/tank_core/src/types/water.rs:16` | `mg O2 total` | Whole-tank dissolved-oxygen mass. | Water column | Well-mixed total; no gas-phase O2 store and no supersaturation cap beyond Euler-step heuristics. |
| `dissolved_inorganic_carbon_mg_c_total` | `crates/tank_core/src/types/water.rs:17` | `mg C total` | Lumped DIC store used by plant/algae growth and pH shortcut. | Water column | Not split into `CO2(aq)`, `HCO3-`, and `CO3--`; pH derives from a shortcut, not equilibrium speciation. |
| `dissolved_organic_carbon_mg_c_total` | `crates/tank_core/src/types/water.rs:18` | `mg C total` | Lumped dissolved organic carbon pool produced from fine detritus and consumed by decomposers. | Water column | One bulk DOC pool; no lability classes or compartment split. |
| `dissolved_organic_nitrogen_mg_n_total` | `crates/tank_core/src/types/water.rs:19` | `mg N total` | Lumped dissolved organic nitrogen pool produced from feed/detritus dissolution and mineralized to TAN. | Water column | One bulk DON pool; no amino/urea distinction. |
| `alkalinity_meq_total` | `crates/tank_core/src/types/water.rs:20` | `meq total` | Whole-tank alkalinity store used by the pH shortcut and nitrification budget. | Water column | Stored alongside explicit bicarbonate mass; the two are only loosely coupled. |
| `calcium_mg_total`, `magnesium_mg_total` | `crates/tank_core/src/types/water.rs:21-22` | `mg total` | Whole-tank calcium and magnesium totals, mainly used for GH and shrimp mineral stress. | Water column | Only Ca and Mg feed GH; no carbonate-complexation or precipitation. |
| `sodium_mg_total`, `potassium_mg_total` | `crates/tank_core/src/types/water.rs:23-24` | `mg total` | Whole-tank sodium and potassium totals, only exposed through TDS/conductivity heuristics. | Water column | Tracked but not biologically active in current logic. |
| `bicarbonate_mg_total`, `chloride_mg_total`, `sulfate_mg_total` | `crates/tank_core/src/types/water.rs:25-27` | `mg total` | Tracked anions used for TDS/conductivity, bicarbonate bookkeeping, and future chemistry extensions. | Water column | Bicarbonate is not solved from carbonate equilibrium; chloride does not yet modulate nitrite toxicity. |
| `ph` | `crates/tank_core/src/types/water.rs:28-29`, recomputed in `chemistry.rs` and `water_change.rs` | `pH units` | Stored display/logic pH derived from alkalinity, DIC, and volume. | Water column | Derived state stored next to totals; formula is `6.3 + log10(alkalinity) - log10(DIC)` with a hard clamp to `5.5..=8.5`. |

### 1.2 Substrate and detritus storage

| Field name | Location | Unit | Meaning | Compartment | Known shortcuts |
| --- | --- | --- | --- | --- | --- |
| `SubstrateLayerState.kind` | `crates/tank_core/src/types/substrate.rs:13` | enum | Coarse substrate class used by habitat and detritus heuristics. | Substrate | Four hardcoded kinds only; no mixed mineralogy beyond layer ordering. |
| `depth_cm` | `crates/tank_core/src/types/substrate.rs:14` | `cm` | Layer thickness, used for weighted substrate indices and root habitat. | Substrate | Depth is scalar only; no explicit oxic/suboxic split or porewater volume. |
| `nutrient_store_mg_n_total` | `crates/tank_core/src/types/substrate.rs:15` | `mg N total` | Root-accessible substrate nitrogen stock. | Substrate | Stored as one bulk elemental-N reserve, not concentration or per-layer speciation. |
| `nutrient_store_mg_p_total` | `crates/tank_core/src/types/substrate.rs:16` | `mg P total` | Root-accessible substrate phosphorus stock. | Substrate | Bulk elemental-P reserve; no sorption/desorption model. |
| `cation_exchange_capacity_index` | `crates/tank_core/src/types/substrate.rs:17` | `0..1 index` | Relative substrate nutrient-holding/reactivity proxy. | Substrate | Index, not physical CEC units. |
| `detritus_trapping_index` | `crates/tank_core/src/types/substrate.rs:18` | `0..1 index` | Relative tendency to retain particulate organics and slow release. | Substrate | Heuristic trap factor; no explicit pore-space or flow model. |
| `colonizable_area_cm2` | `crates/tank_core/src/types/substrate.rs:19`, built in `tank_scenarios/src/lib.rs:630-660` | `cm^2` | Surface area assigned to the layer for periphyton/biofilm carrying-capacity heuristics. | Substrate | Computed as `footprint * colonizable_area_factor`; no direct tie to grain size, rugosity, or filter media. |
| `low_oxygen_tendency_index` | `crates/tank_core/src/types/substrate.rs:20` | `0..1 index` | Relative tendency toward low-oxygen zones. | Substrate | Currently affects warnings only; does not drive denitrification or separate chemistry. |
| `grazing_surface_index` | `crates/tank_core/src/types/substrate.rs:21` | `0..1 index` | Relative shrimp/periphyton grazing accessibility. | Substrate | Heuristic accessibility scalar, not explicit habitat geometry. |
| `particulate_organics_g_total` | `crates/tank_core/src/types/biology.rs:115` | `g total` | Coarse feed/organic particles waiting to leach into fine detritus. | Detritus | Bulk dry-mass proxy; elemental composition is inferred later from `feed_n_to_c_ratio` and `FEED_P_TO_N_MASS_RATIO`. |
| `fine_detritus_g_total` | `crates/tank_core/src/types/biology.rs:116` | `g total` | Fine organic particles available for dissolution, grazing, and filter fouling. | Detritus | Bulk mixed pool; shrimp and microfauna can remove it without explicit excretion routing today. |
| `dissolved_feed_residue_g_total` | `crates/tank_core/src/types/biology.rs:117` | `g total (feed-equivalent bookkeeping)` | Bookkeeping mirror for dissolved feed residue after fine-detritus dissolution. | Detritus / bookkeeping | Mixed with elemental DOC/DON/P pools; units are feed-equivalent grams, not elemental mass. |

### 1.3 Biology, filter, and bookkeeping state

| Field name | Location | Unit | Meaning | Compartment | Known shortcuts |
| --- | --- | --- | --- | --- | --- |
| `plant_guilds[].guild` | `crates/tank_core/src/types/biology.rs:22` | enum | Plant guild identity (`FastStem` or `RootFeedingRosette`). | Biology | Only two guilds; no per-species physiological differentiation. |
| `plant_guilds[].biomass_g` | `crates/tank_core/src/types/biology.rs:23` | `g biomass` | Standing plant biomass for each guild instance. | Biology | All scenario materialization starts each guild at `5.0 g` regardless of tank size. |
| `plant_guilds[].health_index`, `crowding_index`, `habitat_index` | `crates/tank_core/src/types/biology.rs:24-26` | `0..1 index` | Plant condition, density stress, and habitat suitability proxies. | Biology | Derived heuristics, not conserved pools; `habitat_index` is partly hardcoded (`FastStem` gets `0.95`). |
| `plant_guilds[].water_column_uptake_bias`, `substrate_uptake_bias` | `crates/tank_core/src/types/biology.rs:27-32` | relative weight | Biases that route nutrient demand between water and substrate. | Biology / routing | Biases can sum above `1.0`; code renormalizes them each step. |
| `suspended_biomass_g`, `periphyton_biomass_g` | `crates/tank_core/src/types/biology.rs:37-38` | `g biomass` | Free-water algae biomass and attached periphyton biomass. | Biology | No species split; suspended seed floor scales with volume (`0.02 * volume / 10`). |
| `nuisance_index` | `crates/tank_core/src/types/biology.rs:39` | `0..1 index` | Snapshot-friendly algae pressure summary from suspended + attached loads. | Biology / derived | Derived from bloom threshold and periphyton occupancy; not a material pool. |
| `decomposer_biomass_g`, `ammonia_oxidizer_biomass_g`, `nitrite_oxidizer_biomass_g`, `comammox_biomass_g` | `crates/tank_core/src/types/biology.rs:44-47` | `g biomass` | Microbial guild biomass pools driving mineralization and nitrification. | Biology / biofilm | Carrying capacity is hardcoded to `0.5 g` total nitrifier biomass regardless of habitat. |
| `microbe.maturity_index` | `crates/tank_core/src/types/biology.rs:48` | `0..1 index` | Stored generic microbial maturity index. | Biology / bookkeeping | Currently clamped but not consumed by runtime logic; `filter_state.biofilter_maturity_index` is the active maturity signal. |
| `microfauna.population_index`, `grazing_pressure_index` | `crates/tank_core/src/types/biology.rs:53-54` | `0..1 index` | Bulk microfauna abundance and current grazing pressure. | Biology | No explicit counts or biomass; population is smoothed from resource availability and shrimp pressure. |
| `adults_count`, `juveniles_count`, `berried_females_count` | `crates/tank_core/src/types/biology.rs:59-61` | `count` | Shrimp life-stage counts used for reproduction, feeding, and mortality. | Biology | No sex ratio or size structure beyond adult vs juvenile; adults are implicitly half female in spawning logic. |
| `condition_index`, `molt_stress_index`, `reproductive_readiness_index` | `crates/tank_core/src/types/biology.rs:62-64` | `0..1 index` | Shrimp condition, molt stress, and readiness summaries. | Biology | Derived heuristics tuned from water quality, GH, instability, and feeding; not directly measurable state. |
| `egg_progress_days`, `egg_cohorts[].count`, `egg_cohorts[].progress_days` | `crates/tank_core/src/types/biology.rs:65-68` | `days`, `count` | `egg_cohorts` are canonical clutch tracking; `egg_progress_days` is a display mirror of the most advanced cohort. | Biology / bookkeeping | `egg_progress_days` is derived, not authoritative; old saves can materialize berried females without cohorts and are migrated at runtime. |
| `hourly_nh3_stress_accum`, `hourly_nitrite_stress_accum`, `hourly_low_do_stress_accum`, `hourly_heat_stress_accum`, `hourly_instability_stress_accum` | `crates/tank_core/src/types/biology.rs:69-80` | stress score | Hidden hourly shrimp stress accumulators reset after each daily shrimp update. | Biology / bookkeeping | Not exposed in snapshot/API; save-serialized developer state. |
| `daily_food_consumed_g`, `maturation_accum` | `crates/tank_core/src/types/biology.rs:81-84` | `g/day`, fractional count | Shrimp feeding record and juvenile-to-adult fractional maturation accumulator. | Biology / bookkeeping | Used for smooth daily updates, not player-visible totals. |
| `biofilter_maturity_index`, `clogging_index`, `seeded_biomass_index` | `crates/tank_core/src/types/hardware.rs:46-48` | `0..1 index` | Filter summary state: maturity, clogging, and an extra seeding placeholder. | Biofilter / bookkeeping | `seeded_biomass_index` is stored and clamped but currently unused by simulation logic. |
| `prev_temp_c`, `prev_ph`, `prev_gh_d`, `prev_do_mg_l`, `instability_index` | `crates/tank_core/src/types/biology.rs:106-110` | mixed | Stability tracker baselines and current chemistry-instability score. | Bookkeeping | Keeps previous-day values only; instability is heuristic, not a conserved variable. |
| `event_log`, `last_event_day` | `crates/tank_core/src/types/state.rs:35-37` | event list / map | Persistent event history plus once-per-day dedupe state. | Bookkeeping | `event_log` is capped to the newest 200 entries by `invariants.rs`; snapshot/API expose only the newest 20. |

### 1.4 Hardware, environment, geometry, and save-adjacent state

| Field name | Location | Unit | Meaning | Compartment | Known shortcuts |
| --- | --- | --- | --- | --- | --- |
| `light.enabled`, `light.photoperiod_hours`, `light.intensity_index` | `crates/tank_core/src/types/hardware.rs:4-8` | bool, hours, `0..1` | Lighting schedule and relative intensity. | Hardware | Light has no spectral content or depth attenuation; plants normalize photoperiod to `10 h`, algae to `9 h`. |
| `heater.enabled`, `heater.setpoint_c`, `heater.deadband_c`, `heater.max_watts`, `heater.efficiency`, `heater.last_output_w` | `crates/tank_core/src/types/hardware.rs:11-19` | bool, `deg C`, `deg C`, `W`, fraction, `W` | Heater control state and last realized heater output. | Hardware | Heater power is not scaled by geometry during scenario startup; thermostat can heat but never cool. |
| `filter.enabled`, `filter.flow_lph`, `filter.cleanliness_index` | `crates/tank_core/src/types/hardware.rs:23-28` | bool, `L/h`, `0..1` | Mechanical/filter-flow state and cleanliness. | Hardware | Startup overrides leave enabled filters at `200 L/h` unless explicitly set; flow acts as a simple ratio to volume. |
| `aeration.enabled`, `aeration.intensity` | `crates/tank_core/src/types/hardware.rs:31-34` | bool, `0..1` | Aeration toggle and relative intensity. | Hardware | Scenario startup sets enabled aeration to a fixed `0.35`; aeration changes DO only, not CO2 stripping. |
| `environment.ambient_temp_c`, `hour_of_day`, `day` | `crates/tank_core/src/types/environment.rs:5-7` | `deg C`, hour, day | Room temperature and simulation clock. | Environment | Single ambient temperature only; no seasonal/light-cycle variation beyond fixed photoperiod. |
| `geometry.length_cm`, `width_cm`, `height_cm`, `fill_height_cm` | `crates/tank_core/src/types/geometry.rs:7-10` | `cm` | Tank dimensions that determine water volume, footprint, wall area, and exchange ratios. | Geometry | Rectangular prism only; substrate displacement, hardscape displacement, and curved tanks are ignored. |
| `geometry.glass_thickness_mm`, `open_top`, `lid_exchange_factor` | `crates/tank_core/src/types/geometry.rs:11-14` | `mm`, bool, fraction | Additional physical setup knobs. | Geometry | `glass_thickness_mm` is stored but not used in heat-transfer equations; startup materialization hardcodes `open_top=true` and `glass_thickness_mm=5.0`. |
| `source_water_catalog` | `crates/tank_core/src/types/state.rs:39-40`, `crates/tank_core/src/types/source_water.rs` | map of per-liter profiles | Runtime catalog of water-change source profiles. | Bookkeeping / input state | Saved for determinism; not a live in-tank pool. |
| `process_params`, `shrimp_params` | `crates/tank_core/src/types/state.rs:41-48` | structured coefficients | Runtime coefficient packs used by system steps. | Bookkeeping / parameter state | Saved verbatim; `ProcessParams::default()` is not identical to the shipped process preset because DIC rates stay `0.0` until materialized from TOML. |

### 1.5 Exported observables and snapshot/API surface

| Field name | Location | Unit | Meaning | Compartment | Known shortcuts |
| --- | --- | --- | --- | --- | --- |
| `tan_mg_l`, `nh3_mg_l`, `nitrite_mg_l`, `nitrate_mg_l`, `phosphate_mg_l`, `dissolved_inorganic_carbon_mg_l`, `do_mg_l`, `do_sat_mg_l`, `ph` | `crates/tank_core/src/types/snapshot.rs:13-25`, forwarded by `crates/tank_api/src/handlers/snapshot.rs:13-47` | per-liter chemistry plus pH | Per-liter chemistry projection of `WaterState` totals plus pH and saturation DO. | Snapshot / API | Most names omit the underlying element basis (`mg N/L`, `mg P/L`, `mg C/L`); `nh3_mg_l` is also derived from TAN-as-N. |
| `gh_d`, `kh_d`, `tds_mg_l`, `conductivity_us_cm` | `crates/tank_core/src/types/snapshot.rs:21-24, 73-80` | degrees hardness, `mg/L`, `uS/cm` | Derived hardness and ionic-strength proxy outputs. | Snapshot / API | GH/KH are conversion heuristics; TDS only sums 7 tracked ions; conductivity is estimated as `tds / 0.65`. |
| `adult_shrimp_count`, `juveniles_count`, `berried_females_count`, `shrimp_condition_index`, `shrimp_molt_stress_index`, `shrimp_reproductive_readiness`, `microfauna_population_index`, `microfauna_grazing_pressure_index` | `crates/tank_core/src/types/snapshot.rs:34-41`, forwarded by `tank_api` biology endpoint | counts and indices | Player-facing biology summary. | Snapshot / API | Population stages are coarse; no hidden stress accumulators or egg cohorts are exposed. |
| `total_plant_biomass_g`, `fast_stem_biomass_g`, `fast_stem_health_index`, `root_feeding_rosette_biomass_g`, `root_feeding_rosette_health_index`, `suspended_algae_biomass_g`, `periphyton_biomass_g`, `algae_nuisance_index` | `crates/tank_core/src/types/snapshot.rs:42-49` | `g`, `0..1` | Aggregated plant/algae outputs. | Snapshot / API | Guild biomasses are sums; health values are averages across matching guild entries, not direct stored fields. |
| `detritus_particulate_g_total`, `detritus_fine_g_total`, `substrate_nutrient_remaining_mg_n_total`, `substrate_nutrient_remaining_mg_p_total` | `crates/tank_core/src/types/snapshot.rs:50-53, 145-156` | `g total`, `mg total` | Exported long-horizon maintenance and substrate-depletion observables. | Snapshot / API | Substrate outputs are total sums across all layers, hiding layer-level differences. |
| `biofilter_maturity_index`, `ammonia_oxidizer_biomass_g`, `nitrite_oxidizer_biomass_g`, `comammox_biomass_g`, `decomposer_biomass_g` | `crates/tank_core/src/types/snapshot.rs:54-58`, `tank_api` biofilter endpoint | `0..1`, `g` | Exported biofilter and microbial summary. | Snapshot / API | No habitat-level split between filter media, walls, or substrate biofilms. |
| `last_heater_output_w`, `recent_events` | `crates/tank_core/src/types/snapshot.rs:59-60, 99-104, 162-163` | `W`, list | Latest heater output and newest 20 events. | Snapshot / API | `recent_events` is a truncated view of `event_log`; save data can contain up to 200 events. |

### 1.6 Storage-vs-display semantics that are easy to misread

| Internal storage | Snapshot/API/exported name | What is actually stored or shown today | Misread risk |
| --- | --- | --- | --- |
| `WaterState` dissolved fields in saves | `SaveFile.state.water.*` | Save JSON stores whole-tank totals exactly as fields are named in `WaterState`, not per-liter concentrations. | Any consumer reading save JSON without dividing by `geometry.water_volume_l()` will over-read concentrations. |
| `ammonia_total_mg_n_total` | `tan_mg_l` | `mg N/L` after dividing whole-tank TAN by volume. | Low risk: `tan` name is explicit, but still elemental N. |
| `ammonia_total_mg_n_total` + pH/temp | `nh3_mg_l` | Free-ammonia fraction of TAN, still computed from TAN-as-N. | Field name reads like molecular `NH3 mg/L`; code computes `mg N/L as NH3-N`. |
| `nitrite_mg_n_total` | `nitrite_mg_l` | `mg N/L`. | Name reads like `NO2- mg/L`. |
| `nitrate_mg_n_total` | `nitrate_mg_l` | `mg N/L`. | Name reads like `NO3- mg/L`. |
| `phosphate_mg_p_total` | `phosphate_mg_l` | `mg P/L`. | Name reads like `PO4 mg/L`. |
| `dissolved_inorganic_carbon_mg_c_total` | `dissolved_inorganic_carbon_mg_l` | `mg C/L` in one lumped DIC pool. | Name reads like total dissolved carbonate species by molecular mass. |
| `dissolved_oxygen_mg_total` | `do_mg_l`, `do_sat_mg_l` | Actual dissolved O2 per liter plus a temperature-table saturation estimate. | `do_sat_mg_l` is a lookup/interpolation, not a full gas-exchange equilibrium output. |
| `alkalinity_meq_total`, `calcium_mg_total`, `magnesium_mg_total` | `kh_d`, `gh_d` | KH and GH are derived from alkalinity and Ca/Mg using fixed conversion factors. | Names look authoritative, but they are display conversions over a simplified ion set. |
| `calcium + magnesium + sodium + potassium + bicarbonate + chloride + sulfate` | `tds_mg_l`, `conductivity_us_cm` | Tracked-major-ion TDS proxy and conductivity estimate. | Reads like full TDS/conductivity; both omit untracked solutes and use a fixed `0.65` divisor. |
| `plant_guilds`, `event_log` | `*_biomass_g`, `*_health_index`, `recent_events` | Snapshot sums or averages guild state and truncates events to the last 20 items. | Consumers can mistake snapshot fields for canonical storage rather than projections. |

## 2. Rate / Parameter Catalog

Two important runtime facts before the catalog:

- The effective shipped runtime coefficient pack is `crates/tank_data/data/process/default.toml`, materialized through `tank_scenarios::process_preset_to_params`.
- `ProcessParams::default()` mirrors most TOML defaults, but it leaves `respiration_dic_rate_mg_c_per_g_per_hour` and `photosynthesis_dic_rate_mg_c_per_g_per_hour` at `0.0`. Scenario materialization overwrites them with `0.08` and `0.12` from TOML.

The TOML files already carry file-level `[provenance]` blocks, but not per-parameter provenance/unit metadata. That gap is what `G1 / tanksim-6e5.7.1` is meant to close.

### 2.1 Data-backed runtime coefficients

| Parameter name | Location | Unit | Implicit assumptions | Volume- or area-dependent? | Downstream beads affected |
| --- | --- | --- | --- | --- | --- |
| `mineralization_rate_per_day`, `nitrification_vmax` | `crates/tank_core/src/types/process.rs:11-14`, `crates/tank_data/data/process/default.toml:3-4` | per day | Reserved placeholders only; current systems ignore them in favor of guild-specific coefficients. | No | `G1` (document or remove), `A2` (migration if removed) |
| `reaeration_kla_base`, `aeration_kla_boost` | `process.rs:15-16`, TOML `5-6`, used in `dissolved_oxygen.rs:27-33` | effective `1/h` transfer factor | Treated as hourly Euler-step coefficients; top exchange multiplies by `geometry.top_exchange_factor()`, filter flow adds a separate fixed boost. | Yes: depends on top-exchange factor and optional aeration intensity, not directly on measured gas-transfer area | `B2`, `D2`, `E1`, `G1` |
| `background_bod_mg_o2_per_g_biomass_per_hour`, `plant_photosynthesis_o2_mg_per_g_per_hour` | `process.rs:17-18`, TOML `7-8`, used in `dissolved_oxygen.rs:35-50` | `mg O2 / g biomass / h` | All respiring biomass shares one background O2 demand; all photosynthetic biomass shares one O2 production rate scaled only by light intensity. | Indirectly: totals scale with biomass, not water volume | `C1`, `D2`, `G1` |
| `respiration_dic_rate_mg_c_per_g_per_hour`, `photosynthesis_dic_rate_mg_c_per_g_per_hour` | `process.rs:19-20, 135-136`, TOML `9-10`, used in `chemistry.rs:34-55` | `mg C / g biomass / h` | Struct defaults are `0.0`, but shipped TOML overrides to nonzero values; photosynthesis scales with light intensity only. | Indirectly: totals scale with biomass, not water volume | `D1`, `D2`, `G1` |
| `k_surface_w_per_m2_k`, `k_wall_w_per_m2_k` | `process.rs:22-24`, TOML `11-12`, used in `temperature.rs:51-59` | `W / (m^2 * K)` | Single bulk-water heat-transfer model; wall term ignores `glass_thickness_mm` and material. | Yes: multiplied by surface/wall area from geometry | `E1`, `G1` |
| `feed_leach_rate_per_hour`, `fine_detritus_dissolution_rate_per_hour`, `feed_n_to_c_ratio` | `process.rs:28-32`, TOML `15-17`, used in `nitrogen_cycle.rs:47-73` | `1/h`, `1/h`, `mg N / mg C` | Feed organic chemistry is one generic composition; phosphorus enters via a hardcoded `P:N` ratio, not a separate data-backed feed profile. | No direct volume dependency | `C1`, `G1` |
| `decomposer_vmax_per_hour`, `decomposer_k_doc_mg`, `decomposer_growth_yield`, `decomposer_decay_rate_per_hour` | `process.rs:36-42`, TOML `20-23`, used in `nitrogen_cycle.rs:74-119` | per hour, `mg C total`, `g/g`, `1/h` | DOC half-saturation is expressed in whole-tank `mg`, not `mg/L`; decomposer DO limitation also uses whole-tank DO. | Yes: `decomposer_k_doc_mg` and the DO term both smuggle volume assumptions | `B2`, `C1`, `G1` |
| `aob_vmax_mg_n_per_g_per_hour`, `nob_vmax_mg_n_per_g_per_hour`, `comammox_vmax_fraction` | `process.rs:46-75`, TOML `26-44`, used in `nitrogen_cycle.rs:137-220` | `mg N / g biomass / h`, fraction | One shared set of biomass-normalized nitrifier kinetics; comammox is derived as a fixed fraction of AOB vmax. | Indirectly: rate multiplies biomass, but saturation terms below still use totals | `B2`, `E1`, `G1` |
| `aob_k_tan_mg`, `aob_k_do_mg`, `nob_k_nitrite_mg`, `nob_k_do_mg`, `comammox_k_tan_mg`, `comammox_k_do_mg` | `process.rs:47-72`, TOML `27-42`, used in `nitrogen_cycle.rs:139-179, 212-219` | `mg total` | All nitrifier half-saturation constants are expressed as whole-tank totals, so same concentration behaves differently in different tank volumes. | Yes: explicitly volume-dependent today | `B1`, `B2`, `D2`, `E1`, `G1` |
| `aob_growth_yield`, `aob_decay_rate_per_hour`, `nob_growth_yield`, `nob_decay_rate_per_hour`, `comammox_growth_yield`, `comammox_decay_rate_per_hour` | `process.rs:51-76`, TOML `29-44`, used in `nitrogen_cycle.rs:252-271` | `g biomass / mg N`, `1/h` | Growth is later suppressed by a hardcoded global carrying capacity rather than habitat area. | No direct volume dependency | `C1`, `E1`, `G1` |
| `o2_per_mg_n_nitrified`, `alkalinity_meq_per_mg_n_nitrified` | `process.rs:79-82`, TOML `47-48`, used in `nitrogen_cycle.rs:128-136, 273-283` | `mg O2 / mg N`, `meq / mg N` | Only comammox O2 cost is data-backed; AOB/NOB O2 costs are hardcoded nearby. | No | `C1`, `D2`, `G1` |
| `plant_max_growth_rate_fast_stem_per_day`, `plant_max_growth_rate_root_rosette_per_day` | `process.rs:85-86`, TOML `51-52`, used in `plant_growth.rs:58-72` | `1/day` | Two guild-level maxima only; plant preset `growth_rate_index` is not currently applied. | No direct volume dependency | `E1`, `G1` |
| `plant_respiration_fraction_per_day`, `plant_senescence_fraction_per_day`, `plant_health_recovery_per_day`, `plant_health_decline_per_day` | `process.rs:87-90`, TOML `53-56`, used in `plant_growth.rs:73-137` | fraction/day | One set of turnover/health coefficients shared by both guilds. | No | `C1`, `E1`, `G1` |
| `plant_half_saturation_n_mg_total`, `plant_half_saturation_p_mg_total`, `plant_half_saturation_c_mg_total` | `process.rs:91-93`, TOML `57-59`, used in `plant_growth.rs:22-25, 47-57` | `mg total` | N, P, and C limitation are based on whole-tank accessible totals, not concentrations or root-zone areal exposure. | Yes: explicitly volume-dependent today; substrate share also hides compartment assumptions | `B1`, `B2`, `E1`, `G1` |
| `plant_light_half_saturation`, `plant_temp_optimum_c`, `plant_temp_sigma_c`, `plant_crowding_biomass_g_per_m2` | `process.rs:94-97`, TOML `60-63`, used in `plant_growth.rs:7-21, 141-150` | dimensionless, `deg C`, `deg C`, `g/m^2` | Light factor multiplies intensity by `photoperiod / 10`; crowding is area-normalized but all starting biomasses are fixed at `5 g/guild`. | Yes: crowding uses surface area; light normalization is not depth-aware | `B2`, `E1`, `G1` |
| `algae_max_growth_rate_per_day`, `periphyton_max_growth_rate_per_day`, `algae_respiration_fraction_per_day` | `process.rs:100-102`, TOML `66-68`, used in `algae_growth.rs:43-52, 114-123` | `1/day`, `1/day`, fraction/day | Suspended and attached algae share one nutrient/temperature formulation with different capacities and seeds. | No direct volume dependency | `E1`, `G1` |
| `algae_half_saturation_n_mg_total`, `algae_half_saturation_p_mg_total` | `process.rs:103-104`, TOML `69-70`, used in `algae_growth.rs:31-38` | `mg total` | Same whole-tank-total problem as plant and nitrifier half-saturation terms. | Yes: explicitly volume-dependent today | `B1`, `B2`, `G1` |
| `algae_light_half_saturation`, `algae_temp_optimum_c`, `algae_temp_sigma_c`, `periphyton_capacity_g_per_m2`, `algae_bloom_threshold_g_per_l`, `algae_nuisance_biomass_g_per_m2` | `process.rs:105-110`, TOML `71-76`, used in `algae_growth.rs:100-183` | mixed | Periphyton uses colonizable area but suspended algae use water volume; bloom and nuisance outputs are proxy thresholds, not validated ecological carrying capacities. | Mixed: area-dependent for periphyton capacity and nuisance, volume-dependent for bloom threshold | `B2`, `E1`, `E4`, `G1` |
| `shrimp_base_mortality_per_day`, `shrimp_stress_mortality_scale`, `shrimp_juvenile_maturation_days`, `shrimp_periphyton_grazing_g_per_shrimp_per_day`, `shrimp_condition_smoothing` | `process.rs:113-118`, defaults `196-200`, used in `shrimp.rs` | mixed | Shared generic husbandry coefficients for all shrimp species; feeding uses attached algae/fine detritus only. | No direct volume dependency, but stress terms read concentrations after manual division by volume | `C1`, `F-phase`, `G1` |
| `microfauna_mineralization_boost`, `microfauna_periphyton_consumption`, `microfauna_population_smoothing`, `microfauna_shrimp_pressure_threshold` | `process.rs:120-123`, defaults `202-205`, used in `nitrogen_cycle.rs:85-88` and `microfauna.rs` | mixed | Microfauna are modeled as a smoothed index with one shared grazing profile; no explicit biomass. | Threshold uses shrimp per liter; other terms use normalized indices | `C1`, `E1`, `G1` |
| `ShrimpRuntimeParams` from `neocaridina_davidi.toml` | `crates/tank_core/src/types/biology.rs:90-101`, `crates/tank_data/data/shrimp/neocaridina_davidi.toml:4-13` | mixed | Species pack covers temperature window, GH window, spawn rate, egg duration, hatch success, juvenile sensitivity, and high-temp penalty thresholds only. | No | `F-phase`, `G1` |

### 2.2 Hardcoded scientific constants and heuristics

| Parameter name | Location | Unit | Implicit assumptions | Volume- or area-dependent? | Downstream beads affected |
| --- | --- | --- | --- | --- | --- |
| `FEED_P_TO_N_MASS_RATIO = 0.10` | `crates/tank_core/src/systems/nitrogen_cycle.rs:3, 66` | `mg P / mg N` | Every dissolved feed-derived organic pulse shares one fixed P:N ratio. | No | `C1`, `G1` |
| `PLANT_N_MG_PER_G_GROWTH = 28`, `PLANT_P_MG_PER_G_GROWTH = 4`, `ALGAE_N_MG_PER_G_GROWTH = 35`, `ALGAE_P_MG_PER_G_GROWTH = 5` | `plant_growth.rs:3-4`, `algae_growth.rs:6-7` | `mg element / g biomass growth` | Fixed stoichiometry across all plant and algae guilds. | No | `C1`, `E1`, `G1` |
| `o2_for_aob = 3.43`, `o2_for_nob = 1.14` | `crates/tank_core/src/systems/nitrogen_cycle.rs:132-135` | `mg O2 / mg N step` | Only the AOB and NOB step costs are hardcoded; comammox uses TOML. | No | `C1`, `D2`, `G1` |
| `temperature_factor opt=28 sigma=10`, `ph_factor opt=7.5 sigma=1.5`, decomposer DO half-sat `2.0` | `nitrogen_cycle.rs:81-90, 293-308` | `deg C`, `pH`, `mg total` | Shared Gaussian response curves for all nitrifier guilds plus one decomposer DO curve; DO half-sat again uses whole-tank totals. | Yes for the DO half-sat | `B2`, `D2`, `G1` |
| Nitrifier carrying capacity `capacity_g = 0.5` | `nitrogen_cycle.rs:246, 359-364` | `g biomass` | Global biofilter ceiling independent of media area, substrate, or flow history. | No explicit geometry dependency | `E1`, `G1` |
| Plant habitat coefficients (`FastStem -> 0.95`; rosette `0.2 + 0.35*depth + 0.3*CEC + 0.15 if active`) | `crates/tank_core/src/systems/plant_growth.rs:153-185` | dimensionless | Habitat quality is a simple weighted formula over depth, CEC, and active-substrate presence. | Yes: depth and substrate layer setup matter; no explicit root-zone area | `E1`, `G1` |
| Plant photoperiod normalization `/10.0`; algae photoperiod normalization `/9.0`; plant shading multiplier `0.7` with clamp `0.2..=1.0` | `plant_growth.rs:146`, `algae_growth.rs:22-30, 191-195` | hours normalization / fraction | Different reference photoperiods are used without a scientific contract; shading is driven by plant crowding only. | Area/crowding dependent only through plant biomass and surface area | `B2`, `E1`, `G1` |
| Suspended-algae seed floor `0.02 * volume / 10`, periphyton grazing fractions `0.03` and `0.06` | `algae_growth.rs:43, 52, 123` | `g`, fraction/day | Algae cannot fully disappear from the suspended pool because the seed scales with tank volume; microfauna grazing rates are hardcoded. | Yes for the suspended seed floor | `B2`, `E1`, `G1` |
| Shrimp biomass constants `0.12 g adult`, `0.05 g juvenile`; `juveniles_per_clutch = 25`; juvenile feeding weight `0.3`; max daily grazing fractions `50%` periphyton, `20%` fine detritus | `nitrogen_cycle.rs:4-5`, `chemistry.rs:3-4`, `shrimp.rs:119-147, 374-377` | mixed | Fixed body sizes and clutch size for all runs; food handling is by removal only, not mass-routed outputs. | No | `C1`, `F-phase`, `G1` |
| Stability and event thresholds: NH3 `0.02`, nitrite `0.5`, low DO `4.0`/`5.0`, instability normalization `temp/3 + pH/0.5 + GH/3 + DO/3` | `events.rs:17-43`, `shrimp.rs:27-50, 99-115` | mixed | Warning thresholds and instability scoring are built into systems rather than data files. | Concentration-based after manual volume division | `B2`, `F-phase`, `G1` |
| pH shortcut constants `6.3`, alkalinity floor `0.05 meq/L`, DIC floor `0.02 mmol/L`, clamp `5.5..=8.5` | `crates/tank_core/src/systems/chemistry.rs:6-20` | mixed | One log-linear carbonate shortcut for every source water and every runtime state. | Yes: depends on total-to-volume conversion | `D1`, `D2`, `G1` |
| DO saturation lookup table `(0,14.6) (10,11.3) (20,9.1) (30,7.6) (40,6.4)` | `crates/tank_core/src/systems/temperature.rs:3-31` | `mg/L` | Linear interpolation over a small fixed table. | Temperature-dependent only | `D2`, `G1` |
| GH/KH/TDS/conductivity conversion constants `2.497`, `4.118`, `17.848`, `50.0`, `0.65` | `crates/tank_core/src/types/snapshot.rs:73-80`, `shrimp.rs:87-90`, `biology.rs:223-226` | conversion factors | GH and KH are back-computed from stored totals; conductivity is a fixed multiple of TDS. | Yes: depend on total-to-volume conversion | `B1`, `B7`, `G1` |

### 2.3 Startup, materialization, and dormant inputs that affect semantics

| Parameter name | Location | Unit | Implicit assumptions | Volume- or area-dependent? | Downstream beads affected |
| --- | --- | --- | --- | --- | --- |
| `growth_rate_index` in plant presets | `crates/tank_data/data/plants/*.toml`, `crates/tank_data/src/presets.rs:90-98` | index | Data file carries a plant growth-rate hint, but materialization and runtime growth ignore it today. | No | `G1`, `E1` |
| `colonizable_area_factor`, `nutrient_charge_mg_*_total` in substrate presets | `crates/tank_data/data/substrate/*.toml`, materialized in `tank_scenarios/src/lib.rs:630-660` | factor, `mg total` | Colonizable area scales with footprint; nutrient charge is absolute at base size and later rescaled only by footprint overrides. | Yes: colonizable area and override scaling depend on footprint | `E1`, `E4`, `G1` |
| Scenario materialization defaults: `5.0 g` per plant guild, `200 L/h` enabled filter flow, `0.35` aeration intensity, `5 mm` glass, `open_top = true` | `tank_scenarios/src/lib.rs:581-614, 668-705, 455-463` | mixed | Tank-size scaling is incomplete: startup hardware and plant biomass are fixed templates rather than geometry-normalized values. | Mixed: geometry changes do not automatically rescale these defaults | `E1`, `A2`, `G1` |
| `microbe.maturity_index`, `filter_state.seeded_biomass_index`, `glass_thickness_mm` | `types/biology.rs:48`, `types/hardware.rs:48`, `types/geometry.rs:11`, usage search as of 2026-03-27 | mixed | Stored state exists, but runtime systems do not currently consume these values meaningfully. | No | `A2`, `E1`, `G1` |

## 3. Invariant Catalog

### 3.1 What `crates/tank_core/src/invariants.rs` enforces today

| Invariant | Current enforcement | Location | Notes |
| --- | --- | --- | --- |
| Non-negative dissolved water totals | Hard error for TAN, nitrite, nitrate, phosphate, DO, DIC, DOC, DON, alkalinity, major ions | `crates/tank_core/src/invariants.rs:4-37` | DO is first floored to `>= 0` if finite, then checked again. |
| Non-negative detritus and biomass/storage pools | Hard error for detritus pools, microbial biomasses, plant biomass, algae biomass, substrate depth/nutrient stores/colonizable area | `invariants.rs:38-79` | Covers the major stateful masses but not explicit total-N/total-C sums. |
| Positive temperature and finite pH | Temperature must be finite and `> 0`; pH must be finite and is then clamped to `5.5..=8.5` | `invariants.rs:80-94` | pH is range-clamped rather than validated against carbonate chemistry. |
| Geometry sanity | `length_cm`, `width_cm`, `height_cm`, and `fill_height_cm` must be positive; `fill_height_cm <= height_cm` | `invariants.rs:95-105` | No check on unrealistic aspect ratios or substrate displacement. |
| Index clamping | `lid_exchange_factor`, light intensity, filter cleanliness, aeration intensity, filter-state indices, algae nuisance, microbial maturity, microfauna indices, shrimp condition/molt/repro indices, plant health/crowding/habitat, substrate CEC/detritus/low-O2/grazing indices are clamped to `0..1` | `invariants.rs:107-145` | Many heuristics are silently clamped rather than rejected. |
| Shrimp population bookkeeping | `daily_food_consumed_g >= 0`, `berried_females_count <= adults_count`, `egg_progress_days >= 0`, empty egg cohorts removed | `invariants.rs:126-133` plus `AnimalState::clamp_berried_to_adults` | Keeps display/cohort state internally consistent enough to continue sim steps. |
| Selected process parameters must be non-negative | `reaeration_kla_base`, `aeration_kla_boost`, `k_surface_w_per_m2_k`, `k_wall_w_per_m2_k`, background BOD, decomposer vmax, AOB/NOB vmax, DIC rates, plant photosynthesis O2 rate | `invariants.rs:147-181` | This is only a subset of `ProcessParams`; many other coefficients have no runtime-range checks beyond data-load validation. |
| Event-log retention | Keep only the newest 200 events | `invariants.rs:183-186` | Snapshot/API further truncate to the newest 20 events. |

### 3.2 Missing invariants the roadmap needs

| Missing invariant | Why it matters | Expected owner |
| --- | --- | --- |
| Explicit total-N, total-P, total-C, and explicit-import/export budget checks across water, substrate, detritus, plants, algae, microbes, and shrimp | Later routing work cannot safely refactor consumers, trimming, denitrification, or save migrations without knowing whether matter is created or destroyed. | `A2 / tanksim-6e5.1.2`, `C1 / tanksim-6e5.3.1` |
| Canonical concentration / compartment invariants (`mg/L`, `mg/cm^2`) derived from totals in one place | Today the same concentration behaves differently in different tank volumes because half-saturation terms use totals. | `B1 / tanksim-6e5.2.1`, `B2 / tanksim-6e5.2.2` |
| Storage/display consistency for nitrogen, phosphate, DIC, TDS, conductivity, and free-ammonia outputs | Snapshot/API users need machine-checkable semantics, not just best-effort names. | `B7 / tanksim-6e5.2.7` |
| Carbonate consistency between DIC, alkalinity, bicarbonate, pH, and gas exchange | Current pH is just a bounded shortcut. Future chemistry needs invariants that reflect equilibrium and degassing/uptake. | `D1 / tanksim-6e5.4.1`, `D2 / tanksim-6e5.4.2` |
| Habitat-based carrying capacity and colonizable-area invariants for nitrifiers, periphyton, and substrate surfaces | Current `0.5 g` biofilter cap and generic colonizable area hide where microbes actually live. | `E1 / tanksim-6e5.5.1` |
| Oxic/suboxic substrate and denitrification invariants | `low_oxygen_tendency_index` is currently descriptive only. A redox-aware substrate model needs its own state and sanity checks. | `E4 / tanksim-6e5.5.4` |
| Cross-field synchronization for placeholder/derived state (`microbe.maturity_index` vs `filter_state.biofilter_maturity_index`, `seeded_biomass_index`, `egg_progress_days`) | Several serialized fields are informational or derived today. Later save/schema work needs an explicit contract for which fields are canonical. | `A2`, `A2c / tanksim-6e5.1.2.3`, `E1` |
| Scenario/startup scaling invariants | Geometry overrides currently rescale substrate nutrient charge by footprint, but not plant biomass, filter flow, heater power, or aeration defaults. | `E1`, `A2` |

## 4. Known Nonphysical Behaviors / Shortcuts

| Behavior / shortcut | Severity | Evidence | Expected owner |
| --- | --- | --- | --- |
| Monod and half-saturation terms use whole-tank totals instead of concentrations (`mg/L`) or areal exposure (`mg/cm^2`) | Scientifically wrong | `plant_growth.rs`, `algae_growth.rs`, `nitrogen_cycle.rs` all use totals such as `ammonia_total_mg_n_total`, `dissolved_organic_carbon_mg_c_total`, and `nitrate_mg_n_total` directly in saturation terms. | `B2 / tanksim-6e5.2.2` |
| Shrimp grazing removes periphyton and fine detritus without routing consumed mass to feces, excretion, respiration, or shrimp biomass | Scientifically wrong | `crates/tank_core/src/systems/shrimp.rs:119-147` | `C1 / tanksim-6e5.3.1` |
| Microfauna grazing likewise removes periphyton and fine detritus without a closed mass loop | Scientifically wrong | `crates/tank_core/src/systems/microfauna.rs:45-55` | `C1 / tanksim-6e5.3.1` |
| `TrimPlants` always leaves cut biomass in-tank as fine detritus | Design debt | `crates/tank_core/src/engine.rs:189-197` | `C1 / tanksim-6e5.3.1` |
| pH is computed from a log-linear shortcut rather than carbonate equilibrium | Scientifically wrong | `crates/tank_core/src/systems/chemistry.rs:6-20` | `D2 / tanksim-6e5.4.2` |
| Aeration changes DO but not CO2 stripping or pH | Scientifically wrong | `dissolved_oxygen.rs` updates only O2; `chemistry.rs` has no gas exchange term for DIC/CO2 | `D2 / tanksim-6e5.4.2` |
| Bicarbonate mass and alkalinity are both stored explicitly but are only loosely coupled | Design debt | `WaterState` stores both, but only nitrification and water changes update them together. | `D1 / tanksim-6e5.4.1`, `D2 / tanksim-6e5.4.2` |
| Snapshot/API names hide element-basis chemistry (`nitrite_mg_l`, `nitrate_mg_l`, `phosphate_mg_l`, `nh3_mg_l`) | Design debt | `crates/tank_core/src/types/snapshot.rs`, `crates/tank_api/src/handlers/snapshot.rs` | `B7 / tanksim-6e5.2.7` |
| TDS and conductivity are heuristics over 7 tracked ions with a fixed divisor | Design debt | `snapshot.rs:76-80`, `water.rs:96-104` | `B7 / tanksim-6e5.2.7` |
| Nitrifier carrying capacity is hardcoded to `0.5 g` total biomass | Design debt | `nitrogen_cycle.rs:246, 359-364` | `E1 / tanksim-6e5.5.1` |
| Tank-size scaling is incomplete: geometry overrides rescale substrate nutrients but not plant biomass, filter flow, heater power, or aeration defaults | Design debt | `tank_scenarios/src/lib.rs:241-260, 581-614, 668-705` | `E1 / tanksim-6e5.5.1` |
| No denitrification or explicit suboxic substrate chemistry despite `low_oxygen_tendency_index` | Design debt | Substrate state carries `low_oxygen_tendency_index`, but runtime usage is limited to warnings/heuristics. | `E4 / tanksim-6e5.5.4` |
| Nitrite stress ignores chloride protection even though chloride is tracked in water state | Design debt | `shrimp.rs` reads nitrite directly; no chloride modifier exists. | Future shrimp/toxicology phase |
| No depth-based light attenuation | Cosmetic | Light response uses intensity, photoperiod, and plant crowding only; depth is ignored after geometry-derived area terms. | `E-phase` follow-on |
| Dormant or unused scientific fields remain serialized (`growth_rate_index`, `microbe.maturity_index`, `seeded_biomass_index`, `glass_thickness_mm`) | Design debt | Search usage as of 2026-03-27 shows they are loaded/stored but not meaningful in core runtime equations. | `G1 / tanksim-6e5.7.1`, `E1 / tanksim-6e5.5.1` |
| `ProcessParams::default()` chemistry behavior differs from the shipped TOML pack because DIC rates stay `0.0` until scenario materialization loads TOML | Cosmetic | `types/process.rs:135-136` vs `data/process/default.toml:9-10` | `D1 / tanksim-6e5.4.1`, `G1 / tanksim-6e5.7.1` |
| `dissolved_feed_residue_g_total` mixes feed-equivalent grams with elemental DOC/DON/P pools | Design debt | `nitrogen_cycle.rs:61-73, 110-113` | `C1 / tanksim-6e5.3.1` |

## 5. Test Dependency Map

As of 2026-03-27 the suite contains 21 test files: 19 in `crates/tank_core/tests/` and 2 in `crates/tank_scenarios/tests/`.

Legend:

- `deterministic lock`: expects exact equality for serialized state/snapshot or exact error types.
- `numeric threshold`: expects a specific ratio, tolerance, or minimum gap.
- `qualitative / envelope`: checks direction, relative ordering, viability, or story outcome.

| Test file | What it currently locks | Lock type | Likely breakage surface | Future stance |
| --- | --- | --- | --- | --- |
| `crates/tank_core/tests/ambient_temperature_action.rs` | Temperature warms gradually toward ambient, DO saturation falls with warming, heater output is zero above setpoint and positive below it | Qualitative / threshold | Thermal model tuning, heater control changes, area/exchange changes | Keep qualitative; use envelopes rather than exact trajectories |
| `crates/tank_core/tests/chemistry.rs` | Exact NH3 formula match, pH clamp bounds, alkalinity unchanged by DIC-only photosynthesis/respiration, nitrification lowers alkalinity/pH | Mixed exact + qualitative | Carbonate-state rewrite, naming/unit changes, DIC coupling changes | Keep pure-formula checks exact; convert dynamic pH behavior to chemistry envelopes |
| `crates/tank_core/tests/cycling.rs` | Earlier stabilization for seeded tank, non-negative pools under limiters, filter-cleaning setback, low-DO/low-alkalinity caps, dirty-filter slowdown | Mixed threshold + invariant | Unit normalization, conservation fixes, habitat-aware filter rewrite | Keep limiter/non-negative guards strict; make time-to-stable and peak comparisons envelope-based |
| `crates/tank_core/tests/determinism.rs` | Full snapshot JSON and full state equality after a 720-hour scripted journey | Deterministic lock | Any arithmetic change, field rename, event ordering change, or coefficient change | Move to harness-level determinism (`A2`) rather than using it as a scientific-behavior lock |
| `crates/tank_core/tests/dissolved_oxygen.rs` | Night DO lower than lit DO, aeration recovers faster, DO reaches within `0.25 mg/L` of saturation after 24h | Numeric threshold / envelope | DO/gas-exchange retuning, biomass-coupling changes | Keep as envelope tests; retain the saturation sanity bound |
| `crates/tank_core/tests/end_to_end.rs` | Happy/sad scenario stories stay finite and directionally sensible; one mini-journey checks exact deterministic snapshot fields | Mostly qualitative / envelope with one deterministic subtest | Any user-visible scenario tuning, snapshot field renames, changed defaults | Keep story checks qualitative; move the exact mini-determinism case into shared harness work |
| `crates/tank_core/tests/events.rs` | Specific warning/info event kinds appear, dedupe is once-per-day, cause-code lists are non-empty | Event contract | Event threshold retuning or event taxonomy changes | Keep mostly exact on dedupe and cause-code presence; scenario-trigger expectations can stay qualitative |
| `crates/tank_core/tests/filter_cleanliness_decay.rs` | 30 days of feeding lowers cleanliness below `0.99`, more than unfed, and raises clogging above `0` | Numeric threshold | Filter fouling formula changes, maintenance-flow changes | Keep as broad envelope, not as a calibration anchor |
| `crates/tank_core/tests/overfeeding_effects.rs` | Overfeeding yields more detritus, lower nightly DO minimum, and higher algae nuisance than control | Qualitative / envelope | Conservation routing, DO tuning, algae retuning | Keep qualitative |
| `crates/tank_core/tests/plant_algae_integration.rs` | Good conditions produce materially more plant growth, guild uptake direction matches biases, bloom events trigger, trim turns `10 g -> 7.5 g` plant and `2.5 g` detritus | Mixed threshold + exact mass routing | Unit normalization, habitat model, trim/export redesign | Keep trim-routing exact until `C1` splits export vs in-tank trim; make growth/algae outcomes envelope-based |
| `crates/tank_core/tests/rooted_substrate_advantage.rs` | Active substrate gives at least `15%` more rosette biomass and depletes substrate N below `90` | Numeric threshold | Habitat registry, substrate chemistry rewrite, unit normalization | Convert to qualitative substrate-envelope checks after `E1`/`E4` |
| `crates/tank_core/tests/save_load.rs` | Save roundtrip, queued-action roundtrip, and resumed-vs-uninterrupted runs are exactly equal | Deterministic contract | Save schema changes, migration work, serialization renames | Keep exact for same-schema behavior; add migration-specific tests under `A2c` |
| `crates/tank_core/tests/shrimp_population.rs` | Good conditions produce berried events and growth, stress causes egg failure/mortality, removal validation errors are exact, population indices stay in range | Mixed qualitative + exact contract | Shrimp life-history rewrite, closed-loop feeding, new toxicity model | Keep validation and failure-resolution exact; long-horizon reproduction/mortality should stay envelope-based |
| `crates/tank_core/tests/substrate_filter_integration.rs` | Substrate type changes particulate retention and TAN exposure, grazing surface improves shrimp condition, enabled filter lowers TAN/nitrite, higher flow lowers exposure and keeps TAN under `20 mg/L` | Numeric threshold / envelope | Habitat registry, area scaling, nitrifier carrying-capacity rewrite | Convert numeric percentages to broader envelopes after `E1` |
| `crates/tank_core/tests/tank_size_thermal_response.rs` | Geometry volumes are exact to `0.1 L`, small tanks reach half-temperature change in less than half the ticks, open tops warm faster, `10 L` peak TAN is at least `5x` `100 L` | Mixed exact geometry + numeric threshold | Unit normalization and tank-size scaling changes | Keep geometry/materialization exact; convert thermal and TAN scaling assertions to envelopes after `B2`/`E1` |
| `crates/tank_core/tests/thermal_reproduction_penalty.rs` | `30 C` tank ends with at least `20%` lower readiness and fewer juveniles than `25 C` tank | Numeric threshold / envelope | Shrimp thermal curve retuning, improved life-history model | Keep as envelope, not a hard calibration point |
| `crates/tank_core/tests/validation.rs` | Exact action-validation errors and queue behavior for water-change profile validation | Deterministic contract | API/action contract changes | Keep exact |
| `crates/tank_core/tests/water_change_do_mixing.rs` | 50% water change mixes DO totals within `1%` of the expected weighted total | Exact numeric contract | Water-change routing or DO bookkeeping changes | Keep exact |
| `crates/tank_core/tests/water_change_mass_balance.rs` | 50% `ro_like` water change halves dissolved totals within `0.5%`, `0%` is a no-op, temperature mixing stays in bounds, invalid profiles leave state unchanged exactly | Exact numeric contract | Water-change semantics, source-water validation, save/action ordering changes | Keep exact |
| `crates/tank_scenarios/tests/materialization.rs` | Scenario geometry, volume, preset loading, deterministic materialization, geometry overrides, and cycling fixtures are exact | Deterministic contract | Scenario data edits, materialization mapping changes, startup defaults | Keep exact for scenario assembly semantics |
| `crates/tank_scenarios/tests/startup_overrides.rs` | Startup overrides set exact heater/shrimp values and replace source-water totals exactly | Deterministic contract | Startup-override API changes, source-water materialization changes | Keep exact |

Tests most likely to become qualitative envelope checks once the scientific core changes are underway:

- `determinism.rs`
- Dynamic portions of `chemistry.rs`
- Time-to-stable and peak-comparison portions of `cycling.rs`
- Growth/ecology thresholds in `plant_algae_integration.rs`, `rooted_substrate_advantage.rs`, `substrate_filter_integration.rs`, `tank_size_thermal_response.rs`, and `thermal_reproduction_penalty.rs`
- User-journey assertions in `end_to_end.rs`

Tests that should remain exact contracts even after major refactors:

- `save_load.rs`
- `validation.rs`
- `water_change_mass_balance.rs`
- `water_change_do_mixing.rs`
- `materialization.rs`
- `startup_overrides.rs`

## 6. Downstream Bead Handoff

| Bead | Most relevant sections |
| --- | --- |
| `A2 / tanksim-6e5.1.2` and `A2c / tanksim-6e5.1.2.3` | Section 1.3-1.6 for derived-vs-canonical save state, Section 3 for missing invariants, Section 5 for exact save/determinism/materialization locks |
| `B1 / tanksim-6e5.2.1` | Section 1.1 and 1.6 for totals-vs-display semantics, Section 2.1 and 2.2 for unit-bearing parameters, Section 4 rows on concentration misuse and snapshot naming |
| `B2 / tanksim-6e5.2.2` | Section 1.1-1.2 for which pools need canonical concentration helpers, Section 2.1-2.2 for volume-smuggling coefficients, Section 3 missing concentration invariants, Section 4 row on Monod/half-saturation misuse |
| `B7 / tanksim-6e5.2.7` | Section 1.5-1.6 for snapshot/API semantics, Section 4 rows on ambiguous chemistry labels and TDS/conductivity honesty, Section 5 snapshot/materialization contract tests |
| `C1 / tanksim-6e5.3.1` | Section 1.2-1.3 for current matter-bearing pools, Section 2.1-2.2 for stoichiometric constants, Section 3 missing conservation invariants, Section 4 rows on grazing/trim shortcuts |
| `D1 / tanksim-6e5.4.1` and `D2 / tanksim-6e5.4.2` | Section 1.1 and 1.6 for DIC/alkalinity/bicarbonate semantics, Section 2.1-2.2 for chemistry and gas-exchange coefficients, Section 3 carbonate gaps, Section 4 rows on pH shortcut and no CO2 stripping |
| `E1 / tanksim-6e5.5.1` | Section 1.2-1.4 for substrate/filter/hardware geometry state, Section 2.2-2.3 for carrying-capacity and startup-scaling shortcuts, Section 3 habitat gaps, Section 4 rows on hardcoded biofilter capacity and incomplete scaling |
| `E4 / tanksim-6e5.5.4` | Section 1.2 for current substrate fields, Section 2.2 for low-O2 and habitat heuristics, Section 3 missing redox invariants, Section 4 row on missing denitrification |
| `G1 / tanksim-6e5.7.1` | Section 2 in full, plus Section 4 rows on dormant fields and file-level-only provenance; this is the map of what still needs per-parameter provenance and unit metadata |
