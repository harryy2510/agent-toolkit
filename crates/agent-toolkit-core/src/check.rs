use std::fs;
use std::path::Path;
use std::process::Command;

use crate::hooks::{detect_hook_layout, hook_source_path, HookLayout, DEFAULT_INTEGRATIONS};

#[derive(Debug, PartialEq, Eq)]
pub enum IssueCode {
    MissingAgentsMd,
    MissingRepoIntelInstructions,
    MissingAgentsConfig,
    MissingAgentsIntegration,
    MissingAgentCheckScript,
    MissingGitHook,
    UnsafeGitHook,
    MissingHookManagerInstructions,
    DisallowedTrackedFile,
    JavaScriptSource,
    TscUsage,
    EslintUsage,
    PrettierUsage,
    InvalidCommitMessage,
    SlopPattern,
    HardcodedSecret,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RepoIssue {
    pub code: IssueCode,
    pub message: String,
}

pub fn check_repo(root: &Path) -> Vec<RepoIssue> {
    let mut issues = Vec::new();

    let agents_path = root.join("AGENTS.md");
    let agents_contents = match fs::read_to_string(&agents_path) {
        Ok(contents) => {
            if !contents.contains(".agents/intel/summary.md") {
                issues.push(RepoIssue {
                    code: IssueCode::MissingRepoIntelInstructions,
                    message: "AGENTS.md must tell agents to read .agents/intel/summary.md before broad exploration".to_string(),
                });
            }
            Some(contents)
        }
        Err(_) => {
            issues.push(RepoIssue {
                code: IssueCode::MissingAgentsMd,
                message: "AGENTS.md is required as the canonical repo instruction file".to_string(),
            });
            None
        }
    };

    match fs::read_to_string(root.join(".agents/agents.json")) {
        Ok(contents) => push_agents_config_issues(&mut issues, &contents),
        Err(_) => {
            issues.push(RepoIssue {
                code: IssueCode::MissingAgentsConfig,
                message: ".agents/agents.json is required for cross-agent sync".to_string(),
            });
        }
    }
    push_missing_file_issue(
		&mut issues,
		root,
		"scripts/agent-check",
		IssueCode::MissingAgentCheckScript,
		"scripts/agent-check is required so hooks and CI can run repo-local checks without global installs",
	);
    let hook_layout = detect_hook_layout(root);
    if let Some(contents) = agents_contents.as_deref() {
        push_hook_manager_instruction_issue(&mut issues, hook_layout, contents);
    }
    push_missing_hook_issue(
        &mut issues,
        root,
        hook_layout,
        "pre-commit",
        "scripts/agent-check --staged",
        "pre-commit hook must run scripts/agent-check --staged",
    );
    push_missing_hook_issue(
        &mut issues,
        root,
        hook_layout,
        "pre-push",
        "scripts/agent-check",
        "pre-push hook must run scripts/agent-check",
    );
    push_missing_hook_issue(
        &mut issues,
        root,
        hook_layout,
        "commit-msg",
        "Conventional Commit",
        "commit-msg hook must enforce Conventional Commit messages",
    );
    push_unsafe_post_checkout_issue(&mut issues, root, hook_layout);

    push_disallowed_tracked_file_issues(root, &mut issues);

    let package_json = root.join("package.json");
    if let Ok(contents) = fs::read_to_string(package_json) {
        push_tooling_issue(
            &mut issues,
            &contents,
            "tsc",
            IssueCode::TscUsage,
            "use oxlint --type-aware --type-check instead of tsc",
        );
        push_tooling_issue(
            &mut issues,
            &contents,
            "eslint",
            IssueCode::EslintUsage,
            "use oxlint instead of ESLint",
        );
        push_tooling_issue(
            &mut issues,
            &contents,
            "prettier",
            IssueCode::PrettierUsage,
            "use oxfmt instead of Prettier",
        );
    }

    if contains_javascript_source(root) {
        issues.push(RepoIssue {
            code: IssueCode::JavaScriptSource,
            message: "new source code must be TypeScript, not JavaScript".to_string(),
        });
    }

    issues.extend(scan_slop(root));

    issues
}

fn push_agents_config_issues(issues: &mut Vec<RepoIssue>, contents: &str) {
    let Ok(config) = serde_json::from_str::<serde_json::Value>(contents) else {
        issues.push(RepoIssue {
            code: IssueCode::MissingAgentsConfig,
            message: ".agents/agents.json must be valid JSON".to_string(),
        });
        return;
    };
    let enabled = config
        .get("integrations")
        .and_then(|integrations| integrations.get("enabled"))
        .and_then(|enabled| enabled.as_array());
    let Some(enabled) = enabled else {
        issues.push(RepoIssue {
            code: IssueCode::MissingAgentsIntegration,
            message: ".agents/agents.json must list enabled integrations so agents sync can write tool config".to_string(),
        });
        return;
    };
    let missing: Vec<&str> = DEFAULT_INTEGRATIONS
        .into_iter()
        .filter(|integration| {
            !enabled
                .iter()
                .any(|entry| entry.as_str() == Some(*integration))
        })
        .collect();
    if !missing.is_empty() {
        issues.push(RepoIssue {
            code: IssueCode::MissingAgentsIntegration,
            message: format!(
                ".agents/agents.json integrations.enabled is missing: {}",
                missing.join(", ")
            ),
        });
    }
}

pub fn is_conventional_commit(message: &str) -> bool {
    let first_line = message.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return false;
    }

    let Some((prefix, description)) = first_line.split_once(": ") else {
        return false;
    };
    if description.trim().is_empty() {
        return false;
    }

    let prefix = prefix.strip_suffix('!').unwrap_or(prefix);
    let (commit_type, scope) = if let Some(open_index) = prefix.find('(') {
        if !prefix.ends_with(')') || open_index == 0 {
            return false;
        }
        (
            &prefix[..open_index],
            Some(&prefix[open_index + 1..prefix.len() - 1]),
        )
    } else {
        (prefix, None)
    };

    if !is_slug(commit_type) {
        return false;
    }

    if let Some(scope) = scope {
        if scope.is_empty() || !is_scope_list(scope) {
            return false;
        }
    }

    true
}

fn push_missing_file_issue(
    issues: &mut Vec<RepoIssue>,
    root: &Path,
    relative_path: &str,
    code: IssueCode,
    message: &str,
) {
    if !root.join(relative_path).exists() {
        issues.push(RepoIssue {
            code,
            message: format!("{relative_path}: {message}"),
        });
    }
}

fn push_hook_manager_instruction_issue(
    issues: &mut Vec<RepoIssue>,
    layout: HookLayout,
    agents_contents: &str,
) {
    if layout.source_dir != ".vite-hooks" {
        return;
    }
    if !agents_contents.contains("Use Husky for git hooks") {
        return;
    }
    if agents_contents.contains("This repo uses Vite+ hooks")
        && agents_contents.contains("overrides generic DotAgent hook guidance")
    {
        return;
    }

    issues.push(RepoIssue {
        code: IssueCode::MissingHookManagerInstructions,
        message: "AGENTS.md must declare Vite+ .vite-hooks as the active hook manager when DotAgent's generic Husky guidance is present".to_string(),
    });
}

fn push_missing_hook_issue(
    issues: &mut Vec<RepoIssue>,
    root: &Path,
    layout: HookLayout,
    hook_name: &str,
    required_needle: &str,
    message: &str,
) {
    let relative_path = hook_source_path(layout, hook_name);
    let path = root.join(&relative_path);
    let Ok(contents) = fs::read_to_string(&path) else {
        issues.push(RepoIssue {
            code: IssueCode::MissingGitHook,
            message: format!(
                "{relative_path}: repo {} hooks must be committed",
                layout.label
            ),
        });
        return;
    };

    if !contents.contains(required_needle) {
        issues.push(RepoIssue {
            code: IssueCode::MissingGitHook,
            message: format!("{relative_path}: repo {} {message}", layout.label),
        });
    }
}

fn push_unsafe_post_checkout_issue(issues: &mut Vec<RepoIssue>, root: &Path, layout: HookLayout) {
    let relative_path = hook_source_path(layout, "post-checkout");
    let path = root.join(&relative_path);
    let Ok(contents) = fs::read_to_string(&path) else {
        return;
    };

    if contents.contains("bun install") {
        issues.push(RepoIssue {
            code: IssueCode::UnsafeGitHook,
            message: format!(
                "{relative_path}: post-checkout hooks must not run bun install; install dependencies explicitly after checkout"
            ),
        });
    }
}

fn push_tooling_issue(
    issues: &mut Vec<RepoIssue>,
    contents: &str,
    needle: &str,
    code: IssueCode,
    message: &str,
) {
    if contents.contains(needle) {
        issues.push(RepoIssue {
            code,
            message: message.to_string(),
        });
    }
}

fn contains_javascript_source(root: &Path) -> bool {
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if entry_path.is_dir() {
                if should_skip_dir(&name) || name == "public" {
                    continue;
                }
                stack.push(entry_path);
                continue;
            }

            if matches!(
                entry_path
                    .extension()
                    .and_then(|extension| extension.to_str()),
                Some("js" | "jsx")
            ) {
                return true;
            }
        }
    }
    false
}

fn scan_slop(root: &Path) -> Vec<RepoIssue> {
    let mut issues = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if entry_path.is_dir() {
                if should_skip_dir(&name) {
                    continue;
                }
                stack.push(entry_path);
                continue;
            }
            let display_path = entry_path.strip_prefix(root).unwrap_or(&entry_path);
            if !is_text_source(&entry_path) || should_skip_slop_file(display_path) {
                continue;
            }
            let Ok(contents) = fs::read_to_string(&entry_path) else {
                continue;
            };
            let display_path = display_path.display();
            if has_secret_pattern(&contents) {
                issues.push(RepoIssue {
                    code: IssueCode::HardcodedSecret,
                    message: format!("{display_path} appears to contain a hardcoded secret"),
                });
            }
            if has_slop_pattern(&contents) {
                issues.push(RepoIssue {
                    code: IssueCode::SlopPattern,
                    message: format!(
                        "{display_path} contains debug, placeholder, or empty error handling code"
                    ),
                });
            }
        }
    }
    issues
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | ".turbo"
            | ".cache"
            | ".wrangler"
            | ".agents"
            | ".claude"
            | ".codex"
            | ".gemini"
            | ".cursor"
            | ".antigravity"
            | ".windsurf"
            | ".opencode"
            | ".junie"
    )
}

fn is_text_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("ts" | "tsx" | "rs" | "go" | "py" | "sql" | "sh")
    )
}

fn should_skip_slop_file(relative_path: &Path) -> bool {
    let path = relative_path.to_string_lossy();
    path.contains("/skills/deslop/")
        || path.ends_with("/deslop.sh")
        || path.ends_with(".d.ts")
        || path == "plugins/dotagent/scripts/deslop.sh"
}

fn has_secret_pattern(contents: &str) -> bool {
    contains_prefixed_secret(contents, concat!("sk", "_live_"), 20)
        || contains_prefixed_secret(contents, concat!("AK", "IA"), 16)
        || contains_prefixed_secret(contents, concat!("gh", "p_"), 20)
        || contains_prefixed_secret(contents, concat!("xo", "xb-"), 12)
        || contains_env_secret_assignment(contents, concat!("SUPABASE", "_SERVICE_ROLE_KEY"))
}

fn contains_prefixed_secret(contents: &str, prefix: &str, minimum_suffix_len: usize) -> bool {
    let mut offset = 0usize;
    while let Some(index) = contents[offset..].find(prefix) {
        let start = offset + index + prefix.len();
        let suffix_len = contents[start..]
            .chars()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
            })
            .count();
        if suffix_len >= minimum_suffix_len {
            return true;
        }
        offset = start;
    }
    false
}

fn contains_env_secret_assignment(contents: &str, name: &str) -> bool {
    contents.lines().any(|line| {
        let line = line.trim();
        let Some(value) = line
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
        else {
            return false;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        value.len() >= 20
            && !matches!(
                value,
                "" | "changeme" | "change_me" | "replace_me" | "your_key_here"
            )
    })
}

fn has_slop_pattern(contents: &str) -> bool {
    [
        concat!("console", ".debug("),
        concat!("debugger", ";"),
        concat!("throw new Error(\"", "TODO"),
        concat!("throw new Error('", "TODO"),
        concat!("lorem", " ipsum"),
    ]
    .iter()
    .any(|pattern| contents.contains(pattern))
}

fn push_disallowed_tracked_file_issues(root: &Path, issues: &mut Vec<RepoIssue>) {
    for file in tracked_files(root) {
        if !root.join(&file).exists() {
            continue;
        }
        if is_disallowed_tracked_path(&file) {
            issues.push(RepoIssue {
                code: IssueCode::DisallowedTrackedFile,
                message: format!("{file} must stay local/private and should not be tracked"),
            });
        }
    }
}

fn tracked_files(root: &Path) -> Vec<String> {
    let Ok(output) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter_map(|path| std::str::from_utf8(path).ok())
        .filter(|path| !path.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn is_disallowed_tracked_path(path: &str) -> bool {
    path == ".agents/local.json" || path.starts_with(".agents/generated/")
}

fn is_slug(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn is_scope(value: &str) -> bool {
    value.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '-' | '_' | '.' | '/')
    })
}

fn is_scope_list(value: &str) -> bool {
    value
        .split(',')
        .all(|scope| !scope.is_empty() && is_scope(scope))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "agent-toolkit-check-{}-{}",
            std::process::id(),
            TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn conventional_commit_accepts_common_valid_messages() {
        assert!(is_conventional_commit("feat: add repo intelligence"));
        assert!(is_conventional_commit("fix(cli): preserve user files"));
        assert!(is_conventional_commit(
            "feat(widget,ci): enhance sign-out handling"
        ));
        assert!(is_conventional_commit("chore!: drop legacy setup"));
    }

    #[test]
    fn conventional_commit_rejects_freeform_messages() {
        assert!(!is_conventional_commit("updated stuff"));
        assert!(!is_conventional_commit("Fix thing"));
        assert!(!is_conventional_commit("feat(widget,): missing scope"));
        assert!(!is_conventional_commit("feat(,ci): missing scope"));
        assert!(!is_conventional_commit(""));
    }

    #[test]
    fn check_repo_reports_missing_agent_files() {
        let root = temp_dir();
        let issues = check_repo(&root);

        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::MissingAgentsMd));
        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::MissingAgentsConfig));
        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::MissingAgentCheckScript));
        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::MissingGitHook));
    }

    #[test]
    fn check_repo_reports_missing_repo_intel_instructions() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::write(root.join("AGENTS.md"), "# Rules\n").unwrap();

        let issues = check_repo(&root);

        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::MissingRepoIntelInstructions));
    }

    #[test]
    fn check_repo_reports_empty_agent_integrations() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::write(
            root.join(".agents/agents.json"),
            "{\n  \"integrations\": {\n    \"enabled\": []\n  }\n}\n",
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::MissingAgentsIntegration));
    }

    #[test]
    fn check_repo_reports_hooks_that_do_not_run_agent_check() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::write(
            root.join(".husky/pre-commit"),
            "#!/bin/sh\nbun lint-staged --allow-empty\n",
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(issues.iter().any(|issue| {
            issue.code == IssueCode::MissingGitHook
                && issue.message.contains("scripts/agent-check --staged")
        }));
    }

    #[test]
    fn check_repo_reports_post_checkout_hooks_that_run_bun_install() {
        let root = temp_dir();
        write_minimal_repo_files_with_hooks(&root, ".husky");
        fs::write(
            root.join(".husky/post-checkout"),
            "#!/bin/sh\nset -eu\n\nbun install\n",
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(issues.iter().any(|issue| {
            issue.code == IssueCode::UnsafeGitHook
                && issue.message.contains("must not run bun install")
        }));
    }

    #[test]
    fn check_repo_accepts_vite_plus_hooks() {
        let root = temp_dir();
        write_minimal_repo_files_with_hooks(&root, ".vite-hooks");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"prepare":"is-ci || vp config"},"devDependencies":{"vite-plus":"latest"}}"#,
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(!issues
            .iter()
            .any(|issue| issue.code == IssueCode::MissingGitHook));
    }

    #[test]
    fn check_repo_reports_vite_plus_missing_hook_manager_override_for_dotagent_husky_guidance() {
        let root = temp_dir();
        write_minimal_repo_files_with_hooks(&root, ".vite-hooks");
        fs::write(
            root.join("AGENTS.md"),
            "# Rules\n\nRead `.agents/intel/summary.md` before broad exploration.\n\nUse Husky for git hooks.\n",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"prepare":"is-ci || vp config"},"devDependencies":{"vite-plus":"latest"}}"#,
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(issues.iter().any(|issue| {
            issue.code == IssueCode::MissingHookManagerInstructions
                && issue.message.contains("Vite+ .vite-hooks")
        }));
    }

    #[test]
    fn check_repo_accepts_vite_plus_hook_manager_override_for_dotagent_husky_guidance() {
        let root = temp_dir();
        write_minimal_repo_files_with_hooks(&root, ".vite-hooks");
        fs::write(
            root.join("AGENTS.md"),
            "# Rules\n\nRead `.agents/intel/summary.md` before broad exploration.\n\nUse Husky for git hooks.\n\nThis repo uses Vite+ hooks and overrides generic DotAgent hook guidance.\n",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"prepare":"is-ci || vp config"},"devDependencies":{"vite-plus":"latest"}}"#,
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(!issues
            .iter()
            .any(|issue| issue.code == IssueCode::MissingHookManagerInstructions));
    }

    #[test]
    fn check_repo_accepts_existing_husky_hooks_for_vite_plus_project() {
        let root = temp_dir();
        write_minimal_repo_files_with_hooks(&root, ".husky");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"prepare":"is-ci || vp config"},"devDependencies":{"vite-plus":"latest"}}"#,
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(!issues
            .iter()
            .any(|issue| issue.code == IssueCode::MissingGitHook));
    }

    #[test]
    fn check_repo_accepts_vite_plus_hooks_when_git_config_has_stale_husky_path() {
        let root = temp_dir();
        Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "core.hooksPath", ".husky"])
            .current_dir(&root)
            .output()
            .unwrap();
        write_minimal_repo_files_with_hooks(&root, ".vite-hooks");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"prepare":"is-ci || vp config"},"devDependencies":{"vite-plus":"latest"}}"#,
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(!issues
            .iter()
            .any(|issue| issue.code == IssueCode::MissingGitHook));
    }

    #[test]
    fn check_repo_reports_missing_vite_plus_hook_paths_for_vite_plus_projects() {
        let root = temp_dir();
        write_minimal_repo_files_without_hooks(&root);
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"prepare":"is-ci || vp config"},"devDependencies":{"vite-plus":"latest"}}"#,
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(issues.iter().any(|issue| {
            issue.code == IssueCode::MissingGitHook
                && issue.message.contains(".vite-hooks/pre-commit")
        }));
    }

    #[test]
    fn check_repo_reports_disallowed_tooling_for_new_standard() {
        let root = temp_dir();
        fs::create_dir_all(root.join(".agents")).unwrap();
        fs::write(root.join("AGENTS.md"), "# Rules\n").unwrap();
        fs::write(root.join(".agents/agents.json"), minimal_agents_json()).unwrap();
        fs::write(
			root.join("package.json"),
			r#"{"scripts":{"check":"tsc --noEmit","lint":"eslint .","format":"prettier . --check"}}"#,
		)
		.unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/index.js"),
            concat!("console", ".log('bad')\n"),
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::JavaScriptSource));
        assert!(issues.iter().any(|issue| issue.code == IssueCode::TscUsage));
        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::EslintUsage));
        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::PrettierUsage));
    }

    #[test]
    fn check_repo_allows_public_javascript_assets() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::create_dir_all(root.join("public")).unwrap();
        fs::write(
            root.join("public/sw.js"),
            "self.addEventListener('install', () => {})\n",
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(!issues
            .iter()
            .any(|issue| issue.code == IssueCode::JavaScriptSource));
    }

    #[test]
    fn check_repo_allows_nested_wrangler_generated_javascript() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::create_dir_all(root.join("apps/api/.wrangler/tmp/dev")).unwrap();
        fs::write(
            root.join("apps/api/.wrangler/tmp/dev/index.js"),
            "export default {}\n",
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(!issues
            .iter()
            .any(|issue| issue.code == IssueCode::JavaScriptSource));
    }

    #[test]
    fn check_repo_allows_secret_prefixes_and_schema_names() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/constants.ts"),
            concat!(
                "export const SECRET_KEY_LIVE_PREFIX = 'sk",
                "_live_'\n",
                "const generated = generateKey('sk",
                "_live_')\n"
            ),
        )
        .unwrap();
        fs::create_dir_all(root.join("packages/db/migrations")).unwrap();
        fs::write(
            root.join("packages/db/migrations/0000.sql"),
            "CREATE UNIQUE INDEX `sites_sk_live_unique` ON `sites` (`sk_live`);\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("tests/settings.test.ts"),
            concat!("const dummy = 'sk", "_live_123'\n"),
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(!issues
            .iter()
            .any(|issue| issue.code == IssueCode::HardcodedSecret));
    }

    #[test]
    fn check_repo_reports_deslop_patterns() {
        let root = temp_dir();
        fs::create_dir_all(root.join(".agents")).unwrap();
        fs::write(root.join("AGENTS.md"), "# Rules\n").unwrap();
        fs::write(root.join(".agents/agents.json"), minimal_agents_json()).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/index.ts"),
            concat!(
                "console",
                ".debug('debug')\nconst key = 'sk",
                "_live_1234567890abcdef1234567890'\n"
            ),
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::SlopPattern));
        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::HardcodedSecret));
    }

    #[test]
    fn check_repo_reports_realistic_secret_values() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/index.ts"),
            concat!("const key = 'sk", "_live_1234567890abcdef1234567890'\n"),
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::HardcodedSecret));
    }

    #[test]
    fn check_repo_skips_declaration_files_for_slop_patterns() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::write(
            root.join("worker-configuration.d.ts"),
            concat!(
                "declare const docs: \"console",
                ".debug() documents the platform API\"\n"
            ),
        )
        .unwrap();

        let issues = check_repo(&root);

        assert!(!issues
            .iter()
            .any(|issue| issue.code == IssueCode::SlopPattern));
    }

    #[test]
    fn check_repo_reports_disallowed_tracked_generated_files() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::create_dir_all(root.join(".git")).unwrap();
        let _ = Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        fs::write(root.join(".agents/local.json"), "{}\n").unwrap();
        fs::create_dir_all(root.join(".agents/generated")).unwrap();
        fs::write(root.join(".agents/generated/claude.md"), "@AGENTS.md\n").unwrap();
        let _ = Command::new("git")
            .args([
                "add",
                "AGENTS.md",
                ".agents/agents.json",
                ".agents/local.json",
                ".agents/generated/claude.md",
            ])
            .current_dir(&root)
            .output()
            .unwrap();

        let issues = check_repo(&root);

        assert!(issues
            .iter()
            .any(|issue| issue.code == IssueCode::DisallowedTrackedFile));
    }

    #[test]
    fn check_repo_allows_tracked_repo_intel_files() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::create_dir_all(root.join(".git")).unwrap();
        let _ = Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        fs::create_dir_all(root.join(".agents/intel")).unwrap();
        fs::write(root.join(".agents/intel/summary.md"), "# Generated\n").unwrap();
        fs::write(root.join(".agents/intel/api.md"), "# API\n").unwrap();
        let _ = Command::new("git")
            .args([
                "add",
                "AGENTS.md",
                ".agents/agents.json",
                ".agents/intel/summary.md",
                ".agents/intel/api.md",
            ])
            .current_dir(&root)
            .output()
            .unwrap();

        let issues = check_repo(&root);

        assert!(!issues
            .iter()
            .any(|issue| issue.code == IssueCode::DisallowedTrackedFile));
    }

    #[test]
    fn check_repo_allows_tracked_tool_adapter_files() {
        let root = temp_dir();
        write_minimal_repo_files(&root);
        fs::create_dir_all(root.join(".git")).unwrap();
        let _ = Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        fs::write(root.join("CLAUDE.md"), "@AGENTS.md\n").unwrap();
        fs::create_dir_all(root.join(".codex")).unwrap();
        fs::write(root.join(".codex/AGENTS.md"), "@../AGENTS.md\n").unwrap();
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(root.join(".claude/settings.json"), "{}\n").unwrap();
        fs::create_dir_all(root.join(".cursor")).unwrap();
        fs::write(root.join(".cursor/rules.md"), "@../AGENTS.md\n").unwrap();
        fs::create_dir_all(root.join(".gemini")).unwrap();
        fs::write(root.join(".gemini/GEMINI.md"), "@../AGENTS.md\n").unwrap();
        fs::create_dir_all(root.join(".antigravity")).unwrap();
        fs::write(root.join(".antigravity/rules.md"), "@../AGENTS.md\n").unwrap();
        fs::create_dir_all(root.join(".windsurf")).unwrap();
        fs::write(root.join(".windsurf/rules.md"), "@../AGENTS.md\n").unwrap();
        fs::create_dir_all(root.join(".opencode")).unwrap();
        fs::write(root.join(".opencode/config.json"), "{}\n").unwrap();
        fs::write(root.join("opencode.json"), "{}\n").unwrap();
        fs::create_dir_all(root.join(".junie")).unwrap();
        fs::write(root.join(".junie/guidelines.md"), "@../AGENTS.md\n").unwrap();
        let _ = Command::new("git")
            .args([
                "add",
                "AGENTS.md",
                ".agents/agents.json",
                "CLAUDE.md",
                ".codex/AGENTS.md",
                ".claude/settings.json",
                ".cursor/rules.md",
                ".gemini/GEMINI.md",
                ".antigravity/rules.md",
                ".windsurf/rules.md",
                ".opencode/config.json",
                "opencode.json",
                ".junie/guidelines.md",
            ])
            .current_dir(&root)
            .output()
            .unwrap();

        let issues = check_repo(&root);

        assert!(!issues
            .iter()
            .any(|issue| issue.code == IssueCode::DisallowedTrackedFile));
    }

    fn write_minimal_repo_files(root: &Path) {
        write_minimal_repo_files_with_hooks(root, ".husky");
    }

    fn write_minimal_repo_files_without_hooks(root: &Path) {
        fs::create_dir_all(root.join(".agents")).unwrap();
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(
            root.join("AGENTS.md"),
            "# Rules\n\nRead `.agents/intel/summary.md` before broad exploration.\n",
        )
        .unwrap();
        fs::write(root.join(".agents/agents.json"), minimal_agents_json()).unwrap();
        fs::write(root.join("scripts/agent-check"), "#!/bin/sh\n").unwrap();
    }

    fn write_minimal_repo_files_with_hooks(root: &Path, hook_dir: &str) {
        write_minimal_repo_files_without_hooks(root);
        fs::create_dir_all(root.join(hook_dir)).unwrap();
        fs::write(
            root.join(hook_dir).join("pre-commit"),
            "#!/bin/sh\nscripts/agent-check --staged\n",
        )
        .unwrap();
        fs::write(
            root.join(hook_dir).join("pre-push"),
            "#!/bin/sh\nscripts/agent-check\n",
        )
        .unwrap();
        fs::write(
            root.join(hook_dir).join("commit-msg"),
            "#!/bin/sh\necho \"Conventional Commit\"\n",
        )
        .unwrap();
    }

    fn minimal_agents_json() -> String {
        format!(
            "{{\"integrations\":{{\"enabled\":[{}]}}}}\n",
            DEFAULT_INTEGRATIONS
                .iter()
                .map(|integration| format!("\"{integration}\""))
                .collect::<Vec<String>>()
                .join(",")
        )
    }
}
