# Repository Intelligence

## Preferred Context

- Start here and keep startup context small; this file is an index, not a source dump.
- Open `overview.md` and `tasks.md` first, then only the targeted article needed for the task.
- Treat `database.md`, `imports.md`, `calls.md`, and `repo.json` as deep-read only.
- This index is deterministic and local-only. It is a map, not a substitute for reading implementation before editing.

## Read Policy

### Entrypoints

- [`summary.md`](./summary.md) — tiny startup index and read policy
- [`index.md`](./index.md) — same startup index as summary.md

### Short Indexes

- [`overview.md`](./overview.md) — architecture, scale, high-impact files
- [`tasks.md`](./tasks.md) — task-oriented read paths so agents know where to start

### Targeted Context

- [`tooling.md`](./tooling.md) — scripts, configs, package dependencies
- [`routes.md`](./routes.md) — framework route files and route-like modules
- [`api.md`](./api.md) — API handlers, server functions, endpoint declarations
- [`components.md`](./components.md) — UI components and prop surfaces
- [`data.md`](./data.md) — SQL schema, migrations, Supabase/data files
- [`graph.md`](./graph.md) — import graph, blast radius, central modules
- [`impact.md`](./impact.md) — change-impact read plans by high-risk file
- [`boundaries.md`](./boundaries.md) — client/server/data/generated boundary signals
- [`dependencies.md`](./dependencies.md) — external imports and package usage
- [`symbols.md`](./symbols.md) — exported symbols by source file
- [`files.md`](./files.md) — full source-like file inventory with tags
- [`env.md`](./env.md) — environment variable usage by file
- [`testing.md`](./testing.md) — tests, coverage signals, test-adjacent files

### Deep Context

- [`database.md`](./database.md) — pg_query-backed static database design, relationships, RLS, RPCs
- [`imports.md`](./imports.md) — local import adjacency grouped by source file
- [`calls.md`](./calls.md) — AST-derived function and method call sites by source file

### Machine Deep Context

- [`repo.json`](./repo.json) — machine-readable full repo intelligence payload

## Quick Stats

- Source-like files: 28
- Frameworks/signals: none detected
- Routes: 1
- API endpoints/modules: 1
- Components: 0
- SQL objects: 0
- Env vars: 3
- Local import edges: 6
- AST-parsed JS/TS files: 8
- Call-site files: 7

## How To Use

- Start with `summary.md`, `overview.md`, and `tasks.md`, then jump to the article that matches your task.
- For behavior changes, read the listed source files before editing.
- For broad refactors, inspect `graph.md` high-impact files first.
- For framework-specific work, prefer the matching route/API/component/data article over global search.
- Do not bulk-read deep context files such as `repo.json`, `database.md`, `imports.md`, or `calls.md` unless the task specifically requires them.
