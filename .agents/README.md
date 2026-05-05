# .agents

This repository uses `@agents-dev/cli` as the shared source of truth for agent instructions.

Commit these files:

- `../AGENTS.md`
- `agents.json`
- `dotagent.lock.json` when DotAgent is installed for the repo
- `dotagent/` role-profile and command references when DotAgent is installed for the repo
- `skills/` when project or DotAgent skills are committed
- `intel/`
- root `../CLAUDE.md`
- tool adapter folders such as `../.codex/`, `../.claude/`, `../.cursor/`, `../.gemini/`, `../.windsurf/`, `../.opencode/`, and similar generated integration output

Do not commit local or private generated outputs:

- `local.json`
- `generated/`

Useful commands:

```bash
agent-toolkit repo migrate
agent-toolkit repo dotagent
agent-toolkit repo check
agent-toolkit repo sync --check
agents status --path .
agents sync --path .
agents sync --path . --check
agents watch --path .
```

Note: Windsurf writes user-level MCP config when synced. Antigravity is enabled for generated snapshots, but global sync is disabled by default.
