# AGENTS.md

## Issue Tracking

**Note:** `br` is non-invasive and never executes git commands. After `br sync --flush-only`, you must manually run `git add .beads/ && git commit`.

This repository uses `br` (beads_rust) and `bv` for issue tracking and backlog inspection.

- The authoritative backlog lives in `.beads/issues.jsonl`.
- The original project/spec reference materials remain in `docs/` (for example prior scientific review/spec notes), but active task planning and execution state live in the beads.
- Read task context from the beads themselves with `br show <id>`.
- Use `br ready` to find the next unblocked task.
- Use `br list --pretty` or `bv` to inspect the backlog more broadly.
- Run `br lint` after backlog edits.
- Keep task/spike/epic templates clean without duplicating criteria verbatim:
  use the structured criteria field as the source of truth, and if a description needs an `## Acceptance Criteria` or `## Success Criteria` heading for lint/template reasons, make that section a short pointer rather than repeating the same bullet list.
- Use `br graph` or `br dep tree <id>` when dependency shape matters.
- Keep task knowledge in the beads. Do not recreate parallel backlog scripts or mirror plan docs unless explicitly requested.

Common commands:

```bash
br ready
br show <id>
br list --pretty
br lint
br graph
br dep tree <id>
bv
br sync --flush-only
git add .beads/
git commit -m "sync beads"
```

## Working Norms

- Follow the current bead descriptions and comments; they contain the roadmap, assumptions, testing expectations, and deferred-scope notes.
- When revising or adding beads, preserve feature scope while making verification concrete: specify deterministic unit tests for local logic, integration tests for cross-system changes, and e2e/scenario scripts for long-horizon or player-visible behavior.
- Treat structured tracing/logging as part of the deliverable for scientific-core work. Reuse shared regression/e2e harness conventions rather than inventing one-off scenario runners per bead.
- Keep tests deterministic and add right-sized coverage for any code change: unit tests for local logic, integration tests for cross-system behavior, and scenario/e2e coverage when long-horizon behavior changes.
- Prefer explicit, inspectable behavior over hidden tuning. If diagnostics or tracing are needed, wire them in cleanly rather than relying on ad hoc prints.
- If a change affects persisted state or action semantics, update save/migration handling and the relevant bead context.

<!-- bv-agent-instructions-v2 -->

---

## Beads Workflow Integration

This project uses [beads_rust](https://github.com/Dicklesworthstone/beads_rust) (`br`) for issue tracking and [beads_viewer](https://github.com/Dicklesworthstone/beads_viewer) (`bv`) for graph-aware triage. The authoritative backlog export is `.beads/issues.jsonl`; `bv` reads project state from `.beads/` and should be treated as a read-only sidecar.

### Using bv as an AI sidecar

bv is a graph-aware triage engine for this Beads project. For agent and CI usage, prefer robot flags instead of parsing JSONL or hallucinating graph traversal; they provide deterministic, dependency-aware outputs with precomputed metrics (PageRank, betweenness, critical path, cycles, HITS, eigenvector, k-core).

**Scope boundary:** bv handles *what to work on* (triage, priority, planning). `br` handles creating, modifying, and closing beads.

**Interactive note:** bare `bv` launches the interactive TUI. Use it for manual inspection only; in automated or agent sessions prefer `--robot-*` flags so the command returns non-interactive output.

#### The Workflow: Start With Triage

**`bv --robot-triage` is your single entry point.** It returns everything you need in one call:
- `quick_ref`: at-a-glance counts + top 3 picks
- `recommendations`: ranked actionable items with scores, reasons, unblock info
- `quick_wins`: low-effort high-impact items
- `blockers_to_clear`: items that unblock the most downstream work
- `project_health`: status/type/priority distributions, graph metrics
- `commands`: copy-paste shell commands for next steps

```bash
bv --robot-triage        # THE MEGA-COMMAND: start here
bv --robot-next          # Minimal: just the single top pick + claim command

# Token-optimized output (TOON) for lower LLM context usage:
bv --robot-triage --format toon
```

#### Other bv Commands

| Command | Returns |
|---------|---------|
| `--robot-plan` | Parallel execution tracks with unblocks lists |
| `--robot-priority` | Priority misalignment detection with confidence |
| `--robot-insights` | Full metrics: PageRank, betweenness, HITS, eigenvector, critical path, cycles, k-core |
| `--robot-alerts` | Stale issues, blocking cascades, priority mismatches |
| `--robot-suggest` | Hygiene: duplicates, missing deps, label suggestions, cycle breaks |
| `--robot-diff --diff-since <ref>` | Changes since ref: new/closed/modified issues |
| `--robot-graph [--graph-format=json\|dot\|mermaid]` | Dependency graph export |

#### Scoping & Filtering

```bash
bv --robot-plan --label backend              # Scope to label's subgraph
bv --robot-insights --as-of HEAD~30          # Historical point-in-time
bv --recipe actionable --robot-plan          # Pre-filter: ready to work (no blockers)
bv --recipe high-impact --robot-triage       # Pre-filter: top PageRank scores
```

### br Commands for Issue Management

```bash
br ready              # Show issues ready to work (no blockers)
br list --status=open # All open issues
br show <id>          # Full issue details with dependencies
br create --title="..." --type=task --priority=2
br update <id> --status=in_progress
br close <id> --reason="Completed"
br close <id1> <id2>  # Close multiple issues at once
br sync --flush-only  # Export DB to JSONL
```

### Workflow Pattern

1. **Triage**: Run `bv --robot-triage` to find the highest-impact actionable work
2. **Claim**: Use `br update <id> --status=in_progress`
3. **Work**: Implement the task
4. **Complete**: Use `br close <id>`
5. **Sync**: Always run `br sync --flush-only` at session end

### Key Concepts

- **Dependencies**: Issues can block other issues. `br ready` shows only unblocked work.
- **Priority**: P0=critical, P1=high, P2=medium, P3=low, P4=backlog (use numbers 0-4, not words)
- **Types**: task, bug, feature, epic, chore, docs, question
- **Blocking**: `br dep add <issue> <depends-on>` to add dependencies

### Session Protocol

```bash
git status              # Check what changed
git add <files>         # Stage code changes
br sync --flush-only    # Export beads changes to JSONL
git commit -m "..."     # Commit everything
git push                # Push to remote
```

<!-- end-bv-agent-instructions -->
