# Carbonate State Contract

This note is the implementation contract for `tanksim-6e5.4.1`. It fixes the minimum viable carbonate model for the freshwater aquarium envelope the simulator currently targets so `tanksim-6e5.4.2` can implement one solver without reopening the state-model debate.

## Scope

- Target envelope: well-mixed freshwater aquaria, roughly `15..=35 C`, usually `pH 5.5..=8.5`, no marine salinity, and no strong non-carbonate buffers.
- Goal: replace the current `compute_ph_from_totals()` shortcut with a deterministic carbonate-equilibrium solve that keeps DIC and alkalinity authoritative.
- Non-goal: full geochemical speciation. This is a carbonate-only first pass.

## Chosen State Contract

### Canonical explicit state

| Quantity | Current field | Why it stays explicit |
| --- | --- | --- |
| Total dissolved inorganic carbon | `WaterState.dissolved_inorganic_carbon_mg_c_total` | Respiration, photosynthesis, water changes, and future gas exchange all naturally add or remove total carbon. |
| Alkalinity | `WaterState.alkalinity_meq_total` | Nitrification and source-water mixing already mutate alkalinity directly, and it is the correct carbonate charge-balance input. |
| Temperature | `WaterState.temperature_c` | The solver needs it to choose equilibrium constants. |

### Derived per chemistry resolve

| Quantity | Intended representation | Ownership |
| --- | --- | --- |
| Dissolved `CO2(aq)` | `co2_aq_mmol_per_l` | Returned by the carbonate solver; not an independent saved pool in the first pass. |
| `HCO3-` | `hco3_mmol_per_l` | Returned by the carbonate solver; authoritative source for any bicarbonate projection. |
| `CO3--` | `co3_mmol_per_l` | Returned by the carbonate solver; small in the target envelope but still derived for completeness. |
| pH | `ph` plus solver return value | Derived from the same equilibrium solve each time DIC/alkalinity are resolved. |

### Cached compatibility fields

- `WaterState.ph` remains a stored derived value so existing NH3, event, and snapshot readers can keep their current access pattern.
- `WaterState.bicarbonate_mg_total` becomes a cached projection, not an authoritative chemistry store. The chemistry step overwrites it from solved `HCO3-` after every resolve.
- `SourceWaterProfile.bicarbonate_mg_per_l` remains an input-data compatibility field for now, but `dic_mg_c_per_l` and `alkalinity_meq_per_l` stay authoritative for chemistry. After a water change or startup materialization, runtime bicarbonate is re-derived from the equilibrium solve.

### Rejected alternative

The contract does not promote `CO2(aq)` to canonical state. Doing so would force a migration away from the already-shipped DIC budget, complicate water-change/source-water materialization, and provide little first-pass benefit because most current producers and consumers already operate in total-carbon terms.

## Solver Contract

### Units inside the solver

Convert the stored totals into molar units before solving:

- `dic_mol_per_l = dissolved_inorganic_carbon_mg_c_total / 12_000.0 / volume_l`
- `alk_eq_per_l = alkalinity_meq_total / 1_000.0 / volume_l`

Derived concentrations returned to the rest of the engine should use `mmol/L` names:

- `co2_aq_mmol_per_l`
- `hco3_mmol_per_l`
- `co3_mmol_per_l`

### Equilibrium assumptions

Use the freshwater carbonate system only:

- `CO2(aq) + H2O <-> H+ + HCO3-`
- `HCO3- <-> H+ + CO3--`
- `DIC = [CO2(aq)] + [HCO3-] + [CO3--]`

First-pass alkalinity approximation:

- `Alk ~= [HCO3-] + 2[CO3--]`

Deliberately omitted from the first-pass charge balance:

- `OH- - H+`
- borate, phosphate, humic, and other weak-acid systems
- ionic-strength and activity corrections

That approximation is acceptable for the aquarium envelope this simulator currently targets, where bicarbonate dominates most buffered freshwater tanks.

### Equilibrium constants

Use temperature-corrected `pKa1` (Harned & Davis, 1943) and fixed first-pass `pKa2`:

- `pKa1(T) = 3404.71 / (temperature_c + 273.15) + 0.032786 * (temperature_c + 273.15) - 14.8435`
- `pKa2 = 10.33`
- `Ka1 = 10^-pKa1`
- `Ka2 = 10^-pKa2`

The Harned & Davis formula is three arithmetic operations and accurate across the full 15-35°C target range without linearization error. A linear approximation `pKa1(T) ≈ 6.352 - 0.0055 * (temperature_c - 25.0)` is acceptable (~±0.03 pK units across 15-35°C) but should be replaced with the full formula if the temperature range expands.

`pKa2` stays fixed in the first pass because `CO3--` is a minor fraction across most of the `pH 5.5..=8.5` target range. The solver interface should still keep both constants explicit so a later bead can temperature-correct `pKa2` without changing the state layout.

### Closed-form solve

Let `H = [H+]` and define:

- `denom = H^2 + Ka1 * H + Ka1 * Ka2`
- `[CO2(aq)] = DIC * H^2 / denom`
- `[HCO3-] = DIC * Ka1 * H / denom`
- `[CO3--] = DIC * Ka1 * Ka2 / denom`

Substituting the first-pass alkalinity approximation yields a quadratic in `H`:

`Alk * H^2 + Ka1 * (Alk - DIC) * H + Ka1 * Ka2 * (Alk - 2 * DIC) = 0`

Implementation requirements:

1. Solve that quadratic analytically, not with Newton iteration.
2. Choose the positive real root that lands inside the diagnostic guard band `4.0 <= pH <= 10.0`.
3. Compute the species fractions from that root.
4. Convert solved species back to `mmol/L` for the return struct and to cached totals where needed.

### Edge-case handling

- If `volume_l <= f64::EPSILON`, return a neutral fallback (`pH 7.0`) and zero speciation outputs.
- If `dic_mol_per_l <= 1e-12`, return zero carbonate species. Use `pH 7.0` only for truly unbuffered water; if alkalinity remains positive, hold the alkaline ceiling (`pH 8.5`) so the zero-DIC limit stays consistent with the live solver.
- If `alk_eq_per_l <= 1e-9` or the quadratic does not yield a valid positive root, emit a structured diagnostic and use an acid-dominated fallback path. Compute the fallback `pH` from the carbonic-acid quadratic, clamp it to storage bounds, and reproject `CO2(aq)`, `HCO3-`, and `CO3--` at that fallback `pH` so reported species stay thermodynamically self-consistent.

Those fallbacks are intentionally explicit. They are stability guards for aquarium edge cases such as `ro_like` or temporarily DIC-starved water, not a claim that the first-pass model captures atmospheric CO2 equilibrium or full alkalinity chemistry.

### Cached field projection

After each solve:

- `WaterState.ph = solved_ph`
- `WaterState.bicarbonate_mg_total = hco3_mmol_per_l * 61.0 * volume_l`

`bicarbonate_mg_total` should never again be incremented independently by chemistry logic once the new solver lands. It is a projection of solved `HCO3-`, not a second carbonate budget.

## Update Order And Ownership

1. Water changes and scenario materialization write canonical totals (`DIC`, `alkalinity`, temperature, major ions) and then call the shared carbonate resolve helper instead of hand-setting pH or bicarbonate.
2. The nitrogen-cycle step remains responsible for alkalinity depletion from nitrification.
3. The hourly chemistry step remains responsible for biological DIC deltas from respiration and photosynthesis.
4. After all same-hour DIC/alkalinity mutations are applied, one shared carbonate resolve helper computes speciation and overwrites the derived caches (`ph`, `bicarbonate_mg_total`).
5. NH3 speciation, shrimp stress, events, snapshots, and display helpers read the derived outputs but do not mutate them directly.
6. Future CO2 gas exchange plugs in as another DIC delta immediately before the carbonate solve. It does not become a separate persisted pH state.

Ownership boundary:

- Authoritative mutators: water changes, startup materialization, nitrification, respiration/photosynthesis, and future gas-exchange/CO2-injection logic.
- Derived-only readers: NH3 logic, warnings/events, UI snapshots, TDS/conductivity estimates, and debugging tools.

## Stability And Precision Expectations

- Deterministic `O(1)` work per resolve. No iterative convergence tuning.
- The implementation bead should validate `pH` to within about `+/- 0.05` against a reference carbonate calculation across the intended freshwater envelope:
  - `DIC 2..=40 mg C/L`
  - `alkalinity 0.25..=3.0 meq/L`
  - `temperature 15..=35 C`
- Under ordinary buffered conditions the guard clamp should not fire. If it does, the implementation should emit structured tracing so the bad input regime is inspectable.
- Tests should cover:
  - deterministic unit checks for the closed-form species fractions
  - integration checks that nitrification lowers alkalinity and pH
  - integration checks that photosynthesis lowers `CO2(aq)` and raises pH without directly changing alkalinity

## Intentionally Excluded In The First Pass

- atmospheric `pCO2` forcing or Henry-law gas exchange
- CO2 injection hardware
- borate, phosphate, humic, or other non-carbonate buffers
- precipitation/dissolution of carbonate minerals
- ionic-strength/activity corrections
- spatial layering or substrate porewater carbonate chemistry
- source-water `pH` as an independent canonical state variable
