I reviewed the project statically and wrote up a fuller note here: [aquarium_sim_review_vnext.md](aquarium_sim_review_vnext.md)

Top line: this is already a strong **v0.1 foundation**. The crate layout is good, the sim is clearly engine-first instead of UI-first, and you already have meaningful regression coverage around cycling, oxygen, shrimp population, substrate effects, water changes, ambient temperature, and tank-size thermal behavior. The biggest gaps are not architectural — they are in the **scientific core**.

Update on 2026-03-27: the prescriptive routing contract for the mass-conservation gap in issue 2 and the trim/export split in issue 8 now lives in [ROUTING.md](ROUTING.md). This note remains the broader scientific review.

What is already solid

Your direction on nitrogen is better than most hobby sims. Internally you store TAN and compute free NH3 from pH and temperature, and you model AOB/NOB/comammox separately instead of doing instant “ammonia disappears” logic. That is scientifically the right shape: ammonia risk depends strongly on speciation, and recent home-aquarium work supports variable cycling plus AOA/comammox involvement in real aquarium biofilters. ([US EPA][1])

Your shrimp reproduction temperature curve is also directionally good. The current code pushes reproduction into a broad mid-20s sweet spot and strongly penalizes high temperatures. That matches the literature fairly well: *Neocaridina davidi* spawned at 28 °C but not 33 °C in one study, and another found the highest proportion of ovigerous females at 28 °C while females that matured and mated at 32 °C lost their eggs. ([PubMed][2])

The project also correctly treats geometry and ambient temperature as real simulation inputs instead of flavor. `TankGeometry` is first-class, thermal inertia depends on volume and exposed area, and `ChangeAmbientTemperature` already exists in the action model.

The main issues I found

1. **Legacy kinetics field names still look total-based even after the concentration refactor.**

This is the biggest scientific problem right now.

`crates/tank_core/src/systems/plant_growth.rs`, `algae_growth.rs`, and `nitrogen_cycle.rs` now read canonical concentration helpers for water-column Monod and half-saturation math, so same concentration no longer gets a different limitation factor purely because the tank is larger. To preserve existing tuning near the historical default tank, those legacy preset values are normalized against a 20 L reference water volume. The remaining issue is naming: parameters such as `plant_half_saturation_n_mg_total`, `algae_half_saturation_n_mg_total`, `decomposer_k_doc_mg`, `aob_k_tan_mg`, and `aob_k_do_mg` still carry legacy suffixes that read like totals even though the runtime treats them as reference-volume-normalized concentration-scale values.

That specific tank-size artifact is now resolved for the water column, but the rename debt is still important because the current field ids invite future contributors to wire totals back in by mistake.

This should be fixed before adding more species or more features.

2. **Mass conservation breaks in grazing and detritus processing.**

In `crates/tank_core/src/systems/shrimp.rs:119-148`, shrimp remove periphyton and fine detritus, but that consumed mass does not come back as feces, dissolved waste, respiration, or shrimp biomass reserve. It mostly vanishes. `microfauna.rs:45-55` does the same thing on a smaller scale.

That means your food web is not closed. Grazing makes the tank look cleaner, but it underestimates ongoing bioload and nutrient recycling. In a more realistic shrimp tank, grazed biofilm should become some combination of:

* assimilated energy / body reserve,
* feces -> fine detritus,
* excreted nitrogen -> TAN,
* respiration -> O2 demand + DIC.

Without that loop, you cannot get stable long-term nutrient turnover.

3. **The pH / carbonate model is much too compressed.**

The current solver in `crates/tank_core/src/systems/chemistry.rs:6-20` is:

`6.3 + log10(alkalinity) - log10(DIC)`, clamped to 5.5–8.5.

That is useful as a placeholder, but not accurate enough for the kind of simulation you want. Carbonate chemistry and pH are nonlinear and depend on equilibrium among CO2(aq), carbonic acid species, alkalinity, and gas exchange. In real waters, CO2 degassing can materially shift both DIC and pH, which is exactly why a mechanistic carbonate model is preferred when those variables matter. ([USGS][3])

There is also a practical implementation consequence: your default `respiration_dic_rate_mg_c_per_g_per_hour` and `photosynthesis_dic_rate_mg_c_per_g_per_hour` are both `0.0` in `types/process.rs:135-136`, so the hourly chemistry step often does almost nothing to DIC except during water changes and alkalinity loss. That means realistic day/night CO2-pH behavior cannot emerge.

I also checked your shipped source-water presets against the current formula. They collapse to almost the same pH:

* `hard_shrimp` ≈ **6.31**
* `moderate` ≈ **6.30**
* `soft_acidic` ≈ **6.33**
* `ro_like` ≈ **6.70**

So the named waters differ in hardness and ions, but not in a way the current pH model expresses convincingly.

4. **Internal units are mostly fine, but the outputs are mislabeled.**

`WaterState` stores nitrogen species as `mg N total` (`ammonia_total_mg_n_total`, `nitrite_mg_n_total`, `nitrate_mg_n_total`), which is scientifically defensible because EPA ammonia criteria and recent aquarium-biofilter work both use TAN / nitrogen-based reporting. The issue is the snapshot/UI naming in `crates/tank_core/src/types/snapshot.rs:13-17, 66-82`: fields like `nitrite_mg_l` and `nitrate_mg_l` look like full-ion concentrations, not N-as-nitrogen values. ([US EPA][1])

So you need to choose one of two paths:

* keep internal storage as **mg N/L** and label it that way, or
* convert for display to hobby-style ion ppm:

  * NO3 as ion = N × 62/14
  * NO2 as ion = N × 46/14

Right now the simulation and the UI are speaking slightly different chemical languages.

5. **TDS and conductivity are too approximate for a “scientific” mode.**

`WaterState::total_tracked_ions_mg()` only sums Ca, Mg, Na, K, bicarbonate, chloride, and sulfate. Snapshot TDS is then `tracked_ions / volume`, and conductivity is estimated as `tds / 0.65`.

For a casual display, that is okay. For a more serious simulator, it is too rough. Specific conductance depends on the collective concentration of ions and on temperature, and the relation between SC and “TDS” varies with water type; USGS explicitly notes that TDS and salinity are not equivalent for most waters and that major-ion composition matters for SC-based estimation. ([U.S. Geological Survey Publications][4])

So I would either:

* rename the current metric to something like **tracked major ions**, or
* move toward a proper ion-sum / conductivity model.

6. **Biofilter carrying capacity is fixed and not habitat-based.**

In `nitrogen_cycle.rs:246` and again in `:359-364`, nitrifier carrying capacity is hard-coded to `0.5 g`. That means a tiny sponge filter in a nano tank and a large porous canister on a 100 L tank effectively share the same maturity ceiling unless kinetics happen to hide it.

That should scale with at least:

* colonizable biomedia area,
* flow-through,
* oxygen availability,
* perhaps clogging and maintenance history.

Right now the code knows tank geometry, but not enough about **where microbes live**.

7. **Tank-size scaling is only half-finished.**

`crates/tank_scenarios/src/lib.rs:241-259` scales geometry and substrate nutrient charge with footprint. Good. But startup hardware and biomass do not consistently scale with the new geometry. `apply_startup_overrides()` leaves filter flow at a generic 200 lph if enabled (`:581-589`), plant biomass stays fixed at 5 g per guild in `build_plant_guilds()` (`:687-699`), and heater power is not geometry-aware.

So a bigger tank currently gets different thermal and biological behavior from both good reasons and accidental reasons. Those need to be separated.

8. **Plant trimming should distinguish export vs in-tank decay.**

In `engine.rs:189-197`, `TrimPlants` removes biomass from plants and adds all of it to `fine_detritus_g_total`.

That is only realistic for “trim and leave clippings in the tank.” In actual maintenance, most trimming is biomass export. This should be two different actions:

* `TrimPlantsAndRemove`
* `TrimPlantsAndLeaveCuttings`

Scientific accuracy: where it stands now

I would call the current implementation **directionally credible but not yet predictive**.

It is already good enough for:

* comparative gameplay,
* qualitative failure modes,
* demonstrating why small tanks are touchier,
* showing cycling, algae pressure, husbandry tradeoffs.

It is not yet good enough to claim “this can realistically reproduce real chemistry outcomes” because the three biggest requirements for that are still missing:

* concentration-normalized kinetics,
* closed mass loops,
* carbonate / CO2 / pH realism.

There are also a few medium-priority scientific gaps I would address in the next major version. The most important are:

* aeration changes DO but not CO2 stripping or pH,
* no denitrification in low-oxygen substrate zones,
* nitrite stress ignores chloride protection even though chloride is one of the main modifiers of nitrite toxicity in freshwater systems and that effect is documented in crustaceans too, including crayfish, where added chloride reduces nitrite accumulation and stress. ([ResearchGate][5])
* no depth-based light attenuation despite geometry including depth,
* no habitat-specific biofilm pools for glass, plant surfaces, filter media, and substrate.

How I would plan the next version

I would make the next release a **scientific core release**, not a feature release.

### v0.2 — scientific invariants

Focus on getting the math model right before expanding content.

Build goals:

* finish renaming the remaining legacy Monod / half-saturation field ids so their names match the now-concentration-based runtime
* add helper methods for concentration and areal density
* introduce unit-safe newtypes or at least stricter field naming
* add explicit shrimp ingestion -> assimilation -> excretion -> feces
* make microfauna consumption mass-conserving
* split trim actions into export vs leave-in-tank
* fix snapshot unit naming

This is the release that makes “same concentration, different tank size” behave correctly.

### v0.3 — carbonate / gas exchange

After conservation is fixed, do chemistry.

Build goals:

* keep `dissolved_inorganic_carbon_mg_c_total` and `alkalinity_meq_total` as canonical state, and derive `CO2(aq)`, `HCO3-`, `CO3--`, and `pH` each chemistry resolve as defined in [carbonate_state_contract.md](carbonate_state_contract.md)
* replace the current log-linear pH shortcut with the approved first-pass analytical solve: temperature-corrected `pKa1`, fixed first-pass `pKa2`, no Newton iteration in the freshwater target envelope
* link aeration and surface exchange to **both** O2 and DIC/CO2 as a follow-on that feeds the same carbonate resolve step
* treat source-water `pH` or atmospheric `pCO2` targets as later extensions rather than a second authoritative pH state
* optionally add CO2 injection hardware later

That will let you get real day/night pH drift, aeration-driven CO2 stripping, and more believable plant/algae dynamics. ([USGS][3])

### v0.4 — habitatized ecology

Then make the ecosystem spatial in the ecological sense, not in the CFD sense.

Split habitats into:

* filter media
* glass / hardscape
* plant surfaces
* substrate surface
* deeper substrate zone

Each habitat gets:

* colonizable area,
* oxygen exposure,
* flow exposure,
* biofilm load,
* nitrifier/decomposer/periphyton preference.

That is also where denitrification belongs: not “everywhere nitrate sometimes vanishes,” but “suboxic substrate microzones can remove some nitrate.”

### v0.5 — shrimp realism

Only after the chemistry and ecology core is stable. The prescriptive state-model contract for the stage-structured shrimp redesign now lives in [shrimp_state_contract.md](shrimp_state_contract.md).

Build goals:

* stage / size structure (juvenile, sub-adult, adult with per-stage reserve and condition — see [shrimp_state_contract.md](shrimp_state_contract.md)),
* mineral budget for molt success (extension point defined in the state contract),
* density and sex-ratio effects,
* osmotic stress hooks,
* chloride-modulated nitrite hazard,
* more explicit juvenile survival logic.

The first five tickets I would actually implement next

1. **Unit cleanup**
   Rename outputs and parameters so it is impossible to confuse `mg N/L` with `mg/L as ion`.

2. **Concentration helper layer**
   Add methods like `tan_mg_n_l()`, `no3_mg_n_l()`, `no3_mg_l_as_ion()`, `doc_mg_c_l()`, and habitat areal densities.

3. **Normalize all kinetics**
   Rewrite plant, algae, decomposer, and nitrifier saturation terms to use concentration or area-based exposure.

4. **Add animal waste loop**
   Shrimp grazing must return mass into the system as feces, TAN, and respiration.

5. **Replace the pH shortcut**
   Implement `tanksim-6e5.4.2` against [carbonate_state_contract.md](carbonate_state_contract.md), then layer CO2 exchange onto that same contract before tuning chemistry further.

My overall recommendation

Do not jump to fish, diseases, or fancy graphics yet.

The best next version is one that makes the simulation **chemically and ecologically self-consistent**. Once concentration scaling, conservation, and carbonate chemistry are fixed, everything else — algae blooms, stabilization, breeding success, mineral stress, substrate effects — will become much more believable with less ad hoc tuning.

The next step should be to turn this into a **v0.2 implementation backlog** with concrete tickets against the existing crates.

[1]: https://www.epa.gov/sites/default/files/2015-08/documents/aquatic-life-ambient-water-quality-criteria-for-ammonia-freshwater-2013.pdf "https://www.epa.gov/sites/default/files/2015-08/documents/aquatic-life-ambient-water-quality-criteria-for-ammonia-freshwater-2013.pdf"
[2]: https://pubmed.ncbi.nlm.nih.gov/29949439/ "https://pubmed.ncbi.nlm.nih.gov/29949439/"
[3]: https://www.usgs.gov/publications/modeling-co2-degassing-and-ph-a-stream-aquifer-system "https://www.usgs.gov/publications/modeling-co2-degassing-and-ph-a-stream-aquifer-system"
[4]: https://pubs.usgs.gov/tm/09/a6.3/tm9-a6_3.pdf "https://pubs.usgs.gov/tm/09/a6.3/tm9-a6_3.pdf"
[5]: https://www.researchgate.net/publication/226844778_Chloride_uptake_in_freshwater_teleosts_and_its_relationship_to_nitrite_uptake_and_toxicity "https://www.researchgate.net/publication/226844778_Chloride_uptake_in_freshwater_teleosts_and_its_relationship_to_nitrite_uptake_and_toxicity"
