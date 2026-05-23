import { describe, expect, test } from 'bun:test'
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
	findNativeBinary,
	nativeBinaryCandidates,
	nativePlatformKey,
	resolveNativeCommand
} from '../src/native.ts'
import { binaryName } from '../src/platform.ts'

describe('native binary resolution', () => {
	test('puts AGENT_TOOLKIT_NATIVE first when provided', () => {
		const candidates = nativeBinaryCandidates({
			packageRoot: '/toolkit',
			env: {
				AGENT_TOOLKIT_NATIVE: '/custom/agent-toolkit'
			}
		})

		expect(candidates[0]).toBe('/custom/agent-toolkit')
	})

	test('finds the bundled platform binary when present', () => {
		const root = mkdtempSync(join(tmpdir(), 'agent-toolkit-native-'))
		const binary = join(root, 'bin', 'native', nativePlatformKey(), binaryName())
		mkdirSync(join(root, 'bin', 'native', nativePlatformKey()), { recursive: true })
		writeFileSync(binary, '')

		expect(findNativeBinary({ packageRoot: root, env: {} })).toBe(binary)
	})

	test('runs bundled native binary from caller cwd', () => {
		const root = mkdtempSync(join(tmpdir(), 'agent-toolkit-native-cwd-'))
		const callerCwd = mkdtempSync(join(tmpdir(), 'agent-toolkit-caller-cwd-'))
		const binary = join(root, 'bin', 'native', nativePlatformKey(), binaryName())
		mkdirSync(join(root, 'bin', 'native', nativePlatformKey()), { recursive: true })
		writeFileSync(binary, '')

		const command = resolveNativeCommand(['repo', 'migrate'], {
			packageRoot: root,
			cwd: callerCwd,
			env: {}
		})

		expect(command.command).toEqual([binary, 'repo', 'migrate'])
		expect(command.cwd).toBe(callerCwd)
		expect(command.error).toBeNull()
	})

	test('does not fall back to Cargo in an installed package without a native binary', () => {
		const root = mkdtempSync(join(tmpdir(), 'agent-toolkit-installed-'))
		writeFileSync(join(root, 'Cargo.toml'), '[workspace]\n')

		const command = resolveNativeCommand(['repo', 'check'], {
			packageRoot: root,
			env: {}
		})

		expect(command.command).toBeNull()
		expect(command.cwd).toBeNull()
		expect(command.error).toContain('No bundled agent-toolkit native binary')
		expect(command.error).toContain(nativePlatformKey())
	})

	test('allows Cargo fallback only inside a source checkout', () => {
		const root = mkdtempSync(join(tmpdir(), 'agent-toolkit-source-'))
		const callerCwd = mkdtempSync(join(tmpdir(), 'agent-toolkit-caller-cwd-'))
		mkdirSync(join(root, '.git'), { recursive: true })
		writeFileSync(join(root, 'Cargo.toml'), '[workspace]\n')

		const command = resolveNativeCommand(['repo', 'check'], {
			packageRoot: root,
			cwd: callerCwd,
			env: {}
		})

		expect(command.command).toEqual([
			'cargo',
			'run',
			'--manifest-path',
			join(root, 'Cargo.toml'),
			'-p',
			'agent-toolkit',
			'--quiet',
			'--',
			'repo',
			'check'
		])
		expect(command.cwd).toBe(callerCwd)
		expect(command.error).toBeNull()
	})

	test('uses Cargo in source checkouts even when stale target binaries exist', () => {
		const root = mkdtempSync(join(tmpdir(), 'agent-toolkit-source-stale-'))
		const callerCwd = mkdtempSync(join(tmpdir(), 'agent-toolkit-caller-cwd-'))
		mkdirSync(join(root, '.git'), { recursive: true })
		mkdirSync(join(root, 'target', 'release'), { recursive: true })
		writeFileSync(join(root, 'Cargo.toml'), '[workspace]\n')
		writeFileSync(join(root, 'target', 'release', binaryName()), '')

		const command = resolveNativeCommand(['repo', 'check'], {
			packageRoot: root,
			cwd: callerCwd,
			env: {}
		})

		expect(command.command?.slice(0, 8)).toEqual([
			'cargo',
			'run',
			'--manifest-path',
			join(root, 'Cargo.toml'),
			'-p',
			'agent-toolkit',
			'--quiet',
			'--'
		])
		expect(command.cwd).toBe(callerCwd)
		expect(command.error).toBeNull()
	})
})
