import { existsSync } from 'node:fs'
import { join } from 'node:path'

import { binaryName, platformKey } from './platform.ts'

export type NativeRunOptions = {
	packageRoot: string
	cwd?: string
	env?: Record<string, string | undefined>
	platform?: NodeJS.Platform
	arch?: string
}

export type NativeCommandResolution = {
	command: Array<string> | null
	cwd: string | null
	error: string | null
}

export function nativePlatformKey(
	options: Pick<NativeRunOptions, 'arch' | 'platform'> = {}
): string {
	return platformKey(options.platform, options.arch)
}

export function nativeBinaryCandidates(options: NativeRunOptions): Array<string> {
	const env = options.env ?? process.env
	const candidates: Array<string> = []

	if (env.AGENT_TOOLKIT_NATIVE) {
		candidates.push(env.AGENT_TOOLKIT_NATIVE)
	}

	const executableName = binaryName(options.platform)
	candidates.push(
		join(options.packageRoot, 'bin', 'native', nativePlatformKey(options), executableName)
	)

	return candidates
}

export function findNativeBinary(options: NativeRunOptions): string | null {
	return nativeBinaryCandidates(options).find((candidate) => existsSync(candidate)) ?? null
}

export function resolveNativeCommand(
	args: Array<string>,
	options: NativeRunOptions
): NativeCommandResolution {
	const callerCwd = options.cwd ?? process.cwd()
	const env = options.env ?? process.env
	const explicitBinary = env.AGENT_TOOLKIT_NATIVE
	if (explicitBinary && existsSync(explicitBinary)) {
		return {
			command: [explicitBinary, ...args],
			cwd: callerCwd,
			error: null
		}
	}

	if (isSourceCheckout(options.packageRoot)) {
		return {
			command: [
				'cargo',
				'run',
				'--manifest-path',
				join(options.packageRoot, 'Cargo.toml'),
				'-p',
				'agent-toolkit',
				'--quiet',
				'--',
				...args
			],
			cwd: callerCwd,
			error: null
		}
	}

	const binary = findNativeBinary(options)
	if (binary) {
		return {
			command: [binary, ...args],
			cwd: callerCwd,
			error: null
		}
	}

	return {
		command: null,
		cwd: null,
		error: [
			`No bundled agent-toolkit native binary found for ${nativePlatformKey(options)}.`,
			'Install a supported release or build from source in a git checkout.',
			'End users should not need Rust installed.'
		].join(' ')
	}
}

export async function runNative(args: Array<string>, options: NativeRunOptions): Promise<number> {
	const resolution = resolveNativeCommand(args, options)
	if (!resolution.command) {
		console.error(resolution.error)
		return 1
	}

	const result = Bun.spawnSync({
		cmd: resolution.command,
		cwd: resolution.cwd ?? undefined,
		env: { ...process.env, ...options.env },
		stdin: 'inherit',
		stdout: 'inherit',
		stderr: 'inherit'
	})

	return result.exitCode
}

function isSourceCheckout(packageRoot: string): boolean {
	return existsSync(join(packageRoot, '.git')) && existsSync(join(packageRoot, 'Cargo.toml'))
}
