import { describe, expect, test } from 'bun:test'
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, relative } from 'node:path'

import {
	buildNative,
	nativeBuildInputFiles,
	nativeBuildInputHash
} from '../scripts/build-native.ts'

const manifestName = '.agent-toolkit-build.json'

describe('build-native cache', () => {
	test('hashes native source and build script inputs', () => {
		const root = buildFixture()
		const before = nativeBuildInputHash(root)

		writeFileSync(rootFile(root, 'crates/agent-toolkit-cli/src/main.rs'), 'fn main() {}\n')

		expect(nativeBuildInputHash(root)).not.toBe(before)
		expect(relativeInputFiles(root)).toContain('scripts/build-native.ts')
		expect(relativeInputFiles(root)).toContain('crates/agent-toolkit-cli/src/main.rs')
	})

	test('keeps package version-only bumps cacheable when native source does not read it', () => {
		const root = buildFixture()
		const before = nativeBuildInputHash(root)

		bumpWorkspaceVersion(root, '0.1.1')

		expect(nativeBuildInputHash(root)).toBe(before)
	})

	test('includes package version when native source reads Cargo package metadata', () => {
		const root = buildFixture()
		writeFileSync(
			rootFile(root, 'crates/agent-toolkit-cli/src/main.rs'),
			'fn main() { println!("{}", env!("CARGO_PKG_VERSION")); }\n'
		)
		const before = nativeBuildInputHash(root)

		bumpWorkspaceVersion(root, '0.1.1')

		expect(nativeBuildInputHash(root)).not.toBe(before)
	})

	test('reuses a matching native artifact without spawning Cargo', () => {
		const root = buildFixture()
		const targetKey = 'darwin-arm64'
		const destination = nativeArtifact(root, targetKey)
		const manifest = nativeManifest(root, targetKey)
		mkdirSync(join(root, 'bin/native', targetKey), { recursive: true })
		writeFileSync(destination, 'cached')
		writeFileSync(manifest, buildManifest(root, targetKey))
		let builds = 0

		const exitCode = buildNative({
			env: {},
			packageRoot: root,
			platform: 'darwin',
			spawnSync: () => {
				builds += 1
				return { exitCode: 0 }
			},
			targetKey
		})

		expect(exitCode).toBe(0)
		expect(builds).toBe(0)
		expect(readFileSync(destination, 'utf8')).toBe('cached')
	})

	test('rebuilds stale native artifacts and refreshes the manifest', () => {
		const root = buildFixture()
		const targetKey = 'darwin-arm64'
		const destination = nativeArtifact(root, targetKey)
		const manifest = nativeManifest(root, targetKey)
		mkdirSync(join(root, 'bin/native', targetKey), { recursive: true })
		mkdirSync(join(root, 'target/release'), { recursive: true })
		writeFileSync(destination, 'stale')
		writeFileSync(join(root, 'target/release/agent-toolkit'), 'fresh')
		writeFileSync(
			manifest,
			`${JSON.stringify({
				executable: 'agent-toolkit',
				inputHash: 'old',
				schemaVersion: 1,
				target: targetKey
			})}\n`
		)
		let builds = 0

		const exitCode = buildNative({
			env: {},
			packageRoot: root,
			platform: 'darwin',
			spawnSync: () => {
				builds += 1
				return { exitCode: 0 }
			},
			targetKey
		})

		const updatedManifest = JSON.parse(readFileSync(manifest, 'utf8')) as { inputHash?: string }
		expect(exitCode).toBe(0)
		expect(builds).toBe(1)
		expect(readFileSync(destination, 'utf8')).toBe('fresh')
		expect(updatedManifest.inputHash).toBe(nativeBuildInputHash(root))
	})
})

function buildFixture(): string {
	const root = mkdtempSync(join(tmpdir(), 'agent-toolkit-build-native-'))
	writeRootFile(
		root,
		'Cargo.lock',
		[
			'[[package]]',
			'name = "agent-toolkit"',
			'version = "0.1.0"',
			'',
			'[[package]]',
			'name = "agent-toolkit-core"',
			'version = "0.1.0"',
			''
		].join('\n')
	)
	writeRootFile(
		root,
		'Cargo.toml',
		['[workspace]', '', '[workspace.package]', 'version = "0.1.0"', ''].join('\n')
	)
	writeRootFile(root, 'scripts/build-native.ts', '#!/usr/bin/env bun\n')
	writeRootFile(
		root,
		'src/platform.ts',
		'export function binaryName() { return "agent-toolkit" }\n'
	)
	writeRootFile(root, 'crates/agent-toolkit-cli/Cargo.toml', '[package]\n')
	writeRootFile(root, 'crates/agent-toolkit-cli/src/main.rs', 'fn main() { println!("old"); }\n')
	writeRootFile(root, 'crates/agent-toolkit-core/Cargo.toml', '[package]\n')
	writeRootFile(root, 'crates/agent-toolkit-core/src/lib.rs', 'pub fn check() {}\n')
	return root
}

function bumpWorkspaceVersion(root: string, version: string): void {
	writeRootFile(
		root,
		'Cargo.toml',
		readFileSync(rootFile(root, 'Cargo.toml'), 'utf8').replace(
			/version = "0\.1\.0"/,
			`version = "${version}"`
		)
	)
	writeRootFile(
		root,
		'Cargo.lock',
		readFileSync(rootFile(root, 'Cargo.lock'), 'utf8').replaceAll(
			'version = "0.1.0"',
			`version = "${version}"`
		)
	)
}

function buildManifest(root: string, targetKey: string): string {
	return `${JSON.stringify(
		{
			executable: 'agent-toolkit',
			inputHash: nativeBuildInputHash(root),
			schemaVersion: 1,
			target: targetKey
		},
		null,
		'\t'
	)}\n`
}

function nativeArtifact(root: string, targetKey: string): string {
	return rootFile(root, `bin/native/${targetKey}/agent-toolkit`)
}

function nativeManifest(root: string, targetKey: string): string {
	return rootFile(root, `bin/native/${targetKey}/${manifestName}`)
}

function relativeInputFiles(root: string): Array<string> {
	return nativeBuildInputFiles(root).map((path) => relative(root, path).replaceAll('\\', '/'))
}

function rootFile(root: string, path: string): string {
	return join(root, path)
}

function writeRootFile(root: string, path: string, contents: string): void {
	const file = rootFile(root, path)
	mkdirSync(dirname(file), { recursive: true })
	writeFileSync(file, contents)
}
