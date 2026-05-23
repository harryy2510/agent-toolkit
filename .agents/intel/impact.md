# Change Impact Read Plans

Use this before editing high-impact files. Each plan lists the file itself, direct dependencies, direct dependents, related tests, and the intel articles that usually matter.

## `src/platform.ts`

- Imported by: 3 local files
- Tags: ast
- Read first:
  - `src/platform.ts`
- Check direct dependents:
  - `scripts/build-native.ts`
  - `src/native.ts`
  - `test/native.test.ts`
- Relevant intel: `graph.md`, `symbols.md`, `files.md`

## `src/native.ts`

- Imported by: 2 local files
- Tags: ast
- Read first:
  - `src/native.ts`
  - `src/platform.ts`
- Check direct dependents:
  - `src/cli.ts`
  - `test/native.test.ts`
- Related tests:
  - `test/native.test.ts`
- Relevant intel: `graph.md`, `symbols.md`, `files.md`

## `src/cli.ts`

- Imported by: 1 local files
- Tags: ast
- Read first:
  - `src/cli.ts`
  - `src/native.ts`
- Check direct dependents:
  - `bin/agent-toolkit.ts`
- Relevant intel: `graph.md`, `symbols.md`, `files.md`
