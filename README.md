```text
 █████╗  ██████╗ ███████╗███╗   ██╗████████╗
██╔══██╗██╔════╝ ██╔════╝████╗  ██║╚══██╔══╝
███████║██║  ███╗█████╗  ██╔██╗ ██║   ██║
██╔══██║██║   ██║██╔══╝  ██║╚██╗██║   ██║
██║  ██║╚██████╔╝███████╗██║ ╚████║   ██║
╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═══╝   ╚═╝
████████╗ ██████╗  ██████╗ ██╗     ██╗  ██╗██╗████████╗
╚══██╔══╝██╔═══██╗██╔═══██╗██║     ██║ ██╔╝██║╚══██╔══╝
   ██║   ██║   ██║██║   ██║██║     █████╔╝ ██║   ██║
   ██║   ██║   ██║██║   ██║██║     ██╔═██╗ ██║   ██║
   ██║   ╚██████╔╝╚██████╔╝███████╗██║  ██╗██║   ██║
   ╚═╝    ╚═════╝  ╚═════╝ ╚══════╝╚═╝  ╚═╝╚═╝   ╚═╝
```

**Curated plugins for coding agents and developer tools**

[![Agents](https://img.shields.io/badge/Agents-AGENTS.md-111827?style=flat-square)](https://github.com/amtiYo/agents) [![Tools](https://img.shields.io/badge/Tools-Multi_Agent-0F766E?style=flat-square)](README.md#tool-install) [![Plugins](https://img.shields.io/badge/Plugins-3-22C55E?style=flat-square)](https://github.com/harryy2510/agent-toolkit) [![License](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE)

---

## What Is This?

Agent Toolkit is a **fast open-source setup, repo intelligence, and enforcement CLI** for coding agents and developer tools, plus plugin marketplace metadata and an agent catalog.

- `@harryy/agent-toolkit` is the Bun package users run with `bunx`.
- The hot path is a Rust native binary for fast repo scanning and checks.
- Supported surfaces include Claude Code, Codex, Gemini CLI, Cursor, VS Code Copilot, Windsurf, Antigravity, OpenCode, Junie, and AGENTS.md-aware tools.
- Each plugin repo can carry shared `AGENTS.md` instructions, `.agents/agents.json` integration settings, native extension metadata, and generated repo intelligence.

```
+------------------------------------------------------+
|                  agent-toolkit                       |
|                  (marketplace)                        |
|                                                       |
|   +-------------------+   +----------------------+   |
|   |    dotagent        |   |    vibe-pilot        |   |
|   |    21 skills       |   |    4 skills          |   |
|   |    20 agents       |   |    kanban autopilot  |   |
|   +-------------------+   +----------------------+   |
|   +-------------------+                              |
|   |    web-smith      |                              |
|   |    2 skills       |                              |
|   |    site cloning   |                              |
|   +-------------------+                              |
+------------------------------------------------------+
```

---

## Quick Start

For shared teams, make the repo carry the agent contract:

```bash
bunx @harryy/agent-toolkit repo migrate
bunx @harryy/agent-toolkit repo dotagent
agents sync --path .
```

`repo dotagent` pins the DotAgent rules into root `AGENTS.md`, vendors DotAgent skills into `.agents/skills/`, copies role-profile and command references under `.agents/dotagent/`, and writes `.agents/dotagent.lock.json`. Commit those repo files so new contributors get the same rules and skills from `git clone`, then run `agents sync --path .` to materialize local tool bridges. Later runs use the lock's DotAgent version and git revision as the normal no-op gate; use `repo dotagent --force` only when you need to recopy the same snapshot.

Install or preview the personal global setup when you want the same defaults outside an agentized repo:

```bash
bunx @harryy/agent-toolkit setup --dry-run
bunx @harryy/agent-toolkit setup --yes
bunx @harryy/agent-toolkit update
```

End users need Bun only. Rust is not required for normal use because the published package ships prebuilt native binaries.

`setup` installs the shared rules plugin, preserves existing user rules, and inserts managed Agent Toolkit blocks where supported. This is a personal convenience layer, not the team source of truth. `update` pulls the latest DotAgent source and reapplies the packaged global rules without an extra prompt:

- Codex: `~/.codex/AGENTS.md`
- Claude Code: `~/.claude/CLAUDE.md`
- Gemini CLI: links the Gemini extension when `gemini` is installed

Useful setup flags:

```bash
bunx @harryy/agent-toolkit setup --dry-run
bunx @harryy/agent-toolkit setup --yes
bunx @harryy/agent-toolkit update
bunx @harryy/agent-toolkit setup --all --yes
bunx @harryy/agent-toolkit setup --skip-gemini --yes
```

Repo setup:

```bash
bunx @harryy/agent-toolkit repo migrate
bunx @harryy/agent-toolkit repo dotagent
bunx @harryy/agent-toolkit repo check
```

Fleet setup:

```bash
bunx @harryy/agent-toolkit fleet scan ~/Desktop ~/Code
bunx @harryy/agent-toolkit fleet migrate ~/Desktop ~/Code
```

Run `agents sync` explicitly when you want generated local tool config:

```bash
bunx @harryy/agent-toolkit repo sync --check
bunx @harryy/agent-toolkit fleet sync --check ~/Desktop ~/Code
```

---

## Tool Install

Agent Toolkit uses shared repo instructions first, then tool-specific install surfaces where a tool needs them.

### Claude Code

Use the native plugin marketplace when installing into Claude Code.

```bash
claude plugin marketplace add harryy2510/agent-toolkit
claude plugin install dotagent@agent-toolkit
claude plugin install vibe-pilot@agent-toolkit
claude plugin install web-smith@agent-toolkit
```

Update:

```bash
claude plugin marketplace update agent-toolkit
claude plugin update dotagent@agent-toolkit
claude plugin update vibe-pilot@agent-toolkit
claude plugin update web-smith@agent-toolkit
```

### Gemini CLI

Gemini uses local extensions. Global setup links the extension automatically when the `gemini` CLI is detected:

```bash
bunx @harryy/agent-toolkit setup --yes
```

For plugin development or a manual install, link the plugin package:

```bash
gemini extensions link /path/to/dotagent/plugins/dotagent/gemini-extension --consent
gemini extensions link /path/to/vibe-pilot/plugins/vibe-pilot --consent
gemini extensions link /path/to/web-smith/plugins/web-smith --consent
```

Each Gemini package contains `gemini-extension.json` plus a `GEMINI.md` context file.

### Codex

Codex reads repo instructions from `AGENTS.md`. For teams, prefer the repo-scoped DotAgent path so rules and skills are committed with the project. Global setup can still install managed shared rules into `~/.codex/AGENTS.md` for personal defaults.

```bash
bunx @harryy/agent-toolkit repo migrate
bunx @harryy/agent-toolkit repo dotagent
bunx @harryy/agent-toolkit repo sync
```

Codex plugin metadata lives beside each plugin:

```text
dotagent/plugins/dotagent/.codex-plugin/plugin.json
vibe-pilot/plugins/vibe-pilot/.codex-plugin/plugin.json
web-smith/plugins/web-smith/.codex-plugin/plugin.json
```

Each plugin repo also includes `.agents/plugins/marketplace.json` for Codex-compatible local plugin catalogs.

### Other Agents

| Agent/tool             | Support path                                                     |
| ---------------------- | ---------------------------------------------------------------- |
| AGENTS.md-aware agents | Read committed `AGENTS.md` directly.                             |
| Cursor                 | Run `agents sync --path .` to generate local Cursor config.      |
| VS Code Copilot        | Run `agents sync --path .` to generate local VS Code MCP config. |
| Windsurf               | Run `agents sync --path .`; user-level MCP sync stays local.     |
| Antigravity            | Run `agents sync --path .`; global sync is disabled by default.  |
| OpenCode               | Run `agents sync --path .` to generate local `opencode` config.  |
| Junie                  | Run `agents sync --path .` to generate local Junie MCP config.   |

---

## Repo Intelligence And Enforcement

Agent Toolkit builds repo intelligence, repo maps, and quality enforcement from one command surface.

```bash
bunx @harryy/agent-toolkit repo intel
```

This writes repo intelligence files:

```text
.agents/intel/summary.md
.agents/intel/overview.md
.agents/intel/tasks.md
.agents/intel/tooling.md
.agents/intel/routes.md
.agents/intel/api.md
.agents/intel/components.md
.agents/intel/data.md
.agents/intel/database.md
.agents/intel/graph.md
.agents/intel/impact.md
.agents/intel/boundaries.md
.agents/intel/imports.md
.agents/intel/calls.md
.agents/intel/dependencies.md
.agents/intel/symbols.md
.agents/intel/files.md
.agents/intel/env.md
.agents/intel/testing.md
.agents/intel/repo.json
```

The generated wiki gives agents a deterministic repo map with task read paths, OXC-backed JS/TS AST extraction, pg_query-backed SQL parsing, framework signals, route/API/component/data surfaces, ordered static database reduction, runtime boundaries, env usage, import graph hotspots, change-impact plans, local import adjacency, call-site indexes, external dependency usage, exported symbols, full file inventory, package scripts, and follow-up guidance for deeper exploration.

`repo migrate` also adds an idempotent managed block to root `AGENTS.md` that tells agents to start at `.agents/intel/summary.md` before broad exploration. Existing repo instructions are preserved, and `.agents/intel/` is generated repo intelligence that may be committed in migrated repos.

```bash
bunx @harryy/agent-toolkit repo check
```

The check command enforces:

- `AGENTS.md` and `.agents/agents.json` exist.
- `AGENTS.md` points agents to `.agents/intel/summary.md` before broad exploration.
- `scripts/agent-check` and Husky hooks in `.husky/` exist for local and CI enforcement.
- New JavaScript-platform source uses TypeScript, not `.js` or `.jsx`.
- `oxlint --type-aware --type-check` replaces `tsc` checks.
- `oxlint` replaces ESLint.
- `oxfmt` replaces Prettier.
- Deslop-style checks catch debug statements, placeholders, empty catches, and likely hardcoded secrets.
- Local/private agent state such as `.agents/local.json` and `.agents/generated/` is blocked when tracked by git; migrated repo intel and tool adapters such as `.agents/intel/`, `CLAUDE.md`, `.codex/`, `.gemini/`, and `.cursor/` are allowed to be committed.
- Bootstrapped Husky hooks enforce Conventional Commit messages.
- In Supabase projects, `repo check --staged` runs database lint for staged Supabase SQL/config changes, preferring a project script such as `db:lint` or `lint:db` before falling back to `supabase db lint --local --schema public --level warning --fail-on warning`. Add a project script when a repo needs to lint extra app schemas.

Use the migration commands to make this standard repeatable:

```bash
agent-toolkit repo migrate
agent-toolkit fleet bootstrap ~/Desktop ~/Code
agent-toolkit fleet migrate ~/Desktop ~/Code
```

Use sync commands when `@agents-dev/cli` is installed:

```bash
agent-toolkit repo sync --check
agent-toolkit fleet sync --check ~/Desktop ~/Code
```

---

## Architecture

```text
packages:
  @harryy/agent-toolkit
    bin/agent-toolkit.ts    Bun TypeScript wrapper
    src/*.ts                package runtime glue

crates:
  agent-toolkit-core        Rust library for scans, checks, hooks, setup
  agent-toolkit-cli         Rust native binary
```

The Bun wrapper runs the native binary when present and falls back to `cargo run` during local development.

Published packages include prebuilt binaries at:

```text
bin/native/darwin-arm64/agent-toolkit
bin/native/darwin-x64/agent-toolkit
bin/native/linux-x64/agent-toolkit
bin/native/linux-arm64/agent-toolkit
```

Rust is only required for developing Agent Toolkit itself or building release artifacts.

## Development

```bash
bun test
cargo test
bun run build:native
bun bin/agent-toolkit.ts repo check
```

Do not use JavaScript source files, `tsc`, ESLint, or Prettier in this repo. Use TypeScript with Bun, `oxlint --type-aware --type-check`, and `oxfmt`.

---

## Plugin Catalog

| Plugin         | Description                                                  | What You Get                      | Plugin ID                  |
| -------------- | ------------------------------------------------------------ | --------------------------------- | -------------------------- |
| **dotagent**   | Code conventions, agents, and tooling for all projects       | 21 skills, 20 agents, 6 commands  | `dotagent@agent-toolkit`   |
| **vibe-pilot** | AI kanban autopilot for classify, triage, and status reports | 4 skills, 3 commands              | `vibe-pilot@agent-toolkit` |
| **web-smith**  | Website cloning and static site optimization                 | 2 skills, headless audit workflow | `web-smith@agent-toolkit`  |

---

## dotagent

> Code conventions, 20 specialist agents, 21 skills, and tooling for all projects.

```
+------------------------------------------------------------------+
|  dotagent                                                         |
|  "Load a skill, not your whole brain."                           |
+------------------------------------------------------------------+
|                                                                    |
|  SKILLS (21)              AGENTS (20)           COMMANDS (6)      |
|  ----------------         ----------------      ----------------  |
|  ui                       engineer              /setup            |
|  forms-rhf-zod            tester                /update           |
|  react-query-mutative     product               /uninstall        |
|  zustand-x-ui-state       orchestrator          /repo-map         |
|  tanstack-start-cf        reviewer              /skill-lint       |
|  supabase-auth-data       debugger              /deslop           |
|  cloudflare               security              |                 |
|  project-setup            performance           |                 |
|  scaffold                 ...and 12 more        |                 |
|  toolchain                                     |                 |
|  repo-intelligence                             |                 |
|  agent-routing                                 |                 |
|  react-best-practices                           |                 |
|  conventions                                    |                 |
|  shadcn                                         |                 |
|  supabase-postgres                              |                 |
|  vite                                           |                 |
|  repo-map                                       |                 |
|  deslop                                         |                 |
|  testing                                        |                 |
|  debugging                                      |                 |
|                                                                    |
+------------------------------------------------------------------+
```

### Skills at a Glance

| Category            | Skills                                                                                     | What They Cover                                                                                       |
| ------------------- | ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| **UI & Components** | `ui`, `shadcn`, `forms-rhf-zod`                                                            | Tailwind v4, shadcn/base-ui, CVA, dark mode, react-hook-form + zod                                    |
| **State & Data**    | `zustand-x-ui-state`, `react-query-mutative`                                               | zustand-x v6, React Query, optimistic updates, key factories                                          |
| **Frameworks**      | `tanstack-start-cloudflare`, `vite`, `cloudflare`                                          | Routing, server functions, SSR, Workers, Wrangler config                                              |
| **Backend**         | `supabase-auth-data`, `supabase-postgres-best-practices`                                   | 3 Supabase clients, auth flow, RLS, migrations, query optimization                                    |
| **Quality**         | `react-best-practices`, `conventions`, `deslop`, `testing`, `debugging`                    | 57 perf rules, oxlint/oxfmt enforcement, AI slop detection, Vitest + Playwright, root-cause workflows |
| **Tooling**         | `project-setup`, `scaffold`, `toolchain`, `repo-intelligence`, `agent-routing`, `repo-map` | DX tooling, fullstack scaffolding, codebase symbol indexing, agent routing                            |

### Agents at a Glance

20 specialist agents covering engineering, testing, product management, orchestration, code review, debugging, security analysis, performance optimization, and more. Each agent has a focused role and knows when to defer to another.

Full documentation: [github.com/harryy2510/dotagent](https://github.com/harryy2510/dotagent)

---

## vibe-pilot

> Zero-touch kanban autopilot. Discovers repos, classifies tasks, triages complex work, picks what to build, and launches AI workspaces.

```
+------------------------------------------------------------------+
|  vibe-pilot                                                       |
|  "Set it and forget it."                                         |
+------------------------------------------------------------------+
|                                                                    |
|  SKILLS (4)               COMMANDS (3)                            |
|  ----------------         ----------------                        |
|  classify                 /classify                               |
|  triage                   /triage                                 |
|  implement                /status-report                          |
|  status-report                                                    |
|                                                                    |
+------------------------------------------------------------------+
|                                                                    |
|  HOW IT WORKS                                                     |
|  ---------------------------------------------------------------- |
|                                                                    |
|    Discover repos  -->  Classify tasks  -->  Triage complex work  |
|         |                                          |               |
|         v                                          v               |
|    Pick next task  <--  Launch workspace  <--  Break into steps   |
|         |                                          |               |
|         v                                          v               |
|    Implement  -------->  Commit & update  ---->  Next cycle       |
|                                                                    |
+------------------------------------------------------------------+
```

### Skills

| Skill             | What It Does                                                                |
| ----------------- | --------------------------------------------------------------------------- |
| **classify**      | Scans repos and classifies kanban tasks by complexity, domain, and priority |
| **triage**        | Breaks complex tasks into actionable subtasks with dependencies             |
| **implement**     | Picks the next task, launches an AI workspace, and builds it                |
| **status-report** | Generates progress reports across all tracked projects                      |

Full documentation: [github.com/harryy2510/vibe-pilot](https://github.com/harryy2510/vibe-pilot)

---

## web-smith

> Clone and optimize websites into Lighthouse-focused Astro static builds.

### Skills

| Skill              | What It Does                                                                                                  |
| ------------------ | ------------------------------------------------------------------------------------------------------------- |
| **website-cloner** | Scrapes a source site, extracts design tokens, scaffolds Astro, rebuilds pages, and hands off to optimization |
| **site-optimizer** | Audits and fixes performance, accessibility, SEO, images, fonts, and static-site delivery                     |

Full documentation: [github.com/harryy2510/web-smith](https://github.com/harryy2510/web-smith)

---

## Agent Sync

Each plugin repo includes `AGENTS.md` and `.agents/agents.json` for the `agents` CLI:

```bash
agents sync --path .
agents watch --path .
```

Team repos can also carry DotAgent as a pinned project dependency:

```bash
bunx @harryy/agent-toolkit repo dotagent
```

This commits the DotAgent instruction block in `AGENTS.md`, copies DotAgent skills into `.agents/skills/`, keeps role-profile and command references under `.agents/dotagent/`, and records the snapshot in `.agents/dotagent.lock.json`. Re-running the command is a no-op while the locked DotAgent version and revision match the source. Generated tool files stay local and are ignored by git unless the repo intentionally uses commit-generated sync output.

For repos bootstrapped by Agent Toolkit, `scripts/agent-check` runs `agent-toolkit repo check`. Set `AGENT_TOOLKIT_SYNC_CHECK=1` when you also want hooks or CI to run `agents sync --path . --check`. Husky hooks call the same wrapper, so contributors do not need a global Agent Toolkit install.

Gemini CLI native extension wrappers live in plugin packages as `gemini-extension/` where needed.

---

## Author

**Hariom Sharma**, [github.com/harryy2510](https://github.com/harryy2510)

---

## License

MIT
