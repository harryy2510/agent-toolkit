#!/usr/bin/env bun

import { readFileSync, writeFileSync } from 'node:fs'

const nextVersion = Bun.argv[2]

if (!nextVersion || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(nextVersion)) {
	console.error('usage: bun scripts/bump-version.ts <semver>')
	process.exit(1)
}

updatePackageJson(nextVersion)
updateCargoToml(nextVersion)
updateCargoLock(nextVersion)

function updatePackageJson(version: string): void {
	const path = 'package.json'
	const packageJson = JSON.parse(readFileSync(path, 'utf8')) as { version?: string }
	packageJson.version = version
	writeFileSync(path, `${JSON.stringify(packageJson, null, '\t')}\n`)
}

function updateCargoToml(version: string): void {
	const path = 'Cargo.toml'
	const contents = readFileSync(path, 'utf8')
	const updated = replaceRequired(
		contents,
		/(\[workspace\.package\][\s\S]*?\nversion = ")[^"]+(")/,
		version,
		path
	)
	writeFileSync(path, updated)
}

function updateCargoLock(version: string): void {
	const path = 'Cargo.lock'
	let contents = readFileSync(path, 'utf8')
	contents = replacePackageVersion(contents, 'agent-toolkit', version, path)
	contents = replacePackageVersion(contents, 'agent-toolkit-core', version, path)
	writeFileSync(path, contents)
}

function replacePackageVersion(
	contents: string,
	packageName: string,
	version: string,
	path: string
): string {
	const pattern = new RegExp(
		`(\\[\\[package\\]\\]\\r?\\nname = "${escapeRegExp(packageName)}"\\r?\\nversion = ")[^"]+(")`
	)
	return replaceRequired(contents, pattern, version, path)
}

function replaceRequired(contents: string, pattern: RegExp, version: string, path: string): string {
	let replacements = 0
	const updated = contents.replace(pattern, (_match: string, prefix: string, suffix: string) => {
		replacements += 1
		return `${prefix}${version}${suffix}`
	})

	if (replacements !== 1) {
		throw new Error(`Expected exactly one version replacement in ${path}, found ${replacements}`)
	}

	return updated
}

function escapeRegExp(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
