# Aquarium Simulation Review and vNext Plan

## Overall assessment

The codebase is architecturally solid for a v0.1 ecosystem simulator:
- clean workspace split (`tank_core`, `tank_data`, `tank_scenarios`, `tank_tui`, `tank_api`)
- deterministic seed/save-load support
- first-class geometry, source-water profiles, and ambient temperature actions
- useful regression tests already covering cycling, oxygen, shrimp reproduction, tank-size thermal response, water changes, and substrate effects

The main weaknesses are not in the crate structure but in the scientific core:
1. several kinetics are based on **total mass** rather than **concentration**
2. multiple food/grazing pathways delete biomass without returning excretion/waste
3. pH / carbonate chemistry is oversimplified enough that different source waters collapse to nearly the same pH
4. several important aquarium interactions are still missing (CO2 gas exchange, habitat-specific biofilms, denitrification, chloride protection against nitrite, explicit animal excretion)

## High-priority issues

### 1) Total-mass kinetics instead of concentration kinetics
Files:
- `crates/tank_core/src/systems/nitrogen_cycle.rs`
- `crates/tank_core/src/systems/plant_growth.rs`
- `crates/tank_core/src/systems/algae_growth.rs`
- `crates/tank_core/src/types/process.rs`

Examples:
- `decomposer_k_doc_mg`, `aob_k_tan_mg`, `aob_k_do_mg`, `nob_k_nitrite_mg`, `plant_half_saturation_n_mg_total`, `algae_half_saturation_n_mg_total` are all expressed in absolute totals.
- This makes a larger tank with the same concentration appear less nutrient-limited than a smaller tank.

### 2) Matter sinks in shrimp and microfauna feeding
Files:
- `crates/tank_core/src/systems/shrimp.rs`
- `crates/tank_core/src/systems/microfauna.rs`

Current behavior:
- shrimp remove periphyton and fine detritus, but that consumed mass is not returned as feces, DOC, DON, TAN, or biomass
- microfauna consume periphyton and detritus, again largely deleting matter

Result:
- nutrient loops are broken
- grazing can make the tank look cleaner than it should while underestimating bioload

### 3) Source-water pH profiles are not really differentiated
Files:
- `crates/tank_core/src/systems/chemistry.rs`
- `crates/tank_core/src/types/water.rs`
- `crates/tank_data/data/source_water/*.toml`

Current pH solver:
- `pH = 6.3 + log10(alkalinity_meq_l) - log10(dic_mmol_l)` (clamped)

With shipped presets this collapses to roughly:
- `hard_shrimp`: ~6.31
- `moderate`: ~6.30
- `soft_acidic`: ~6.33
- `ro_like`: ~6.70

So the named water chemistries differ in GH/KH/TDS, but not meaningfully in pH.

### 4) Snapshot/UI unit ambiguity
Files:
- `crates/tank_core/src/types/water.rs`
- `crates/tank_core/src/types/snapshot.rs`

Internally many nitrogen pools are stored as `mg N`, but snapshot fields are named `nitrite_mg_l` / `nitrate_mg_l` and will read like full-ion ppm to users.

### 5) TDS/conductivity are undercounted
Files:
- `crates/tank_core/src/types/water.rs`
- `crates/tank_core/src/types/snapshot.rs`

Current TDS only sums Ca, Mg, Na, K, HCO3, Cl, sulfate.
Missing from displayed TDS include nitrate, ammonium/ammonia, phosphate, DOC contribution, and any future fertilizers.

### 6) Biofilter carrying capacity does not scale with habitat
Files:
- `crates/tank_core/src/systems/nitrogen_cycle.rs`

Current implementation uses a fixed nitrifier capacity of `0.5 g`, regardless of tank size, media volume, colonizable surface, or flow.

### 7) Equipment and biomass are not consistently scaled when geometry scales
Files:
- `crates/tank_scenarios/src/lib.rs`

Geometry scaling correctly changes dimensions and substrate nutrient stores, but filter flow, heater power, initial plant mass, and initial stock do not automatically scale with tank size.

### 8) Trimming adds all cut biomass back into detritus
Files:
- `crates/tank_core/src/engine.rs`

Current `TrimPlants` turns all trimmed biomass into fine detritus.
In real maintenance, trimming usually exports most biomass from the system.

## Medium-priority scientific gaps

- no explicit animal excretion model
- no explicit CO2(aq) / HCO3- / CO3-- state model
- aeration changes DO but not CO2 stripping or pH
- no denitrification in low-oxygen substrate zones
- no chloride mitigation of nitrite toxicity
- no light attenuation by depth/turbidity despite geometry carrying depth
- rooted plants do not have per-layer root-zone oxygen/redox effects
- periphyton is a single undifferentiated biomass pool
- no explicit surface habitats (glass, substrate, plant leaves, filter media)

## Recommended vNext milestones

### v0.2 — Conservation + concentration normalization
- convert Monod/half-saturation constants to concentration units
- add helper APIs for compartment concentrations
- add shrimp excretion, feces, and assimilation fractions
- add microfauna assimilation/excretion
- rename snapshot outputs to `*_mg_n_l` where applicable, or convert to ion units for display
- include more dissolved species in TDS/conductivity accounting
- split `TrimPlants` into `TrimAndRemove` vs `TrimAndLeaveCuttings`

### v0.3 — Carbonate / CO2 realism
- explicit pools for CO2(aq), HCO3-, CO3--, alkalinity
- gas exchange for both O2 and CO2
- source water should include either pH or pCO2 / equilibrium target
- optional CO2 injection hardware
- pH recalculation from carbonate equilibrium instead of the current shortcut

### v0.4 — Habitatized biofilm and substrate ecology
- separate habitats: filter media, glass, hardscape, plant surfaces, substrate top, substrate deep
- habitat-specific colonizable area and flow / oxygen multipliers
- oxic vs suboxic substrate layers
- denitrification in low-oxygen zones
- periphyton and decomposer pools per habitat

### v0.5 — Better shrimp ecology
- explicit ingestion -> assimilation -> excretion -> feces
- age/stage and size structure
- mineral budget for molt success
- egg success affected by temperature, instability, condition, and maybe osmotic stress
- stocking density and sex-ratio effects

### v0.6 — Validation harness
- regression scenarios tied to literature and expected qualitative behaviors
- parameter provenance fields on major biological/chemical parameters
- calibration reports that compare simulated outcomes to target envelopes

## Suggested backlog order

1. **Fix concentration-vs-total kinetics**
2. **Fix mass conservation for grazing and feeding**
3. **Fix output units and TDS display**
4. **Replace pH shortcut with carbonate/CO2 model**
5. **Scale biofilter carrying capacity and equipment with geometry**
6. **Add habitat-specific biofilm / denitrification**
7. **Deepen shrimp life-history realism**

## Concrete first sprint for vNext

### Ticket 1: unit cleanup
- introduce newtypes for `MassMgN`, `MassMgIon`, `ConcentrationMgL`, `VolumeL`
- rename fields and snapshot labels

### Ticket 2: concentration helpers
- add helper methods:
  - `tank.conc_tan_mg_n_l()`
  - `tank.conc_no3_mg_n_l()`
  - `tank.conc_no3_mg_l_as_ion()`
  - `compartment.conc_do_mg_l()`

### Ticket 3: mass-conserving animal loop
- shrimp grazing returns:
  - assimilated fraction -> condition/biomass reserve
  - excreted dissolved N -> TAN
  - feces -> fine detritus
  - respiration -> O2 demand / DIC

### Ticket 4: carbonate system
- add `co2_aq_mg_c_total`, `hco3_mg_total`, `co3_mg_total`
- compute pH from carbonate equilibrium each hour
- link aeration to CO2 stripping

### Ticket 5: habitat model
- create `HabitatId` and separate areas/oxygenation for:
  - filter_media
  - glass
  - plant_surfaces
  - substrate_surface
  - substrate_deep

### Ticket 6: validation suite
- add tests for:
  - same concentration in different tank sizes => same Monod factor
  - grazing conserves N/C mass
  - high-aeration strips CO2 and raises pH when CO2 is high
  - chloride reduces nitrite hazard at equal nitrite concentration
  - deeper tanks receive lower average plant light than shallow tanks under same lamp setting
