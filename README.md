# Tank Simulation

`tank-simulation` is a Rust workspace for a deterministic freshwater planted shrimp tank simulator. It models the aquarium as an ecosystem rather than a scripted event loop: water chemistry, biofilter succession, plant growth, algae pressure, detritus, ambient temperature, and `Neocaridina davidi` population dynamics all feed back on each other over time.

The simulator has undergone a scientific-core upgrade across phases 2-5, resolving the main limitations of the original v0.1 foundation. The architecture overview and reading guide live in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), the canonical unit/display policy in [docs/UNITS.md](docs/UNITS.md), and parameter confidence annotations in [docs/PROVENANCE_STATUS.md](docs/PROVENANCE_STATUS.md).

## What it does

The simulator supports:

- deterministic simulation with seed/save-load support
- configurable tank geometry and fill height
- source-water profiles with hardness, alkalinity, and tracked ions
- substrate presets and rooted plant guilds
- nitrogen cycling with distinct microbial groups (AOB, NOB, comammox, decomposers)
- carbonate equilibrium with temperature-corrected pKa1 and closed-form quadratic pH solver
- five-zone habitat registry (filter media, glass/hardscape, plant surfaces, substrate surface, substrate deep) with flow, oxygen, and light exposure modifiers
- stage-structured shrimp population (juvenile, sub-adult, adult) with per-stage reserves, molt cycle, reproduction, and mineral stress
- dissolved oxygen, temperature, lighting, heater, filter, and aeration effects
- parameter provenance with four-tier confidence metadata (literature, expert, heuristic, placeholder)
- envelope-based calibration harness with eight validation scenarios
- shrimp stocking and maintenance actions such as feeding, water changes, plant trimming (export or leave cuttings), siphoning, and filter cleaning
- long-horizon scenario stepping through a terminal UI or over an HTTP API

Representative shipped presets include:

- scenarios: `nano_cycle`, `medium_planted`, `warm_room`
- source water: `soft_acidic`, `moderate`, `ro_like`, `hard_shrimp`
- plant guilds: `fast_stem`, `root_rosette`
- substrate presets: `inert_sand`, `inert_gravel`, `active_planted`, `coarse_porous`

## Workspace layout

- `crates/tank_core`: simulation engine, domain types, systems, save/load, events
- `crates/tank_data`: packaged preset data for scenarios, plants, substrate, process defaults, shrimp, and source water
- `crates/tank_scenarios`: scenario materialization and startup overrides
- `crates/tank_api`: Axum-based HTTP server that exposes snapshots, actions, stepping, scenarios, and save/load
- `crates/tank_tui`: terminal UI client that connects to `tank_api`
- `crates/tank_harness`: calibration harness and validation suite
- `docs/`: scientific documentation, contracts, and implementation guidance (see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for reading guide)
- `.beads/issues.jsonl`: authoritative backlog used with `br`

## Scope and fidelity

The simulator is intentionally engine-first and deterministic. It aims to teach what it knows and flag what it is still approximating.

**What the model measures** — deterministic mass-balance chemistry: nitrogen cycling (TAN, nitrite, nitrate with AOB/NOB/comammox guilds), carbonate equilibrium (DIC + alkalinity to pH via closed-form quadratic solver with temperature-corrected pKa1), dissolved oxygen dynamics, and seven tracked major ions (Ca, Mg, Na, K, HCO3, Cl, SO4). Stage-structured shrimp population dynamics with per-stage reserves, condition-dependent molt and reproduction, and mineral stress. Habitat-aware biofilm ecology across five zones with geometry-derived colonizable areas and exposure modifiers.

**What it estimates** — TDS and conductivity are derived from the seven tracked ions, explicitly omitting trace species, organics, and nitrogen/phosphorus species. GH and KH are ion-derived proxies. Free ammonia (NH3) is speciated from TAN using pH and temperature. All estimates are labeled as such in the TUI and API.

**What it deliberately approximates** — a well-mixed water column with no stratification, a quadratic carbonate solver without activity corrections (sufficient for the freshwater 15-35 C envelope), guild-based biology rather than individual-based models, and fixed atmospheric CO2. Microfauna grazing uses a simpler mass-conservation approximation than the full shrimp routing.

**Where parameters are provisional** — the [parameter provenance status](docs/PROVENANCE_STATUS.md) documents which values are literature-anchored, which are expert-curated, and which are heuristic calibration fits. Heuristic-tier parameters (some decomposer rates, plant growth ceilings, shrimp thermal thresholds) are calibrated to literature envelopes but not field-validated. Placeholder-tier parameters must be resolved before any scientific claim depends on them.

Active planning lives in `.beads/issues.jsonl` (use `br list --pretty` or `bv` to browse). For background on the review that motivated the scientific-core upgrade, see [docs/aquarium_sim_review_vnext.md](docs/aquarium_sim_review_vnext.md).

## Quick start

### Prerequisites

- Rust toolchain with Cargo
- optionally, Nix for the provided development shell

The repository includes a simple `flake.nix` that provides stable Rust plus `rust-src`, `rust-analyzer`, and `clippy`.

```bash
nix develop
```

### Build and test

From the repository root:

```bash
cargo build
cargo test
```

### Run the API server

Start the simulation server in one terminal:

```bash
cargo run -p tank_api
```

Defaults:

- scenario: `nano_cycle`
- bind address: `127.0.0.1:3000`

You can also provide both explicitly:

```bash
cargo run -p tank_api -- warm_room 127.0.0.1:3000
```

### Run the TUI

Start the terminal UI in another terminal:

```bash
cargo run -p tank_tui -- http://127.0.0.1:3000
```

The TUI presents a startup configuration flow, materializes the chosen scenario with overrides, pushes that state to the API server, and then drives the simulation remotely.

## API overview

The API is intended for local control and inspection of a running simulation. Representative routes include:

- `GET /snapshot`
- `GET /snapshot/chemistry`
- `GET /snapshot/biology`
- `GET /snapshot/hardware`
- `GET /events`
- `POST /actions`
- `POST /time/step`
- `GET /scenarios`
- `POST /scenarios/load`
- `GET /source-water`
- `GET /save`
- `POST /load`

The full router is implemented in `crates/tank_api/src/routes.rs`.

## Player actions and simulation controls

The current action model supports:

- feeding
- partial water changes with source-water selection
- plant trimming
- detritus siphoning
- filter cleaning
- shrimp add/remove
- photoperiod changes
- light intensity changes
- heater setpoint changes
- ambient temperature changes
- aeration changes

Snapshots expose chemistry, hardware, shrimp population, plant biomass, algae/periphyton, detritus, biofilter state, and recent events for inspection or UI rendering.

## Testing

The repository already contains broad automated coverage across the core engine, scenario materialization, save/load behavior, and end-to-end flows. Existing tests cover areas such as:

- cycling and nitrification behavior
- dissolved oxygen dynamics
- deterministic stepping
- event logging
- water changes and source-water handling
- substrate and plant interactions
- shrimp population and reproduction behavior
- tank-size thermal response
- API/TUI-facing scenario and startup behavior

Run the full suite with:

```bash
cargo test
```

## Backlog and planning

Active planning lives in beads rather than ad hoc documents. Use `br` to inspect the backlog:

```bash
br ready
br show <id>
br list --pretty
br graph
br dep tree <id>
```

The authoritative backlog file is `.beads/issues.jsonl`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
