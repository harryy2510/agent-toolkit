# Call Sites

## Read Policy

- Tier: `deep-context`
- Deep-read only. Open for exact API/call-site tracing after a focused target is known.

Function and method call names extracted from the JS/TS AST. Use this to find framework APIs, server helpers, route declarations, and shared utility usage before scanning source.

## Top Calls

- `join` — 4 files
- `Bun.spawnSync` — 3 files
- `binaryName` — 3 files
- `process.exit` — 3 files
- `resolve` — 3 files
- `writeFileSync` — 3 files
- `JSON.stringify` — 2 files
- `console.error` — 2 files
- `describe` — 2 files
- `dirname` — 2 files
- `expect` — 2 files
- `fileURLToPath` — 2 files
- `findNativeBinary` — 2 files
- `mkdirSync` — 2 files
- `mkdtempSync` — 2 files
- `nativeBinaryCandidates` — 2 files
- `nativePlatformKey` — 2 files
- `platformKey` — 2 files
- `readFileSync` — 2 files
- `resolveNativeCommand` — 2 files
- `test` — 2 files
- `tmpdir` — 2 files
- `Bun.argv.slice` — 1 files
- `JSON.parse` — 1 files
- `candidates.push` — 1 files
- `command.command.slice` — 1 files
- `contents.replace` — 1 files
- `copyFileSync` — 1 files
- `escapeRegExp` — 1 files
- `existsSync` — 1 files
- `isSourceCheckout` — 1 files
- `process.cwd` — 1 files
- `process.stdout.write` — 1 files
- `replacePackageVersion` — 1 files
- `replaceRequired` — 1 files
- `rootFile` — 1 files
- `runAgentToolkit` — 1 files
- `runNative` — 1 files
- `updateCargoLock` — 1 files
- `updateCargoToml` — 1 files
- `updatePackageJson` — 1 files
- `value.replace` — 1 files

## By File

### `bin/agent-toolkit.ts`

- `Bun.argv.slice`
- `process.exit`
- `runAgentToolkit`

### `scripts/build-native.ts`

- `Bun.spawnSync`
- `binaryName`
- `copyFileSync`
- `dirname`
- `fileURLToPath`
- `join`
- `mkdirSync`
- `platformKey`
- `process.exit`
- `process.stdout.write`
- `resolve`

### `scripts/bump-version.ts`

- `JSON.parse`
- `JSON.stringify`
- `console.error`
- `contents.replace`
- `escapeRegExp`
- `process.exit`
- `readFileSync`
- `replacePackageVersion`
- `replaceRequired`
- `updateCargoLock`
- `updateCargoToml`
- `updatePackageJson`
- `value.replace`
- `writeFileSync`

### `src/cli.ts`

- `dirname`
- `fileURLToPath`
- `resolve`
- `runNative`

### `src/native.ts`

- `Bun.spawnSync`
- `binaryName`
- `candidates.push`
- `console.error`
- `existsSync`
- `findNativeBinary`
- `isSourceCheckout`
- `join`
- `nativeBinaryCandidates`
- `nativePlatformKey`
- `platformKey`
- `process.cwd`
- `resolveNativeCommand`

### `test/bump-version.test.ts`

- `Bun.spawnSync`
- `JSON.stringify`
- `describe`
- `expect`
- `join`
- `mkdtempSync`
- `readFileSync`
- `resolve`
- `rootFile`
- `test`
- `tmpdir`
- `writeFileSync`

### `test/native.test.ts`

- `binaryName`
- `command.command.slice`
- `describe`
- `expect`
- `findNativeBinary`
- `join`
- `mkdirSync`
- `mkdtempSync`
- `nativeBinaryCandidates`
- `nativePlatformKey`
- `resolveNativeCommand`
- `test`
- `tmpdir`
- `writeFileSync`
