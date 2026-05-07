# Import Graph

## Path Aliases

- `@/` -> `src/`
- `~/` -> `src/`

## High-Impact Files

- `src/platform.ts` — imported by 3 local files
- `src/native.ts` — imported by 2 local files
- `src/cli.ts` — imported by 1 local files

## Import-Heavy Files

- `crates/agent-toolkit-cli/src/main.rs` — 12 imports
- `crates/agent-toolkit-core/src/intel.rs` — 10 imports
- `crates/agent-toolkit-core/src/migrate.rs` — 7 imports
- `crates/agent-toolkit-core/src/dotagent.rs` — 6 imports
- `test/native.test.ts` — 6 imports
- `crates/agent-toolkit-core/src/check.rs` — 5 imports
- `crates/agent-toolkit-core/src/global_setup.rs` — 5 imports
- `crates/agent-toolkit-core/src/hooks.rs` — 5 imports
- `crates/agent-toolkit-core/src/supabase.rs` — 4 imports
- `scripts/build-native.ts` — 4 imports
- `test/bump-version.test.ts` — 4 imports
- `crates/agent-toolkit-core/src/fleet.rs` — 3 imports
- `src/cli.ts` — 3 imports
- `src/native.ts` — 3 imports
- `bin/agent-toolkit.ts` — 1 imports
- `scripts/bump-version.ts` — 1 imports

## Local Edges Sample

- `bin/agent-toolkit.ts` -> `src/cli.ts`
- `scripts/build-native.ts` -> `src/platform.ts`
- `src/cli.ts` -> `src/native.ts`
- `src/native.ts` -> `src/platform.ts`
- `test/native.test.ts` -> `src/native.ts`
- `test/native.test.ts` -> `src/platform.ts`
