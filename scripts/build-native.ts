#!/usr/bin/env bun

import { createHash } from 'node:crypto'
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	writeFileSync
} from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { binaryName, platformKey } from '../src/platform.ts'

const buildManifestName = '.agent-toolkit-build.json'
const buildManifestVersion = 1

type BuildNativeOptions = {
	arch?: string
	env?: Record<string, string | undefined>
	packageRoot?: string
	platform?: NodeJS.Platform
	spawnSync?: BuildSpawnSync
	targetKey?: string
}

type BuildSpawnOptions = {
	cmd: Array<string>
	cwd: string
	stderr: 'inherit'
	stdin: 'inherit'
	stdout: 'inherit'
}

type BuildSpawnSync = (options: BuildSpawnOptions) => {
	exitCode: number | null
}

type BuildManifest = {
	executable: string
	inputHash: string
	schemaVersion: number
	target: string
}

if (import.meta.main) {
	const exitCode = buildNative()
	if (exitCode !== 0) {
		process.exit(exitCode)
	}
}

export function buildNative(options: BuildNativeOptions = {}): number {
	const env = options.env ?? process.env
	const packageRoot = options.packageRoot ?? defaultPackageRoot()
	const key =
		options.targetKey ?? env.AGENT_TOOLKIT_TARGET ?? platformKey(options.platform, options.arch)
	const executable = binaryName(options.platform)
	const source = join(packageRoot, 'target', 'release', executable)
	const destinationDirectory = join(packageRoot, 'bin', 'native', key)
	const destination = join(destinationDirectory, executable)
	const manifestPath = join(destinationDirectory, buildManifestName)
	const inputHash = nativeBuildInputHash(packageRoot)
	const manifest: BuildManifest = {
		executable,
		inputHash,
		schemaVersion: buildManifestVersion,
		target: key
	}

	if (!forceBuild(env) && cachedArtifactMatches(manifestPath, destination, manifest)) {
		process.stdout.write(`reused ${destination}\n`)
		return 0
	}

	const spawnSync =
		options.spawnSync ?? ((buildOptions: BuildSpawnOptions) => Bun.spawnSync(buildOptions))
	const build = spawnSync({
		cmd: ['cargo', 'build', '--release', '-p', 'agent-toolkit'],
		cwd: packageRoot,
		stdin: 'inherit',
		stdout: 'inherit',
		stderr: 'inherit'
	})

	if (build.exitCode !== 0) {
		return build.exitCode ?? 1
	}

	if (!existsSync(source)) {
		process.stderr.write(`expected build output at ${source}\n`)
		return 1
	}

	mkdirSync(destinationDirectory, { recursive: true })
	copyFileSync(source, destination)
	writeFileSync(manifestPath, `${JSON.stringify(manifest, null, '\t')}\n`)
	process.stdout.write(`wrote ${destination}\n`)
	return 0
}

export function nativeBuildInputHash(packageRoot: string): string {
	const hash = createHash('sha256')
	const includePackageVersion = nativeSourceUsesCargoPackageVersion(packageRoot)
	hash.update(`agent-toolkit-build-native-v${buildManifestVersion}\n`)

	for (const file of nativeBuildInputFiles(packageRoot)) {
		const relativePath = relative(packageRoot, file).replaceAll('\\', '/')
		hash.update(relativePath)
		hash.update('\0')
		hash.update(nativeBuildInputContents(file, relativePath, includePackageVersion))
		hash.update('\0')
	}

	return hash.digest('hex')
}

export function nativeBuildInputFiles(packageRoot: string): Array<string> {
	const files = ['Cargo.lock', 'Cargo.toml', 'scripts/build-native.ts', 'src/platform.ts'].flatMap(
		(path) => existingFile(join(packageRoot, path))
	)

	for (const directory of ['crates', '.cargo']) {
		files.push(...collectFiles(join(packageRoot, directory)))
	}

	return files.sort()
}

function cachedArtifactMatches(
	manifestPath: string,
	destination: string,
	expected: BuildManifest
): boolean {
	if (!existsSync(destination) || !existsSync(manifestPath)) {
		return false
	}

	try {
		const manifest = JSON.parse(readFileSync(manifestPath, 'utf8')) as Partial<BuildManifest>
		return (
			manifest.schemaVersion === expected.schemaVersion &&
			manifest.target === expected.target &&
			manifest.executable === expected.executable &&
			manifest.inputHash === expected.inputHash
		)
	} catch {
		return false
	}
}

function collectFiles(directory: string): Array<string> {
	if (!existsSync(directory)) {
		return []
	}

	return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const path = join(directory, entry.name)
		if (entry.isDirectory()) {
			return collectFiles(path)
		}
		if (entry.isFile()) {
			return [path]
		}
		return []
	})
}

function defaultPackageRoot(): string {
	return resolve(dirname(fileURLToPath(import.meta.url)), '..')
}

function existingFile(path: string): Array<string> {
	return existsSync(path) ? [path] : []
}

function forceBuild(env: Record<string, string | undefined>): boolean {
	const value = env.AGENT_TOOLKIT_BUILD_NATIVE_FORCE
	return value === '1' || value === 'true'
}

function nativeBuildInputContents(
	file: string,
	relativePath: string,
	includePackageVersion: boolean
): Buffer | string {
	if (includePackageVersion) {
		return readFileSync(file)
	}

	if (relativePath === 'Cargo.toml') {
		return readFileSync(file, 'utf8').replace(
			/(\[workspace\.package\][\s\S]*?\r?\nversion = ")[^"]+(")/,
			'$1<workspace-version>$2'
		)
	}

	if (relativePath === 'Cargo.lock') {
		return normalizeWorkspacePackageVersions(readFileSync(file, 'utf8'))
	}

	return readFileSync(file)
}

function nativeSourceUsesCargoPackageVersion(packageRoot: string): boolean {
	return collectFiles(join(packageRoot, 'crates')).some((file) =>
		readFileSync(file, 'utf8').includes('CARGO_PKG_')
	)
}

function normalizeWorkspacePackageVersions(contents: string): string {
	return ['agent-toolkit', 'agent-toolkit-core'].reduce(
		(updated, packageName) =>
			updated.replace(
				new RegExp(
					`(\\[\\[package\\]\\]\\r?\\nname = "${packageName}"\\r?\\nversion = ")[^"]+(")`,
					'g'
				),
				'$1<workspace-version>$2'
			),
		contents
	)
}
