# Task Read Paths

Use this file to avoid broad project scans. Pick the task type, read the listed intel article, then read the source files named there.

## Universal Start

- Read `overview.md` for stack, scale, high-impact files, generated files, and large modules.
- Read `tooling.md` before running commands or changing package/config surfaces.
- Read `graph.md` before touching a high-impact shared module.
- Do not open `repo.json`, `database.md`, `imports.md`, or `calls.md` unless the current task specifically needs deep structured context.

## By Task Type

- **UI component or screen work**: `components.md, routes.md, graph.md` — Find component ownership, route entrypoints, props, and shared UI dependencies.
- **Route behavior or navigation**: `routes.md, api.md, graph.md` — Find route files, server/API dependencies, and shared modules used by the route.
- **API/server function work**: `api.md, data.md, env.md, graph.md` — Find server modules, schema/data dependencies, env requirements, and blast radius.
- **Database or migration work**: `data.md, env.md, testing.md` — Find SQL objects, migration files, data helpers, and test scripts.
- **Auth/config/secrets work**: `env.md, tooling.md, api.md` — Find env names and files that reference them without exposing values.
- **Refactor/shared helper work**: `impact.md, graph.md, symbols.md, imports.md, files.md` — Find import fan-in/fan-out, exported symbols, and all files in the affected area.
- **Test work**: `testing.md, files.md, graph.md` — Find existing tests, test scripts, and nearby source/test boundaries.

## Highest Blast-Radius Files

- `src/platform.ts` — read before changing; imported by 3 local files
- `src/native.ts` — read before changing; imported by 2 local files
- `src/cli.ts` — read before changing; imported by 1 local files
