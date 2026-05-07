# Overview

## Stack Signals

- No framework signals detected from package metadata or file layout.

## Scale

- Source-like files: 28
- UI components: 0
- Routes: 1
- API endpoints/modules: 1
- SQL objects: 0
- Tests: 2

## Source Areas

- `crates/agent-toolkit-core`: 10
- `.`: 6
- `.github/workflows`: 2
- `crates/agent-toolkit-cli`: 2
- `bin`: 1
- `scripts/build-native.ts`: 1
- `scripts/bump-version.ts`: 1
- `src/cli.ts`: 1
- `src/native.ts`: 1
- `src/platform.ts`: 1
- `test/bump-version.test.ts`: 1
- `test/native.test.ts`: 1

## High-Impact Files

- `src/platform.ts` — imported by 3 local files
- `src/native.ts` — imported by 2 local files
- `src/cli.ts` — imported by 1 local files

## Largest Non-Generated Files

- `crates/agent-toolkit-core/src/intel.rs` — 5558 lines
- `crates/agent-toolkit-core/src/global_setup.rs` — 1129 lines
- `crates/agent-toolkit-core/src/check.rs` — 909 lines
- `crates/agent-toolkit-cli/src/main.rs` — 871 lines
- `crates/agent-toolkit-core/src/hooks.rs` — 630 lines
- `crates/agent-toolkit-core/src/dotagent.rs` — 623 lines
- `README.md` — 461 lines
- `.github/workflows/release.yml` — 170 lines
- `crates/agent-toolkit-core/src/migrate.rs` — 144 lines
- `crates/agent-toolkit-core/src/supabase.rs` — 127 lines
- `test/native.test.ts` — 115 lines
- `src/native.ts` — 114 lines
- `crates/agent-toolkit-core/src/fleet.rs` — 90 lines
- `scripts/bump-version.ts` — 76 lines
- `test/bump-version.test.ts` — 57 lines
- `AGENTS.md` — 46 lines
- `package.json` — 36 lines
- `scripts/build-native.ts` — 30 lines
- `.github/workflows/agent-check.yml` — 29 lines
- `tsconfig.json` — 29 lines

## Exported Symbols Sample

- `crates/agent-toolkit-core/src/check.rs` — IssueCode, RepoIssue, check_repo, is_conventional_commit
- `crates/agent-toolkit-core/src/dotagent.rs` — RepoDotAgentOptions, RepoDotAgentResult, RepoDotAgentSkippedSkill, install_repo_dotagent, install_repo_dotagent_with_options
- `crates/agent-toolkit-core/src/fleet.rs` — discover_git_repos
- `crates/agent-toolkit-core/src/global_setup.rs` — AgentDetection, GlobalSetupAction, GlobalSetupActionKind, GlobalSetupExtensionSkip, GlobalSetupOptions, GlobalSetupPlan, GlobalSetupResult, GlobalSetupSkip, GlobalTeardownOptions, GlobalTeardownResult, apply_global_setup_plan, build_global_setup_plan, default_global_setup_options, detect_installed_agents, install_global_rules, teardown_global_rules
- `crates/agent-toolkit-core/src/hooks.rs` — BootstrapChange, BootstrapChangeKind, bootstrap_repo, commit_msg_hook, verb
- `crates/agent-toolkit-core/src/intel.rs` — RepoIntel, build_repo_intel, write_repo_intel
- `crates/agent-toolkit-core/src/lib.rs` — check, dotagent, fleet, global_setup, hooks, intel, migrate, supabase
- `crates/agent-toolkit-core/src/migrate.rs` — RepoMigrationResult, RepoSetupOptions, RepoSetupResult, migrate_repo, setup_repo
- `crates/agent-toolkit-core/src/supabase.rs` — db_lint_script, staged_db_lint_needed
- `src/cli.ts` — runAgentToolkit
- `src/native.ts` — NativeCommandResolution, NativeRunOptions, findNativeBinary, nativeBinaryCandidates, nativePlatformKey, resolveNativeCommand, runNative
- `src/platform.ts` — binaryName, platformKey
