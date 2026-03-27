# Unit Policy

This document is the prescriptive unit and naming policy for the chemistry-facing parts of the simulator. It turns the descriptive findings in [scientific_inventory.md](scientific_inventory.md) into rules that future refactors must follow across engine state, snapshots, API responses, TUI labels, save files, presets, and tests. The companion routing policy for how those quantities move between pools lives in [ROUTING.md](ROUTING.md).

## Goals

- Keep conserved state cheap to update and easy to reason about.
- Make element-basis versus ion-basis values explicit in every name and label.
- Let the simulator serve both scientific and hobby audiences without mixing those two reporting styles in the same field.
- Prevent semantic drift in future save files, API payloads, and preset data.

## Canonical Quantity Taxonomy

Use the quantity basis directly in the identifier.

| Pattern | Meaning | Examples |
| --- | --- | --- |
| `_mg_n_total` | whole-tank elemental nitrogen mass | `ammonia_total_mg_n_total`, `nitrite_mg_n_total` |
| `_mg_p_total` | whole-tank elemental phosphorus mass | `phosphate_mg_p_total`, `nutrient_store_mg_p_total` |
| `_mg_c_total` | whole-tank elemental carbon mass | `dissolved_inorganic_carbon_mg_c_total`, `dissolved_organic_carbon_mg_c_total` |
| `_mg_total` | whole-tank mass where the stored basis is already the tracked species or ion | `dissolved_oxygen_mg_total`, `calcium_mg_total`, `bicarbonate_mg_total` |
| `_meq_total` | whole-tank charge-equivalent store | `alkalinity_meq_total` |
| `_mg_n_per_l`, `_mg_p_per_l`, `_mg_c_per_l`, `_mg_per_l` | concentration projection or source-water input | `ammonia_mg_n_per_l`, `calcium_mg_per_l` |
| `_mmol_per_l` | derived molar concentration used for equilibrium/speciation views | `co2_aq_mmol_per_l`, `hco3_mmol_per_l`, `co3_mmol_per_l` |
| `_per_hour`, `_per_day`, `_per_g_per_hour`, `_g_per_l`, `_g_per_m2` | rate or density parameter units | `feed_leach_rate_per_hour`, `aob_vmax_mg_n_per_g_per_hour`, `algae_bloom_threshold_g_per_l` |

Rules:

- Never use bare `_mg_l` for nitrogen-, phosphorus-, or carbon-basis chemistry. Those names hide whether the value means `mg N/L`, `mg P/L`, `mg C/L`, or full-ion `mg/L`.
- Keep abbreviations only when they are already domain-standard and the basis is still explicit: `tan_mg_n_per_l`, `gh_d`, `kh_d`.
- Carbonate speciation that exists only as a solver output uses molar names such as `co2_aq_mmol_per_l`, not ad hoc `mg/L` identifiers. If a UI wants `mg/L` later, that conversion belongs in the presentation layer.
- Index, count, boolean, and temperature fields stay unit-specific without `_total`: `ph`, `temperature_c`, `cleanliness_index`, `adults_count`.

## Internal Storage Rules

### 1. Whole-tank state stays in totals

Canonical persistent state keeps conserved or quasi-conserved pools as whole-tank totals, not concentrations. This matches the current `WaterState`, `SubstrateLayerState`, detritus, and many biology stores, and it avoids re-multiplying by volume whenever a system updates mass.

Current canonical internal examples:

- `WaterState` totals in `crates/tank_core/src/types/water.rs`
- substrate nutrient totals in `crates/tank_core/src/types/substrate.rs`
- detritus and biomass totals in `crates/tank_core/src/types/biology.rs`

This bead does not change the storage basis. Future chemistry or habitat work can add helper views, but the authoritative state remains whole-tank totals unless a later bead makes an explicit schema change.

Approved carbonate exception:

- `dissolved_inorganic_carbon_mg_c_total` and `alkalinity_meq_total` remain the authoritative carbonate stores.
- `ph` and `bicarbonate_mg_total` are derived or cached projections.
- Speciated carbonate outputs such as `co2_aq_mmol_per_l` are helper/view values, not canonical saved state.

### 2. Source-water and preset inputs stay per liter

Input profiles that describe incoming water chemistry use per-liter names, then materialize into totals by multiplying by `geometry.water_volume_l()`. `SourceWaterProfile` already follows this policy and remains the model for future input data.

Examples in `crates/tank_core/src/types/source_water.rs`:

- `ammonia_mg_n_per_l`
- `nitrate_mg_n_per_l`
- `phosphate_mg_p_per_l`
- `dic_mg_c_per_l`
- `calcium_mg_per_l`

### 3. Save files store canonical totals plus geometry

Save files serialize `TankState`, so the canonical persisted chemistry remains the internal total-based fields plus `geometry.water_volume_l()` derivation inputs. Save payloads should not duplicate concentration mirrors alongside those totals; duplicated totals and concentrations would drift.

Policy for save/schema work:

- `SaveFile.state.water.*` remains total-based and basis-explicit.
- Concentrations are re-derived from saved totals and saved geometry on load or snapshot projection.
- Snapshot/API renames that affect serialized structs require a deliberate migration step, not silent semantic reuse.

### 4. Parameter names must encode the quantity they expect

Rates already encode units well. The remaining problem area is half-saturation and threshold parameters that still use total-mass units for what should become concentration-based kinetics.

Legacy total-based parameter names that downstream work must migrate:

| Current field | Canonical target name | Notes |
| --- | --- | --- |
| `decomposer_k_doc_mg` | `decomposer_k_doc_mg_c_per_l` | concentration-normalized DOC Monod term |
| `aob_k_tan_mg` | `aob_k_tan_mg_n_per_l` | TAN as elemental N |
| `aob_k_do_mg` | `aob_k_do_mg_per_l` | dissolved oxygen mass concentration |
| `nob_k_nitrite_mg` | `nob_k_nitrite_mg_n_per_l` | nitrite as elemental N |
| `nob_k_do_mg` | `nob_k_do_mg_per_l` | dissolved oxygen mass concentration |
| `comammox_k_tan_mg` | `comammox_k_tan_mg_n_per_l` | TAN as elemental N |
| `comammox_k_do_mg` | `comammox_k_do_mg_per_l` | dissolved oxygen mass concentration |
| `plant_half_saturation_n_mg_total` | `plant_half_saturation_n_mg_n_per_l` | water-column elemental N exposure |
| `plant_half_saturation_p_mg_total` | `plant_half_saturation_p_mg_p_per_l` | water-column elemental P exposure |
| `plant_half_saturation_c_mg_total` | `plant_half_saturation_c_mg_c_per_l` | water-column DIC as elemental C |
| `algae_half_saturation_n_mg_total` | `algae_half_saturation_n_mg_n_per_l` | water-column elemental N exposure |
| `algae_half_saturation_p_mg_total` | `algae_half_saturation_p_mg_p_per_l` | water-column elemental P exposure |

Those renames belong to `tanksim-6e5.2.2`. This policy exists so the rename target is fixed before that refactor starts.

## Helper Naming Policy

### 1. Total-to-concentration helpers

Helpers that divide a stored total by water volume must expose the basis in the function name:

- `ammonia_mg_n_per_l(volume_l)`
- `nitrite_mg_n_per_l(volume_l)`
- `nitrate_mg_n_per_l(volume_l)`
- `phosphate_mg_p_per_l(volume_l)`
- `dissolved_inorganic_carbon_mg_c_per_l(volume_l)`

Guidelines:

- Put these helpers on `WaterState` or on a dedicated concentration-view type created from `WaterState`.
- Helpers that derive a concentration from a total require `volume_l` explicitly unless they are called from a type that already owns canonical volume.
- Avoid helper names that hide the basis, such as `nitrite_mg_l()` or `phosphate_ppm()`.

Carbonate-equilibrium helpers follow the same rule:

- `co2_aq_mmol_per_l(volume_l)`
- `hco3_mmol_per_l(volume_l)`
- `co3_mmol_per_l(volume_l)`

Those are derived views over canonical DIC + alkalinity state, not new persisted stores.

### 2. Pure conversion helpers

Element-to-ion or element-to-molecule display conversions should be pure functions collected in one conversion module. The destination basis belongs in the function name.

Examples:

- `nitrate_mg_no3_per_l(mg_n_per_l)`
- `nitrite_mg_no2_per_l(mg_n_per_l)`
- `phosphate_mg_po4_per_l(mg_p_per_l)`
- `nh3_mg_nh3_per_l(mg_n_per_l)`

Canonical fixed conversion factors:

- nitrate: `mg NO3/L = mg N/L * (62.0 / 14.0)` ≈ `4.43`
- nitrite: `mg NO2/L = mg N/L * (46.0 / 14.0)` ≈ `3.29`
- phosphate: `mg PO4/L = mg P/L * (95.0 / 31.0)` ≈ `3.06`
- free ammonia: `mg NH3/L = mg NH3-N/L * (17.0 / 14.0)` ≈ `1.21`

Important TAN exception:

- `TAN` is stored and displayed scientifically as elemental nitrogen by default.
- Do not invent a single fixed "TAN as ion" conversion. Total ammonia includes both `NH3` and `NH4+`, so a full-ion display requires explicit speciation rather than one shared multiplier.

## Snapshot, API, and TUI Display Policy

### 1. Default display style is scientific

The default public chemistry surface uses scientific element-basis concentrations:

- TAN: `mg N/L`
- free ammonia: `mg NH3-N/L`
- nitrite: `mg N/L`
- nitrate: `mg N/L`
- phosphate: `mg P/L`
- dissolved inorganic carbon: `mg C/L`
- dissolved oxygen: `mg/L`

That policy matches the current internal basis, avoids false precision, and keeps the API aligned with literature-facing calculations.

### 2. Field names and labels must carry the basis

Snapshot and API field names must be basis-explicit for N/P/C chemistry.

Canonical snapshot/API names for `tanksim-6e5.2.7`:

| Current field | Canonical field | Canonical label |
| --- | --- | --- |
| `tan_mg_l` | `tan_mg_n_per_l` | `TAN (mg N/L)` |
| `nh3_mg_l` | `nh3_mg_n_per_l` | `Free NH3-N (mg NH3-N/L)` |
| `nitrite_mg_l` | `nitrite_mg_n_per_l` | `Nitrite (mg N/L)` |
| `nitrate_mg_l` | `nitrate_mg_n_per_l` | `Nitrate (mg N/L)` |
| `phosphate_mg_l` | `phosphate_mg_p_per_l` | `Phosphate (mg P/L)` |
| `dissolved_inorganic_carbon_mg_l` | `dissolved_inorganic_carbon_mg_c_per_l` | `DIC (mg C/L)` |

Bare `mg/L` labels remain fine for species already stored and displayed as the tracked mass itself, such as dissolved oxygen and the tracked major ions.

Carbonate-equilibrium debug or snapshot fields should keep their molar basis in the name:

- `co2_aq_mmol_per_l`
- `hco3_mmol_per_l`
- `co3_mmol_per_l`

If `bicarbonate_mg_total` continues to appear in state or debugging output, treat it as a derived cache sourced from `hco3_mmol_per_l`, not as an independent chemistry input.

### 3. Hobby-style ion display is opt-in and clearly marked

The simulator serves two audiences, so an alternate display mode is acceptable, but it must stay opt-in and presentation-only.

Policy:

- The canonical stored values and canonical API field names stay scientific.
- TUI and other presentation clients may offer an ion-style toggle for user-facing display.
- If the HTTP API later exposes alternate display projections, use an explicit presentation control such as `unit_style=ion` and keep the canonical response schema scientific by default.
- Ion-style labels must use the full destination basis, for example `mg NO3/L`, `mg NO2/L`, `mg PO4/L`, `mg NH3/L`.
- Do not reuse the scientific field name when serving an ion-style projection.

### 4. Estimated TDS and conductivity must say they are estimates

Current TDS and conductivity are heuristics built from seven tracked ions and one fixed divisor. That is useful, but it is not a full dissolved-solids or conductivity model.

Tracked ions in the current estimate:

- calcium
- magnesium
- sodium
- potassium
- bicarbonate
- chloride
- sulfate

Required labeling:

- TUI labels should read `Est. TDS (7-ion)` or equivalent.
- Conductivity labels should read `Est. conductivity`.
- API and snapshot renames should make the approximation explicit: `estimated_tds_7_ion_mg_per_l` and `estimated_conductivity_us_cm`.
- Any tooltip, docs text, or endpoint description should note the current formula: `estimated conductivity = estimated_tds_7_ion_mg_per_l / 0.65`.

### 5. GH and KH stay derived display values

GH and KH remain common user-facing terms, but labels should acknowledge their derivation:

- `GH (Ca+Mg)`
- `KH (alkalinity)`

These are display conversions over the tracked ion set, not new canonical storage pools.

### 6. Precision defaults

Use a stable display precision policy unless a downstream UI has a stronger reason:

- pH: `2` decimals
- scientific N/P/C concentrations: `3` decimals by default
- free ammonia warnings: up to `5` decimals when needed
- GH and KH: `1` decimal
- estimated TDS and estimated conductivity: `0` decimals in summaries, finer precision only in debugging views

## Type-Safety Strategy

### Phase 1: naming discipline first

Immediate scope is naming discipline plus documentation. This is the right first move because the codebase already has meaningful semantics in names, and the highest-risk confusion comes from a small number of ambiguous concentration fields.

Phase 1 rules:

- prefer basis-explicit names over wrapper proliferation
- centralize conversion constants instead of duplicating literals
- make review reject new bare `_mg_l` chemistry names for N/P/C values

### Phase 2: targeted newtypes at the highest-risk boundaries

Add a small number of newtypes only where naming alone cannot prevent common mistakes:

- total versus concentration, for example `MgElementTotal` versus `MgElementPerL`
- elemental concentration versus ion/molecular display concentration, for example `MgElementPerL` versus `MgIonPerL`

These types should live in a focused units module and require explicit conversion methods that take `volume_l` or a conversion factor.

### Phase 3: broader coverage stays deferred

Do not introduce a full wrapper-type forest yet. Temperature, hardness, alkalinity-display, and other derived units can wait until the chemistry/API contracts stabilize. The policy preference is a small set of high-value types plus disciplined naming, not type maximalism.

## Migration and Test Expectations

- `tanksim-6e5.2.2` should add canonical concentration helpers and migrate kinetics to concentration-based parameter names.
- `tanksim-6e5.2.7` should rename snapshot/API chemistry fields, add the scientific-versus-ion display policy, and relabel estimated TDS/conductivity in the TUI.
- Save/schema work that renames serialized fields must use explicit migration handling such as `serde(alias)` during transition or a schema-version bump with a loader migration.
- Tests that lock serialized snapshots or exact JSON field names must be updated alongside those schema changes; tests should never preserve an ambiguous chemistry name merely for backward compatibility.

## Summary Decision Record

- Internal authoritative chemistry storage remains total-based.
- Source-water and other incoming chemistry profiles remain per-liter.
- Scientific element-basis display is the default public contract.
- Ion-style display is allowed only as an explicit alternate projection with basis-explicit labels.
- TDS and conductivity remain estimates and must be named as estimates.
- Naming discipline is the immediate safeguard; targeted newtypes come later at the most error-prone conversion boundaries.
