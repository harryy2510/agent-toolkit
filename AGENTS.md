# Agent Toolkit Repository Instructions

## Purpose

This repository is the Agent Toolkit package and marketplace index for Hariom Sharma's agent plugins. It contains the open-source `@harryy/agent-toolkit` CLI, the Rust native core, the Bun TypeScript wrapper, marketplace metadata, and catalog documentation. Plugin implementation lives in the individual plugin repositories.

## Agent Sync

- `AGENTS.md` is the canonical instruction file for coding agents.
- `.agents/agents.json` is the source of truth for `agents` CLI integration settings.
- Run `agents sync --path .` after changing `.agents/agents.json` when you want to materialize local tool config.
- Commit generated tool adapter files such as root `CLAUDE.md`, `.codex/`, `.claude/`, `.cursor/`, `.gemini/`, `.windsurf/`, `.opencode/`, and similar when produced for migrated repos.
- Commit `.agents/intel/` when produced for migrated repos so repo intelligence can travel with the project.
- Do not commit `.agents/local.json` or `.agents/generated/`; they are local/private generated state.

## Toolchain

- Use TypeScript for JavaScript-platform code. Do not create `.js` or `.jsx` source files.
- Execute TypeScript with Bun.
- Use Rust for the native performance core.
- Use `oxlint --type-aware --type-check`, not `tsc`.
- Use `oxlint`, not ESLint.
- Use `oxfmt`, not Prettier.
- Use the repo's active committed hook manager for git hooks: Vite+ projects use `.vite-hooks`, other repos default to Husky `.husky`. Do not introduce `.githooks` or ad hoc hook folders.
- Any commits must use Conventional Commit format.

## Marketplace Rules

- Keep `.claude-plugin/marketplace.json` in sync with README install commands.
- Marketplace name is `agent-toolkit`. Use `agent-toolkit` in install and update examples.
- Plugin sources should point at each plugin repo via `git-subdir` and the `plugins/<name>` path.
- Add a plugin entry only after the matching plugin repo has a valid plugin manifest.

## Safety

- Do not add Superpowers docs or planning docs to this public repo.
- Do not run `git push` unless the user explicitly asks in the current message.
- Keep changes scoped to the requested package, marketplace, or documentation surface.

<!-- AGENT-TOOLKIT:REPO-INTEL:START -->
## Agent Toolkit Repo Intelligence

- Before broad exploration, read `.agents/intel/summary.md` if it exists.
- Use the task-specific intel files it links to (`overview.md`, `tasks.md`, `graph.md`, `database.md`, and similar) to find the relevant source files before editing.
- `.agents/intel/` is generated repo intelligence and may be committed in migrated repos.
<!-- AGENT-TOOLKIT:REPO-INTEL:END -->
