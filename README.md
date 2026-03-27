# Tank Simulation

`tank-simulation` is a Rust workspace for a deterministic freshwater planted shrimp tank simulator. It models the aquarium as an ecosystem rather than a scripted event loop: water chemistry, biofilter succession, plant growth, algae pressure, detritus, ambient temperature, and `Neocaridina davidi` population dynamics all feed back on each other over time.

The current codebase is a strong v0.1 foundation. The engine, data layer, scenarios, TUI, and HTTP API are already separated cleanly, and the repository has meaningful automated coverage. The main remaining work is in the scientific core, not the architecture. A fuller review and next-step plan live in [docs/aquarium_sim_review_vnext.md](docs/aquarium_sim_review_vnext.md).

## What it does

The simulator currently supports:

- deterministic simulation with seed/save-load support
- configurable tank geometry and fill height
- source-water profiles with hardness, alkalinity, and tracked ions
- substrate presets and rooted plant guilds
- nitrogen cycling with distinct microbial groups
- dissolved oxygen, temperature, lighting, heater, filter, and aeration effects
- shrimp stocking and maintenance actions such as feeding, water changes, plant trimming, siphoning, and filter cleaning
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
- `docs/`: scientific notes, design review, and implementation guidance
- `.beads/issues.jsonl`: authoritative backlog used with `br`

## Status

The project is intentionally engine-first and deterministic, and it already covers many of the behaviors that matter for a tank sim: cycling, oxygen stress, ambient temperature shifts, tank-size thermal response, source-water changes, substrate effects, and shrimp population outcomes.

The main known limitations are documented rather than hidden:

- several kinetics still need concentration-normalized treatment instead of total-mass treatment
- some grazing and detritus paths still need tighter mass conservation
- pH and carbonate chemistry are still simplified
- habitat-specific biofilms, denitrification, and deeper shrimp ecology are planned but not fully implemented

A structured scientific roadmap covering units/kinetics normalization, mass conservation, carbonate chemistry, habitat-aware ecology, shrimp life history, and calibration is tracked as an 80-bead backlog in `.beads/issues.jsonl` (use `br list --pretty` or `bv` to browse). For background on the review that motivated the roadmap, see [docs/aquarium_sim_review_vnext.md](docs/aquarium_sim_review_vnext.md) and [docs/scientific_specs.md](docs/scientific_specs.md).

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
