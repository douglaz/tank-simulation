---
artifact: completer-verdict
loop: 10
project: tank-sim
backend: codex(gpt-5.4-xhigh)
role: completer
created_at: 2026-03-15T08:06:39Z
---

Reviewing `prompt.md`, the workspace structure, and the implemented startup path from the code itself. I’m checking this as a completion gate, not a planner artifact review, so I’ll verify the mandatory surface area and the acceptance tests directly.
`prompt.md` is not present at repo root, so I’m locating the actual requirements source before treating that as a failure. Next I’m reading the implementation and tests against the prompt text the task supplied, plus any in-repo equivalent.
The repo’s requirements file exists under [`.ralph/projects/tank-sim/prompt.md`](/root/tank-simulation/.ralph/projects/tank-sim/prompt.md), not at root. I’m using that plus the prompt text in this task, then checking whether the code satisfies the mandatory contract rather than assuming the tests are exhaustive.
I have the core types and save/load contract. Next pass is the systems layer and the TUI surface, because that’s where completion gaps usually hide even when the acceptance tests are present.
The systems pass is mostly there, but I still need to prove the implementation behaves, not just exists. I’m switching to verification now: toolchain check, then targeted/full tests, and I’ll treat any inability to run them as evidence rather than ignoring it.
The first verification run failed for environment reasons, not repo logic: `rustc` could start compiling but couldn’t find `cc`. I’m wiring a proper Nix linker path and rerunning, because completion shouldn’t rest on a broken shell PATH.
I’ve found at least one substantive prompt gap that the current test suite doesn’t catch: substrate layers carry `detritus_trapping_index` and `grazing_surface_index`, but those fields never feed any simulation behavior. I’m confirming line-level evidence for that and checking whether the startup filter choice has the same problem.
# Verdict: CONTINUE

## Missing Requirements
1. `Substrate` layer effects are incomplete. The prompt requires each layer to affect detritus trapping and a shrimp grazing-surface proxy, but those fields only exist in [substrate.rs](/root/tank-simulation/crates/tank_core/src/types/substrate.rs#L12) and are never consumed by the simulation logic. The current systems use CEC/nutrient stores in [plant_growth.rs](/root/tank-simulation/crates/tank_core/src/systems/plant_growth.rs#L110), colonizable area in [algae_growth.rs](/root/tank-simulation/crates/tank_core/src/systems/algae_growth.rs#L64), and generic periphyton/detritus food in [shrimp.rs](/root/tank-simulation/crates/tank_core/src/systems/shrimp.rs#L119), so changing `detritus_trapping_index` or `grazing_surface_index` does not change behavior.
2. Startup `filter` selection is not implemented as a causal feature. The startup path sets `FilterHardware.enabled` and `flow_lph` in [lib.rs](/root/tank-simulation/crates/tank_scenarios/src/lib.rs#L562), and the fields exist in [hardware.rs](/root/tank-simulation/crates/tank_core/src/types/hardware.rs#L23), but there are no core-system read sites for either field. A user can toggle the filter at launch, but that choice does not alter cycling, oxygenation, or stability.

## Recommended Next Features
1. Wire `detritus_trapping_index` into detritus routing/mineralization and `grazing_surface_index` into periphyton capacity or shrimp food access, then add regression tests proving substrate presets diverge because of those parameters.
2. Make `filter.enabled` and/or `flow_lph` affect nitrifier performance, gas exchange, and cleaning/clogging behavior, then add a startup-override test showing filter-on and filter-off tanks evolve differently.
