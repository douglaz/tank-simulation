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

### 1) Total-mass kinetics instead of concentration kinetics — RESOLVED

*Implemented in tanksim-6e5.2.3 through 6e5.2.5. All half-saturation parameters renamed to concentration-based (`_mg_n_per_l`, `_mg_per_l`, etc.). See [UNITS.md](UNITS.md) parameter rename table for the complete migration.*

Files:
- `crates/tank_core/src/systems/nitrogen_cycle.rs`
- `crates/tank_core/src/systems/plant_growth.rs`
- `crates/tank_core/src/systems/algae_growth.rs`
- `crates/tank_core/src/types/process.rs`

Examples:
- ~~`decomposer_k_doc_mg`, `aob_k_tan_mg`, `aob_k_do_mg`, `nob_k_nitrite_mg`, `plant_half_saturation_n_mg_total`, `algae_half_saturation_n_mg_total`~~ are all now concentration-based with basis-explicit names.
- This makes a larger tank with the same concentration appear less nutrient-limited than a smaller tank.

### 2) Matter sinks in shrimp and microfauna feeding — PARTIALLY RESOLVED

*Shrimp mass routing implemented in tanksim-6e5.6.x: consumed mass now returns through feces, DOC/TAN excretion, respiration, and reserve. Microfauna still use a simpler approximation. See [ROUTING.md](ROUTING.md) for the canonical routing contract.*

Files:
- `crates/tank_core/src/systems/shrimp.rs`
- `crates/tank_core/src/systems/microfauna.rs`

Original behavior:
- shrimp now route consumed periphyton and fine detritus back through feces, dissolved waste, respiration, and body reserve
- microfauna consume periphyton and detritus with a simpler but improved approximation

Result:
- shrimp nutrient loops are now closed; microfauna routing remains approximate

### 3) Source-water pH profiles are not really differentiated — RESOLVED

*Implemented in tanksim-6e5.4.x: the log-linear pH shortcut has been replaced by a closed-form quadratic carbonate equilibrium solver with temperature-corrected pKa1 (Harned & Davis 1943). Source waters now produce meaningfully different pH values. See [carbonate_state_contract.md](carbonate_state_contract.md).*

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

### 4) Snapshot/UI unit ambiguity — RESOLVED

*Implemented in tanksim-6e5.2.7: all snapshot/API chemistry fields renamed to basis-explicit names (e.g., `tan_mg_n_per_l`, `nitrite_mg_n_per_l`). Legacy aliases maintained for backward compatibility. TUI labels carry the correct basis. See [UNITS.md](UNITS.md) snapshot field table.*

Files:
- `crates/tank_core/src/types/water.rs`
- `crates/tank_core/src/types/snapshot.rs`

Internally many nitrogen pools are stored as `mg N`, but snapshot fields are named `nitrite_mg_l` / `nitrate_mg_l` and will read like full-ion ppm to users.

### 5) TDS/conductivity are undercounted — RESOLVED (labeling)

*Implemented in tanksim-6e5.2.7: TDS and conductivity are now explicitly labeled as estimates everywhere — TUI shows "Est. TDS (7-ion)" and "Est. conductivity", API fields are `estimated_tds_7_ion_mg_per_l` and `estimated_conductivity_us_cm`, and the API response includes `estimated_tds_scope` documenting which ions are tracked and what is omitted.*

Files:
- `crates/tank_core/src/types/water.rs`
- `crates/tank_core/src/types/snapshot.rs`

Current TDS only sums Ca, Mg, Na, K, HCO3, Cl, sulfate.
Missing from displayed TDS include nitrate, ammonium/ammonia, phosphate, DOC contribution, and any future fertilizers.

### 6) Biofilter carrying capacity does not scale with habitat — RESOLVED

*Implemented in tanksim-6e5.5.x: a five-zone habitat registry now computes colonizable area and exposure modifiers (flow, oxygen, light) from geometry, hardware, and plant biomass. See `crates/tank_core/src/types/habitat.rs`.*

Files:
- `crates/tank_core/src/systems/nitrogen_cycle.rs`

Current implementation uses a fixed nitrifier capacity of `0.5 g`, regardless of tank size, media volume, colonizable surface, or flow.

### 7) Equipment and biomass are not consistently scaled when geometry scales
Files:
- `crates/tank_scenarios/src/lib.rs`

Geometry scaling correctly changes dimensions and substrate nutrient stores, but filter flow, heater power, initial plant mass, and initial stock do not automatically scale with tank size.

### 8) Trimming adds all cut biomass back into detritus — RESOLVED

*Implemented: the action model now distinguishes `TrimPlantsAndRemove` (exports biomass from the system) from `TrimPlantsAndLeaveCuttings` (adds to fine detritus). The TUI exposes both as "Trim & remove" and "Trim & leave".*

Files:
- `crates/tank_core/src/engine.rs`

Current `TrimPlants` turns all trimmed biomass into fine detritus.
In real maintenance, trimming usually exports most biomass from the system.

## Medium-priority scientific gaps

- ~~no explicit animal excretion model~~ — **RESOLVED**: shrimp excretion routes TAN, DOC, feces, and respiration
- ~~no explicit CO2(aq) / HCO3- / CO3-- state model~~ — **RESOLVED**: carbonate equilibrium solver derives speciation each step
- ~~aeration changes DO but not CO2 stripping or pH~~ — **RESOLVED**: aeration now affects both O2 and CO2/pH through the carbonate solver
- ~~no denitrification in low-oxygen substrate zones~~ — **RESOLVED**: denitrification implemented with suboxic substrate zone support
- ~~no chloride mitigation of nitrite toxicity~~ — **RESOLVED**: chloride protection factor modulates effective nitrite hazard
- ~~no light attenuation by depth/turbidity despite geometry carrying depth~~ — **RESOLVED**: Beer-Lambert light attenuation implemented with extinction coefficient
- rooted plants do not have per-layer root-zone oxygen/redox effects — *still planned*
- periphyton is a single undifferentiated biomass pool — *still planned*
- ~~no explicit surface habitats (glass, substrate, plant leaves, filter media)~~ — **RESOLVED**: five-zone habitat registry implemented

## Recommended vNext milestones — implementation status

### v0.2 — Conservation + concentration normalization — DONE (tanksim-6e5.2.x)
- convert Monod/half-saturation constants to concentration units
- add helper APIs for compartment concentrations
- add shrimp excretion, feces, and assimilation fractions
- add microfauna assimilation/excretion
- rename snapshot outputs to `*_mg_n_l` where applicable, or convert to ion units for display
- include more dissolved species in TDS/conductivity accounting
- split `TrimPlants` into `TrimAndRemove` vs `TrimAndLeaveCuttings`

### v0.3 — Carbonate / CO2 realism — DONE (tanksim-6e5.4.x)
- explicit pools for CO2(aq), HCO3-, CO3--, alkalinity
- gas exchange for both O2 and CO2
- source water should include either pH or pCO2 / equilibrium target
- optional CO2 injection hardware
- pH recalculation from carbonate equilibrium instead of the current shortcut

### v0.4 — Habitatized biofilm and substrate ecology — DONE (tanksim-6e5.5.x)
- separate habitats: filter media, glass, hardscape, plant surfaces, substrate top, substrate deep
- habitat-specific colonizable area and flow / oxygen multipliers
- oxic vs suboxic substrate layers
- denitrification in low-oxygen zones
- periphyton and decomposer pools per habitat

### v0.5 — Better shrimp ecology — DONE (tanksim-6e5.6.x)

The prescriptive state-model contract for the stage-structured shrimp redesign now lives in [shrimp_state_contract.md](shrimp_state_contract.md).

- explicit ingestion -> assimilation -> excretion -> feces
- age/stage and size structure (juvenile / sub-adult / adult per the state contract)
- mineral budget for molt success (extension point defined in the state contract)
- egg success affected by temperature, instability, condition, and maybe osmotic stress
- stocking density and sex-ratio effects

### v0.6 — Validation harness — DONE (tanksim-6e5.7.x)
- ~~regression scenarios tied to literature and expected qualitative behaviors~~ — 8 validation scenarios shipped in [validation_scenarios.md](validation_scenarios.md)
- ~~parameter provenance fields on major biological/chemical parameters~~ — `ParamMeta` infrastructure with 4-tier confidence, documented in [PROVENANCE_STATUS.md](PROVENANCE_STATUS.md)
- ~~calibration reports that compare simulated outcomes to target envelopes~~ — calibration-report workflow implemented in `tank_harness`

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
