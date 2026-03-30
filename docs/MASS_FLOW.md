# Mass-Flow Map: Feed, Detritus, DOC, and Mineralization

This document traces how nitrogen (N) and carbon (C) flow through the
tank simulation from feed inputs and organism death/senescence through
detritus, dissolved organics, and mineralization.  It is the reference
map for the bookkeeping audit (tanksim-6e5.3.4).

## Pool Inventory

### Particulate Pools (grams of organic matter)

| Pool | Description |
|------|-------------|
| `particulate_organics_g_total` | Coarse uneaten feed |
| `fine_detritus_g_total` | Mixed fine particulates: feces, carcasses, plant/algae loss, leached feed |

### Dissolved Pools (elemental mg)

| Pool | Description |
|------|-------------|
| `dissolved_organic_carbon_mg_c_total` (DOC) | Dissolved organic carbon from detritus dissolution + consumer excretion + microbe decay |
| `dissolved_organic_nitrogen_mg_n_total` (DON) | Dissolved organic nitrogen, tracks proportionally with DOC via `feed_n_to_c_ratio` |
| `dissolved_inorganic_carbon_mg_c_total` (DIC) | CO₂/HCO₃⁻/CO₃²⁻ pool; destination for respiration C |
| `ammonia_total_mg_n_total` (TAN) | NH₃/NH₄⁺; destination for excretion N + mineralization N |
| `nitrite_mg_n_total` | Intermediate nitrification product |
| `nitrate_mg_n_total` | Terminal nitrification product |
| `phosphate_mg_p_total` | Dissolved phosphorus |

### Biomass Pools

| Pool | N conversion | C conversion |
|------|-------------|-------------|
| `plant_guilds[*].biomass_g` | × PLANT_N_MG_PER_G_BIOMASS (28.0) | plant_carbon_mg(biomass, ratio) |
| `algae.suspended_biomass_g` | × ALGAE_N_MG_PER_G_BIOMASS (35.0) | algae_carbon_mg(biomass, ratio) |
| `algae.periphyton_biomass_g` | × ALGAE_N_MG_PER_G_BIOMASS (35.0) | algae_carbon_mg(biomass, ratio) |
| `microbe.decomposer_biomass_g` | live_biomass_nitrogen_mg(g, ratio) | live_biomass_carbon_mg(g, ratio) |
| `microbe.ammonia_oxidizer_biomass_g` | live_biomass_nitrogen_mg(g, ratio) | live_biomass_carbon_mg(g, ratio) |
| `microbe.nitrite_oxidizer_biomass_g` | live_biomass_nitrogen_mg(g, ratio) | live_biomass_carbon_mg(g, ratio) |
| `microbe.comammox_biomass_g` | live_biomass_nitrogen_mg(g, ratio) | live_biomass_carbon_mg(g, ratio) |
| `animal.{adult,sub_adult,juvenile}.count` | count × stage_mass × body_n_per_g | count × stage_mass × body_c_per_g |
| `animal.{adult,sub_adult,juvenile}.reserve_g` | detritus_nitrogen_mg(g, ratio) | detritus_carbon_mg(g, ratio) |
| `microfauna.reserve_g` | detritus_nitrogen_mg(g, ratio) | detritus_carbon_mg(g, ratio) |

### Bookkeeping-only (not a mass pool)

| Field | Purpose |
|-------|---------|
| `dissolved_feed_residue_g_total` | Tracks cumulative dissolved organic matter that originated from feed. Incremented by detritus dissolution, decremented by decomposer consumption. Not a conservation pool — the actual mass lives in DOC/DON. |

## Stoichiometric Constants

| Constant | Value | Meaning |
|----------|-------|---------|
| `feed_n_to_c_ratio` | 0.16 | mg N per mg C in organic matter |
| `FEED_P_TO_N_MASS_RATIO` | 0.10 | mg P per mg N released during dissolution |
| `PLANT_N_MG_PER_G_BIOMASS` | 28.0 | Plant wet biomass → N |
| `ALGAE_N_MG_PER_G_BIOMASS` | 35.0 | Algae wet biomass → N |
| `LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G` | 0.20 | Generic microbe wet mass → organic matter |
| `SHRIMP_N_MG_PER_G_WET_MASS` | 27.59 | Shrimp body N (default) |
| `SHRIMP_C_MG_PER_G_WET_MASS` | 172.41 | Shrimp body C (default) |
| `O2_PER_MG_C_RESPIRED` | 2.67 | Stoichiometric O₂:C for organic oxidation (32/12) |

### Organic-matter gram ↔ elemental mg conversions

All particulate pools store mass in grams of organic matter.  The N:C
ratio (`feed_n_to_c_ratio = 0.16`) determines the split:

```
organic_nitrogen_mg(mass_g, ratio) = mass_g × 1000 × ratio / (1 + ratio)
organic_carbon_mg(mass_g, ratio)   = mass_g × 1000 / (1 + ratio)
```

When biomass enters `fine_detritus_g_total`, dedicated conversion
functions (`plant_detrital_mass_g`, `algae_detrital_mass_g`,
`shrimp_body_detrital_mass_g`) first compute the elemental N + C content
of the source, then express the sum as organic-matter grams:
`detrital_g = (N_mg + C_mg) / 1000`.  This ensures that the N:C ratio
embedded in fine detritus is always consistent with `feed_n_to_c_ratio`,
regardless of the source organism's per-gram stoichiometry.

## Flow Pathways

### 1. Feed Input → Coarse Particulates

```
PlayerAction::Feed { grams }
  └─► particulate_organics_g_total += grams        [engine.rs]
```

Feed enters as coarse organic matter.  The hourly nitrogen cycle then
cascades it through the detritus/dissolution/mineralization chain.

### 2. Coarse → Fine Detritus (Hourly Leaching)

```
nitrogen_cycle.rs  §1  Feed leaching
  particulate_organics_g_total
    │  × feed_leach_rate_per_hour × (1 − 0.5 × substrate_trapping)
    ▼
  fine_detritus_g_total
```

Substrate trapping slows leaching: porous/planted substrates hold
coarse matter longer.

### 3. Fine Detritus → DOC / DON / P (Hourly Dissolution)

```
nitrogen_cycle.rs  §2  Fine detritus dissolution
  fine_detritus_g_total
    │  × fine_detritus_dissolution_rate_per_hour
    │
    ├─► dissolved_organic_carbon_mg_c_total  += dissolved × 1000 / (1 + ratio)
    ├─► dissolved_organic_nitrogen_mg_n_total += DOC × ratio
    ├─► phosphate_mg_p_total                 += DON × FEED_P_TO_N_MASS_RATIO
    └─► dissolved_feed_residue_g_total       += dissolved  (bookkeeping only)
```

### 4. DOC / DON → TAN / DIC (Decomposer Mineralization)

```
nitrogen_cycle.rs  §3  Decomposer mineralization
  DOC pool ──────────► decomposer consumption (Monod kinetics)
  DON pool (proportional)
    │
    ├─ growth yield ──► decomposer_biomass_g (constrained by N + C availability)
    ├─ remineralized N ► ammonia_total_mg_n_total  (DON consumed − growth N)
    └─ remineralized C ► dissolved_inorganic_carbon_mg_c_total  (DOC consumed − growth C)

  decomposer decay ──► DOC + DON  (route_live_biomass_to_dissolved_organics)
```

Microfauna boost: `decomposer_vmax × (1 + microfauna_mineralization_boost × population_index)`.

### 5. TAN → NO₂⁻ → NO₃⁻ (Nitrification)

```
nitrogen_cycle.rs  §4  Nitrification
  TAN ──AOB──► NO₂⁻ ──NOB──► NO₃⁻       (two-step canonical path)
  TAN ──comammox──────────────► NO₃⁻       (one-step shortcut)

  O₂ costs:  AOB 3.43, comammox 4.57, NOB 1.14  mg O₂ per mg N
  Alkalinity: AOB + comammox consume meq; NOB does not
  Growth: all three guilds assimilate TAN + DIC into biomass
  Decay:  all three guilds route decayed biomass → DOC + DON
```

### 6. Consumer Feeding (Shrimp and Microfauna)

Both shrimp and microfauna follow the same consumer routing contract.

```
Ingested (periphyton + fine_detritus)
  ├─ Feces (1 − AE)           ──► fine_detritus_g_total
  └─ Assimilated (AE)
       ├─ Excretion (10%)     ──► TAN (N) + DOC (C) + alkalinity
       ├─ Respiration (70%)   ──► DIC (C) + O₂ demand + TAN (N) + alkalinity
       │    └─ O₂-limited shortfall ──► redirected to retained
       └─ Retained (20%)      ──► organism reserve_g (shrimp per-stage or microfauna)
```

Default fractions (both consumers):
- `assimilation_efficiency`: 0.50
- `respiration_fraction_of_assimilated`: 0.70
- `excretion_fraction_of_assimilated`: 0.10
- `growth_fraction_of_assimilated`: 0.20

### 7. Plant Growth and Senescence (Daily)

```
Growth:
  TAN + NO₃⁻ + substrate_N + PO₄³⁻ + DIC ──► plant_guilds[i].biomass_g

Loss (respiration + senescence):
  plant biomass ──► fine_detritus_g_total
    via plant_detrital_mass_g(loss_g, n_to_c_ratio)
```

Senescence increases under stress (low nutrient/light/habitat).

### 8. Algae Growth and Loss (Daily)

```
Growth (suspended + periphyton):
  TAN + NO₃⁻ + PO₄³⁻ + DIC ──► algae biomass

Loss (respiration + microfauna grazing + excess shedding):
  algae biomass ──► fine_detritus_g_total
    via algae_detrital_mass_g(loss_g, n_to_c_ratio)
```

### 9. Death and Senescence Routing (C7 Interface)

All organism death enters `fine_detritus_g_total`, which then follows
the **same** downstream path (dissolution → DOC/DON → mineralization →
TAN/DIC) as feed-derived detritus.  There is no separate corpse pool.

```
Shrimp death (mortality function):
  body biomass ──► fine_detritus_g_total
    via shrimp_body_detrital_mass_g(dead_mass, body_n_per_g, body_c_per_g)
  per-stage reserve_g ──► fine_detritus_g_total
    via transfer_dead_reserve_g() (proportional to deaths/count)

Plant senescence:
  biomass loss ──► fine_detritus_g_total
    via plant_detrital_mass_g(loss_g, n_to_c_ratio)

Algae loss (respiration + grazing + excess):
  biomass loss ──► fine_detritus_g_total
    via algae_detrital_mass_g(loss_g, n_to_c_ratio)

Microbe decay (decomposers + nitrifiers):
  decayed biomass ──► DOC + DON directly
    via route_live_biomass_to_dissolved_organics()
    (bypasses fine_detritus; already dissolved-scale organisms)
```

**Key invariant**: Death/senescence routing from C7 (tanksim-6e5.3.7)
enters the same `fine_detritus_g_total` pool as feed-derived waste.
From there, dissolution and decomposer mineralization treat all fine
detritus identically.  There is no double-counting because:

1. The source biomass pool is decremented (count, biomass_g, or reserve_g).
2. The detrital mass added to `fine_detritus_g_total` is computed from
   the source's elemental content, not from the detritus-side ratio.
3. The budget tracker uses `detritus_nitrogen_mg(mass, ratio)` to
   recover N from the aggregate fine detritus pool.

The detrital-mass conversion functions ensure that `N:C` embedded in
the organic-matter grams is always consistent with `feed_n_to_c_ratio`,
so dissolution and mineralization downstream are stoichiometrically
coherent with the budget tracker's recovery functions.

## Execution Order

### Hourly (every tick)

1. Player actions (Feed → `particulate_organics_g_total`)
2. Temperature
3. **Nitrogen cycle**: feed leaching → fine detritus dissolution → decomposer mineralization → nitrification
4. Chemistry (carbonate equilibrium, pH)
5. Dissolved oxygen (reaeration, BOD)
6. Shrimp hourly stress
7. Hourly events
8. Invariant enforcement

### Daily (at hour 0)

1. Plants (growth + senescence → fine detritus)
2. Algae (growth + loss → fine detritus)
3. Microfauna (consumption + routing)
4. Stability tracker
5. Shrimp (feeding + condition + molt + reproduction + mortality → fine detritus)
6. Filter clogging + biofilter maturity

## Known Simplifications

1. **Single fine-detritus pool**: Feces, carcasses, plant cuttings, and
   leached feed share `fine_detritus_g_total` without source identity.

2. **Uniform detritus stoichiometry**: All fine detritus dissolves with
   the same `feed_n_to_c_ratio` regardless of biological origin.  The
   conversion functions ensure this is budget-consistent.

3. **Microfauna reserve is one-way**: `microfauna.reserve_g` accumulates
   from the retained share of ingestion but has no decay or starvation
   draw.  A future biomass-backed microfauna model would close this.

4. **Shrimp reserve has no maintenance draw**: Reserve accumulates from
   feeding and is consumed only by reproduction or death transfer.
   Starvation metabolism is not yet modeled.

5. **Microbe decay bypasses fine detritus**: Decomposer and nitrifier
   decay routes directly to DOC/DON rather than through fine detritus.
   This is appropriate for dissolved-scale organisms but means microbe
   decay does not contribute to the fine-detritus pool.

6. **`dissolved_feed_residue_g_total` is bookkeeping only**: It tracks
   the flow-through of feed-derived dissolved organics for diagnostics
   (e.g., filter clogging pressure) but is not a mass pool in the
   conservation budget.

7. **Decomposer mineralization does not debit O₂**: The decomposer
   DOC→DIC remineralization step converts dissolved organic carbon to
   dissolved inorganic carbon but does not consume dissolved oxygen
   stoichiometrically.  Dissolved oxygen acts only as a Monod modulation
   factor (`f_do_decomp`) that slows mineralization under low-DO
   conditions.  The `background_bod_mg_o2_per_g_biomass_per_hour`
   parameter in the dissolved oxygen system provides an aggregate
   respiration demand that implicitly covers decomposer activity.  A
   future phase may add explicit stoichiometric O₂ coupling
   (2.67 mg O₂ per mg C remineralized) to the decomposer section.

## Budget Coverage

The budget tracker (budget.rs) enumerates 19 nitrogen components and
16 carbon components.  Every transfer between pools uses explicit
stoichiometric conversions.  The `assert_n_conserved` and
`assert_c_conserved` helpers verify that total N and C are invariant
across any number of ticks in a closed system (gas exchange disabled).

Open-system paths that can change total N or C:
- Gas exchange (reaeration/aeration): O₂ in/out, CO₂ in/out
- Water changes: dissolved pools exchanged with source water
- Explicit exports: siphon detritus, trim-and-export, remove shrimp
