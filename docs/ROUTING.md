# Routing Conventions

This document is the prescriptive routing policy for organic matter, dissolved nitrogen, and respiration-linked carbon flows in consumer behavior and maintenance actions. It complements [UNITS.md](UNITS.md): `UNITS.md` fixes names and quantity bases, while this note fixes where material goes when organisms eat, die, get trimmed, or get removed from the tank.

Scope for phase 1:

- Close the obvious mass-balance gaps in grazing, mortality, trimming, and husbandry actions.
- Keep the bookkeeping simple enough for deterministic tests and later tuning.
- Stay intentionally lumped where the current model does not yet carry full stoichiometric or carbonate detail.

## Basis Rules

- Particulate organic matter stays in gram-based pools: `particulate_organics_g_total` and `fine_detritus_g_total`.
- Dissolved nitrogen and carbon stay on the elemental basis defined in [UNITS.md](UNITS.md): `ammonia_total_mg_n_total`, `dissolved_organic_nitrogen_mg_n_total`, `dissolved_organic_carbon_mg_c_total`, and `dissolved_inorganic_carbon_mg_c_total`.
- Dissolved oxygen stays in `dissolved_oxygen_mg_total`.
- Cross-basis transfers must use explicit stoichiometry. Do not silently treat particulate `g` as dissolved `mg N` or `mg C`.

This means the routing contract is topological first: every removal from a source pool must land in an explicit destination pool or explicit export path. When an implementation crosses from gram-based solids to elemental dissolved pools, it must do so through named stoichiometric parameters or constants.

## Canonical Pools And Flow Topology

Relevant phase-1 pools:

- `particulate_organics_g_total`: coarse uneaten feed and similar solids.
- `fine_detritus_g_total`: mixed in-tank particulate organics, including feces, carcasses, cuttings, and fine feed residue.
- `dissolved_organic_carbon_mg_c_total` / `dissolved_organic_nitrogen_mg_n_total`: dissolved organics produced from detritus breakdown.
- `ammonia_total_mg_n_total`: dissolved inorganic nitrogen waste target for consumer excretion and mineralization.
- `dissolved_inorganic_carbon_mg_c_total`: respiration-linked carbon sink/source.
- `dissolved_oxygen_mg_total`: oxygen budget affected by respiration.
- Consumer biomass or reserve state: the retained share of assimilated intake. This may be explicit biomass or a reserve/condition proxy, but it must be an explicit retained destination.
- Export: matter intentionally removed from the system. Export is not a hidden sink.

```text
feed action
  -> particulate_organics_g_total
  -> fine_detritus_g_total
  -> DOC/DON/P dissolved pools
  -> TAN via mineralization

periphyton or fine detritus
  -> consumer ingestion
  -> retained biomass/reserve
  -> feces -> fine_detritus_g_total
  -> excretion -> ammonia_total_mg_n_total
  -> respiration -> O2 demand + dissolved_inorganic_carbon_mg_c_total

plant biomass
  -> trim and export -> export
  -> trim and leave cuttings -> fine_detritus_g_total
  -> senescence/death -> fine_detritus_g_total

shrimp / algae / eggs
  -> death -> fine_detritus_g_total

siphon detritus / water change / explicit removal actions
  -> export
```

## Consumer Routing Contract

All consumer implementations in this phase follow one template on a clearly defined bookkeeping basis before any explicit stoichiometric conversion:

1. `ingested = retained + feces + dissolved_excretion + respiration_routed_share`
2. `assimilation_efficiency + fecal_fraction = 1.0`
3. `growth_or_reserve_fraction + respiration_fraction + excretion_fraction = 1.0` on the assimilated share

Required destinations:

- `feces` always route to `fine_detritus_g_total`
- `dissolved_excretion` routes to `ammonia_total_mg_n_total`
- `respiration` contributes oxygen demand and adds to `dissolved_inorganic_carbon_mg_c_total`
- `retained` stays in the organism state as biomass, reserve, or another explicit retained store

Recommended parameter names for future implementation beads:

| Consumer | Parameter | Meaning | Phase-1 default range |
| --- | --- | --- | --- |
| shrimp | `shrimp_assimilation_efficiency` | fraction of ingested organic matter that is assimilated | `0.4..=0.7` |
| shrimp | `shrimp_respiration_fraction_of_assimilated` | share of assimilated matter routed to respiration | `0.6..=0.8` |
| shrimp | `shrimp_excretion_fraction_of_assimilated` | share of assimilated matter routed to dissolved N waste | `0.1..=0.2` |
| shrimp | `shrimp_growth_fraction_of_assimilated` | retained remainder after respiration and excretion | `0.1..=0.2` |
| microfauna | `microfauna_assimilation_efficiency` | fraction of ingested organic matter that is assimilated | `0.4..=0.7` |
| microfauna | `microfauna_respiration_fraction_of_assimilated` | share of assimilated matter routed to respiration | `0.6..=0.8` |
| microfauna | `microfauna_excretion_fraction_of_assimilated` | share of assimilated matter routed to dissolved N waste | `0.1..=0.2` |
| microfauna | `microfauna_growth_fraction_of_assimilated` | retained remainder after respiration and excretion | `0.1..=0.2` |

Derived rule:

- `fecal_fraction = 1.0 - assimilation_efficiency`
- The ranges above are order-of-magnitude starting points, not independent defaults; concrete parameter picks must satisfy the sum rules above.

Implementation notes:

- Shrimp may ingest from `periphyton_biomass_g` and `fine_detritus_g_total`, but consumed material must not disappear after those source pools are decremented.
- Microfauna use the same routing template even though the model currently stores them as `population_index` rather than explicit biomass. In phase 1 this remains an intentional approximation: the implementation may map retained material to a reserve/index-support term rather than a standalone biomass pool, but waste routing still needs explicit destinations.
- Consumer dissolved N waste is intentionally lumped into TAN in phase 1. Do not create separate urea or amino-acid pools yet.
- Respiration should affect oxygen demand and DIC, but carbonate speciation stays deferred; the DIC destination is the one lumped pool `dissolved_inorganic_carbon_mg_c_total`.

## Detritus Breakdown And Decay Routing

Detritus and uneaten feed follow one canonical in-tank decay path:

1. `Feed { grams }` adds coarse solids to `particulate_organics_g_total`
2. `feed_leach_rate_per_hour` moves that coarse material into `fine_detritus_g_total`
3. `fine_detritus_dissolution_rate_per_hour` moves fine detritus into dissolved organic pools
4. `feed_n_to_c_ratio` plus the phosphorus constant determine the DOC/DON/P split for that dissolved step
5. decomposer mineralization converts dissolved organic nitrogen into `ammonia_total_mg_n_total`

Phase-1 rule:

- decay stays in-tank unless a maintenance action explicitly exports the source material first

This preserves one simple story for feed, feces, carcasses, and cuttings: solids become detritus first, dissolved organics second, and TAN later through mineralization.

## Death, Senescence, And Failed-Reproduction Routing

Default rule:

- `death_biomass_to_detritus_fraction = 1.0`

Unless the player explicitly removes biomass from the system, all dead organic biomass stays in-tank and becomes fine detritus.

Required phase-1 routing:

- shrimp death -> `fine_detritus_g_total`
- plant senescence -> `fine_detritus_g_total`
- algae death/senescence -> `fine_detritus_g_total`
- failed eggs or reproductive loss -> `fine_detritus_g_total`

Notes:

- `plant_growth.rs` already routes senescence to fine detritus and should remain aligned with this policy.
- Shrimp mortality and failed-egg handling are the primary implementation gaps that downstream beads should close against this contract.
- There is no separate corpse pool in phase 1. Carcasses, feces, and plant cuttings all share the same `fine_detritus_g_total` pool by design.
- Molt shells and calcium accounting remain out of scope until the later carbonate/mineral phase. Do not invent a shell-routing pool in this bead family.

## Maintenance And Husbandry Actions

| Action | In-tank routing | Export routing | Policy |
| --- | --- | --- | --- |
| `Feed { grams }` | Add all input to `particulate_organics_g_total`; uneaten feed then follows the existing `particulate -> fine_detritus -> dissolved organics` path | none | This bead codifies the current coarse-to-fine-to-dissolved chain rather than redesigning it. |
| `TrimPlantsAndLeaveCuttings` | Remove plant biomass and add the removed share to `fine_detritus_g_total` | none | This is the in-tank decay variant of trimming. |
| `TrimPlantsAndExport` | none | Remove plant biomass from the model entirely | This is the default real-world maintenance interpretation and should become the explicit export action. |
| Current `TrimPlants { fraction }` | Today it behaves like `TrimPlantsAndLeaveCuttings` | none | Future action work should split the current behavior into the two explicit variants above. |
| `SiphonDetritus { fraction }` | none | Remove the chosen fraction of `particulate_organics_g_total` and `fine_detritus_g_total` from the model | Siphoned solids are exported waste, not recycled. |
| `CleanFilter { intensity }` | Reduce cleanliness and implicit biofilter biomass only | Removed fouling is treated as maintenance export | Filter biofilm is implicit in current state, so no new detritus pool is created during cleaning. |
| `WaterChangePercent { percent, ... }` | Replace removed water with source-water totals | Remove the same fraction of dissolved water-column pools with the old water | This includes TAN, nitrite, nitrate, phosphate, DOC, DON, DIC, alkalinity, and tracked ions. |
| `RemoveShrimp { count }` | none | Export removed shrimp biomass | User removal is an explicit export action, not mortality detritus. |

Export definition for this phase:

- Export means the material is intentionally removed from the closed-loop tank budget and should not reappear later through decomposition or water chemistry.
- The sanctioned organic export paths are plant export, siphoning, water changes for dissolved pools, and explicit animal removal. Everything else stays in-tank.

## Intentional Lumping And Deferred Detail

The following simplifications are deliberate for phase 1:

- Single fine-detritus pool: feces, carcasses, plant cuttings, and general fine organics all route to `fine_detritus_g_total`.
- TAN-only dissolved excretion: consumer dissolved N waste goes to `ammonia_total_mg_n_total`, not to separate urea or DON pools.
- Lumped DIC: respiration adds to `dissolved_inorganic_carbon_mg_c_total` without splitting `CO2(aq)`, `HCO3-`, and `CO3--`.
- Microfauna are index-based: their retained share may stay approximate until a later biomass-backed model exists.
- Filter fouling removal is implicit: `CleanFilter` exports maintenance waste without modeling a separate captured-solids pool.
- No detritus subtype stoichiometry pool: the source identity of feces vs corpse vs cuttings is not persisted once material enters `fine_detritus_g_total`.

What would justify un-lumping later:

- A carbonate-state rewrite that needs explicit CO2 species
- A molting/mineral budget that needs shell or calcium routing
- Habitat-specific decomposition or feces-vs-carcass turnover differences
- A biomass-backed microfauna model

## Downstream Bead Map

| Bead | Routing contract it should implement |
| --- | --- |
| `tanksim-6e5.3.2` | Shrimp ingestion -> assimilation -> feces -> TAN excretion -> respiration -> retained share |
| `tanksim-6e5.3.3` | Microfauna ingestion and recycling using the same consumer template, with index-based approximation called out explicitly |
| `tanksim-6e5.3.5` | Split trimming into export vs leave-cuttings semantics |
| `tanksim-6e5.3.7` | Death and failed-reproduction biomass routing to `fine_detritus_g_total` |

## Verification Expectations

Every implementation bead using this contract should keep the routing locally testable.

- Unit tests: verify fraction sums, source-pool decrements, destination-pool increments, and export semantics for one isolated step.
- Integration tests: verify feed, grazing, trimming, death, and water-change actions no longer delete matter silently.
- Scenario/e2e tests: verify long-horizon runs remain stable and inspectable after the new waste loops are added.

The target standard is simple: if a process consumes or removes something, the code should show exactly where it went.
