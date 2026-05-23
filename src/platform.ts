export function platformKey(
	platform: NodeJS.Platform = process.platform,
	arch: string = process.arch
): string {
	return `${platform}-${arch}`
}

export function binaryName(platform: NodeJS.Platform = process.platform): string {
	return platform === 'win32' ? 'agent-toolkit.exe' : 'agent-toolkit'
}
