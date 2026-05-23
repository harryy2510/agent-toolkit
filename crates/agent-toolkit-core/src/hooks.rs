use std::fs;
use std::io::Write;
use std::path::Path;

pub(crate) const DEFAULT_INTEGRATIONS: [&str; 9] = [
    "codex",
    "claude",
    "gemini",
    "copilot_vscode",
    "cursor",
    "antigravity",
    "windsurf",
    "opencode",
    "junie",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HookLayout {
    pub source_dir: &'static str,
    pub label: &'static str,
}

pub(crate) fn detect_hook_layout(root: &Path) -> HookLayout {
    let has_vite_hooks = root.join(".vite-hooks").exists();
    let has_husky_hooks = root.join(".husky").exists();
    let configured_path = configured_hooks_path(root);

    if has_vite_hooks && has_husky_hooks {
        if configured_path
            .as_deref()
            .is_some_and(|path| path.starts_with(".husky"))
        {
            return husky_hook_layout();
        }
        return vite_plus_hook_layout();
    }

    if has_vite_hooks {
        return vite_plus_hook_layout();
    }

    if has_husky_hooks {
        return husky_hook_layout();
    }

    if let Some(configured_path) = configured_path {
        if configured_path.starts_with(".vite-hooks") {
            return vite_plus_hook_layout();
        }
        if configured_path.starts_with(".husky") {
            return husky_hook_layout();
        }
    }

    if uses_vite_plus(root) {
        return vite_plus_hook_layout();
    }

    husky_hook_layout()
}

pub(crate) fn hook_source_path(layout: HookLayout, hook_name: &str) -> String {
    format!("{}/{}", layout.source_dir, hook_name)
}

pub(crate) fn hook_runtime_path(root: &Path, layout: HookLayout) -> String {
    if layout.source_dir == ".vite-hooks" && root.join(".vite-hooks/_").exists() {
        return ".vite-hooks/_".to_string();
    }

    layout.source_dir.to_string()
}

fn husky_hook_layout() -> HookLayout {
    HookLayout {
        source_dir: ".husky",
        label: "Husky",
    }
}

fn vite_plus_hook_layout() -> HookLayout {
    HookLayout {
        source_dir: ".vite-hooks",
        label: "Vite+",
    }
}

fn configured_hooks_path(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["config", "--local", "--get", "core.hooksPath"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let hooks_path = String::from_utf8(output.stdout).ok()?;
    let hooks_path = hooks_path.trim();
    if hooks_path.is_empty() {
        return None;
    }

    Some(hooks_path.to_string())
}

fn uses_vite_plus(root: &Path) -> bool {
    fs::read_to_string(root.join("package.json")).is_ok_and(|contents| {
        contents.contains("\"vite-plus\"")
            || contents.contains("@voidzero-dev/vite-plus")
            || contents.contains("vp config")
            || contents.contains("vp staged")
    }) || fs::read_to_string(root.join("vite.config.ts"))
        .is_ok_and(|contents| contents.contains("vite-plus"))
}

const REPO_INTEL_AGENTS_BLOCK: &str = concat!(
    "<!-- AGENT-TOOLKIT:REPO-INTEL:START -->\n",
    "## Agent Toolkit Repo Intelligence\n\n",
    "- Before broad exploration, read `.agents/intel/summary.md` if it exists.\n",
    "- Treat `summary.md`, `overview.md`, and `tasks.md` as the startup index; open task-specific files only after that index points to them.\n",
    "- Treat `.agents/intel/repo.json`, `database.md`, `imports.md`, and `calls.md` as deep-read only. Do not bulk-read them during normal startup.\n",
    "- `.agents/intel/` is generated repo intelligence and may be committed in migrated repos.\n",
    "<!-- AGENT-TOOLKIT:REPO-INTEL:END -->\n",
);
const HOOK_MANAGER_BLOCK_START: &str = "<!-- AGENT-TOOLKIT:HOOK-MANAGER:START -->";
const HOOK_MANAGER_BLOCK_END: &str = "<!-- AGENT-TOOLKIT:HOOK-MANAGER:END -->";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapChangeKind {
    Created,
    Updated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapChange {
    pub kind: BootstrapChangeKind,
    pub path: String,
}

impl BootstrapChange {
    pub fn verb(&self) -> &'static str {
        match self.kind {
            BootstrapChangeKind::Created => "created",
            BootstrapChangeKind::Updated => "updated",
        }
    }

    fn created(path: &str) -> Self {
        Self {
            kind: BootstrapChangeKind::Created,
            path: path.to_string(),
        }
    }

    fn updated(path: &str) -> Self {
        Self {
            kind: BootstrapChangeKind::Updated,
            path: path.to_string(),
        }
    }
}

pub fn bootstrap_repo(root: &Path) -> std::io::Result<Vec<BootstrapChange>> {
    let mut changes = Vec::new();
    let hook_layout = detect_hook_layout(root);
    create_file_if_missing(root, "AGENTS.md", agents_md(), &mut changes)?;
    ensure_repo_intel_instructions(root, &mut changes)?;
    ensure_hook_manager_instructions(root, hook_layout, &mut changes)?;
    ensure_agents_json(root, &mut changes)?;
    create_file_if_missing(root, ".agents/README.md", agents_readme(), &mut changes)?;
    create_file_if_missing(
        root,
        "scripts/agent-check",
        agent_check_script(),
        &mut changes,
    )?;
    ensure_hook(
        root,
        &hook_source_path(hook_layout, "pre-commit"),
        pre_commit_hook(),
        "scripts/agent-check --staged",
        pre_commit_agent_check_block(),
        &mut changes,
    )?;
    ensure_hook(
        root,
        &hook_source_path(hook_layout, "pre-push"),
        pre_push_hook(),
        "scripts/agent-check",
        pre_push_agent_check_block(),
        &mut changes,
    )?;
    ensure_hook(
        root,
        &hook_source_path(hook_layout, "commit-msg"),
        commit_msg_hook(),
        "Conventional Commit",
        commit_msg_agent_check_block(),
        &mut changes,
    )?;
    remove_managed_post_checkout_install(
        root,
        &hook_source_path(hook_layout, "post-checkout"),
        &mut changes,
    )?;
    ensure_gitignore_entries(root, &mut changes)?;
    make_executable(root.join("scripts/agent-check").as_path())?;
    make_executable(
        root.join(hook_source_path(hook_layout, "pre-commit"))
            .as_path(),
    )?;
    make_executable(
        root.join(hook_source_path(hook_layout, "pre-push"))
            .as_path(),
    )?;
    make_executable(
        root.join(hook_source_path(hook_layout, "commit-msg"))
            .as_path(),
    )?;
    configure_git_hooks_path(root, hook_layout);
    Ok(changes)
}

pub fn commit_msg_hook() -> &'static str {
    "#!/bin/sh\nset -eu\n\nfirst_line=$(sed -n '1p' \"$1\")\n\nif printf '%s\\n' \"$first_line\" | grep -Eq '^[a-z0-9-]+(\\([a-z0-9._/-]+(,[a-z0-9._/-]+)*\\))?!?: .+'; then\n\texit 0\nfi\n\necho \"Commit message must use Conventional Commit format, for example: feat: add repo intelligence\" >&2\nexit 1\n"
}

fn agents_md() -> &'static str {
    concat!(
        "# Repository Agent Instructions\n\n",
        "Use this file as the canonical source for coding agent guidance in this repo.\n\n",
        "<!-- AGENT-TOOLKIT:REPO-INTEL:START -->\n",
        "## Agent Toolkit Repo Intelligence\n\n",
        "- Before broad exploration, read `.agents/intel/summary.md` if it exists.\n",
        "- Treat `summary.md`, `overview.md`, and `tasks.md` as the startup index; open task-specific files only after that index points to them.\n",
        "- Treat `.agents/intel/repo.json`, `database.md`, `imports.md`, and `calls.md` as deep-read only. Do not bulk-read them during normal startup.\n",
        "- `.agents/intel/` is generated repo intelligence and may be committed in migrated repos.\n",
        "<!-- AGENT-TOOLKIT:REPO-INTEL:END -->\n",
    )
}

fn pre_commit_hook() -> &'static str {
    "#!/bin/sh\nset -eu\n\nscripts/agent-check --staged\n"
}

fn pre_push_hook() -> &'static str {
    "#!/bin/sh\nset -eu\n\nscripts/agent-check\n"
}

fn old_post_checkout_install_hook() -> &'static str {
    "#!/bin/sh\nset -eu\n\nbun install\n"
}

fn pre_commit_agent_check_block() -> &'static str {
    "# agent-toolkit:start\nscripts/agent-check --staged\n# agent-toolkit:end\n"
}

fn pre_push_agent_check_block() -> &'static str {
    "# agent-toolkit:start\nscripts/agent-check\n# agent-toolkit:end\n"
}

fn commit_msg_agent_check_block() -> &'static str {
    "# agent-toolkit:start\nfirst_line=$(sed -n '1p' \"$1\")\n\nif ! printf '%s\\n' \"$first_line\" | grep -Eq '^[a-z0-9-]+(\\([a-z0-9._/-]+(,[a-z0-9._/-]+)*\\))?!?: .+'; then\n\techo \"Commit message must use Conventional Commit format, for example: feat: add repo intelligence\" >&2\n\texit 1\nfi\n# agent-toolkit:end\n"
}

fn old_post_checkout_install_block() -> &'static str {
    "# agent-toolkit:start\nbun install\n# agent-toolkit:end\n"
}

fn agent_check_script() -> &'static str {
    "#!/bin/sh\nset -eu\n\nif [ \"${1:-}\" = \"--staged\" ] && { [ \"${CI:-}\" = \"true\" ] || [ \"${GITHUB_ACTIONS:-}\" = \"true\" ]; }; then\n\techo \"Skipping agent-check in CI pre-commit\"\n\texit 0\nfi\n\nif [ -z \"${AGENT_TOOLKIT_BIN:-}\" ] && ! command -v bunx >/dev/null 2>&1; then\n\techo \"agent-toolkit requires bunx. Install Bun, then rerun this command.\" >&2\n\texit 1\nfi\n\nif [ -n \"${AGENT_TOOLKIT_BIN:-}\" ]; then\n\t\"$AGENT_TOOLKIT_BIN\" repo check \"$@\"\nelse\n\tbunx @harryy/agent-toolkit repo check \"$@\"\nfi\n\nif [ \"${AGENT_TOOLKIT_SYNC_CHECK:-}\" = \"1\" ] && command -v agents >/dev/null 2>&1; then\n\tagents sync --path . --check\nfi\n"
}

fn agents_json() -> &'static str {
    "{\n\t\"schemaVersion\": 3,\n\t\"instructions\": {\n\t\t\"path\": \"AGENTS.md\"\n\t},\n\t\"integrations\": {\n\t\t\"enabled\": [\n\t\t\t\"codex\",\n\t\t\t\"claude\",\n\t\t\t\"gemini\",\n\t\t\t\"copilot_vscode\",\n\t\t\t\"cursor\",\n\t\t\t\"antigravity\",\n\t\t\t\"windsurf\",\n\t\t\t\"opencode\",\n\t\t\t\"junie\"\n\t\t],\n\t\t\"options\": {\n\t\t\t\"cursorAutoApprove\": true,\n\t\t\t\"antigravityGlobalSync\": false\n\t\t}\n\t},\n\t\"syncMode\": \"source-only\",\n\t\"mcp\": {\n\t\t\"servers\": {}\n\t},\n\t\"workspace\": {\n\t\t\"vscode\": {\n\t\t\t\"hideGenerated\": true,\n\t\t\t\"hiddenPaths\": [\n\t\t\t\t\"**/.agents/generated\"\n\t\t\t]\n\t\t}\n\t},\n\t\"lastSync\": null,\n\t\"lastSyncSourceHash\": null\n}\n"
}

fn agents_readme() -> &'static str {
    "# .agents\n\nProject-local source files for agent setup.\n\n- `agents.json`: cross-agent sync config\n- `skills/`: committed project skills, including vendored DotAgent skills when `agent-toolkit repo dotagent` is used\n- `dotagent/`: vendored DotAgent role-profile and command references when DotAgent is installed for the repo\n- `dotagent.lock.json`: pinned DotAgent rules and skills snapshot when DotAgent is installed for the repo\n- `intel/`: generated repo intelligence that may be committed in migrated repos. Agents should start at `intel/summary.md`, use `intel/manifest.json` for read-policy tiers, and treat `repo.json`, `database.md`, `imports.md`, and `calls.md` as deep-read only.\n- `local.json`: machine-specific overrides, ignored by git\n"
}

fn create_file_if_missing(
    root: &Path,
    relative_path: &str,
    contents: &str,
    changes: &mut Vec<BootstrapChange>,
) -> std::io::Result<()> {
    let path = root.join(relative_path);
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(path)?;
    file.write_all(contents.as_bytes())?;
    changes.push(BootstrapChange::created(relative_path));
    Ok(())
}

fn ensure_agents_json(root: &Path, changes: &mut Vec<BootstrapChange>) -> std::io::Result<()> {
    let path = root.join(".agents/agents.json");
    if !path.exists() {
        create_file_if_missing(root, ".agents/agents.json", agents_json(), changes)?;
        return Ok(());
    }

    let existing = fs::read_to_string(&path)?;
    let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&existing) else {
        return Ok(());
    };
    if !config.is_object() {
        return Ok(());
    }
    let default_config =
        serde_json::from_str::<serde_json::Value>(agents_json()).map_err(json_to_io_error)?;
    let mut changed = merge_missing_json(&mut config, &default_config);

    {
        let config_object = config.as_object_mut().unwrap();
        let integrations_value = config_object
            .entry("integrations")
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if !integrations_value.is_object() {
            *integrations_value = serde_json::Value::Object(serde_json::Map::new());
            changed = true;
        }
        let integrations = integrations_value.as_object_mut().unwrap();
        let enabled_value = integrations
            .entry("enabled")
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if !enabled_value.is_array() {
            *enabled_value = serde_json::Value::Array(Vec::new());
            changed = true;
        }
        let enabled = enabled_value.as_array_mut().unwrap();

        for integration in DEFAULT_INTEGRATIONS {
            if !enabled
                .iter()
                .any(|entry| entry.as_str() == Some(integration))
            {
                enabled.push(serde_json::Value::String(integration.to_string()));
                changed = true;
            }
        }
    }
    changed |= merge_missing_json(&mut config, &default_config);

    if changed {
        let mut updated = serde_json::to_string_pretty(&config).map_err(json_to_io_error)?;
        updated.push('\n');
        fs::write(path, updated)?;
        changes.push(BootstrapChange::updated(".agents/agents.json"));
    }

    Ok(())
}

fn merge_missing_json(target: &mut serde_json::Value, defaults: &serde_json::Value) -> bool {
    let (Some(target_object), Some(default_object)) =
        (target.as_object_mut(), defaults.as_object())
    else {
        return false;
    };
    let mut changed = false;

    for (key, default_value) in default_object {
        if let Some(target_value) = target_object.get_mut(key) {
            changed |= merge_missing_json(target_value, default_value);
        } else {
            target_object.insert(key.clone(), default_value.clone());
            changed = true;
        }
    }

    changed
}

fn json_to_io_error(error: serde_json::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error)
}

fn ensure_hook(
    root: &Path,
    relative_path: &str,
    create_contents: &str,
    required_needle: &str,
    append_block: &str,
    changes: &mut Vec<BootstrapChange>,
) -> std::io::Result<()> {
    let path = root.join(relative_path);
    if !path.exists() {
        create_file_if_missing(root, relative_path, create_contents, changes)?;
        return Ok(());
    }

    let existing = fs::read_to_string(&path)?;
    if existing.contains(required_needle) {
        return Ok(());
    }

    let mut updated = existing.trim_end().to_string();
    if !updated.is_empty() {
        updated.push_str("\n\n");
    }
    updated.push_str(append_block);
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    fs::write(path, updated)?;
    changes.push(BootstrapChange::updated(relative_path));
    Ok(())
}

fn remove_managed_post_checkout_install(
    root: &Path,
    relative_path: &str,
    changes: &mut Vec<BootstrapChange>,
) -> std::io::Result<()> {
    let path = root.join(relative_path);
    let Ok(existing) = fs::read_to_string(&path) else {
        return Ok(());
    };

    if existing == old_post_checkout_install_hook() {
        fs::remove_file(path)?;
        changes.push(BootstrapChange::updated(relative_path));
        return Ok(());
    }

    let updated = existing.replace(old_post_checkout_install_block(), "");
    if updated == existing {
        return Ok(());
    }

    if hook_has_only_shell_boilerplate(&updated) {
        fs::remove_file(path)?;
    } else {
        let mut updated = updated.trim_end().to_string();
        updated.push('\n');
        fs::write(path, updated)?;
    }
    changes.push(BootstrapChange::updated(relative_path));
    Ok(())
}

fn hook_has_only_shell_boilerplate(contents: &str) -> bool {
    let meaningful_lines: Vec<&str> = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    meaningful_lines.is_empty()
        || meaningful_lines == ["#!/bin/sh"]
        || meaningful_lines == ["#!/bin/sh", "set -eu"]
        || meaningful_lines == ["#!/usr/bin/env sh"]
        || meaningful_lines == ["#!/usr/bin/env sh", "set -eu"]
}

fn ensure_repo_intel_instructions(
    root: &Path,
    changes: &mut Vec<BootstrapChange>,
) -> std::io::Result<()> {
    ensure_managed_block(
        root,
        "AGENTS.md",
        "<!-- AGENT-TOOLKIT:REPO-INTEL:START -->",
        "<!-- AGENT-TOOLKIT:REPO-INTEL:END -->",
        repo_intel_agents_block(),
        changes,
    )
}

fn ensure_hook_manager_instructions(
    root: &Path,
    layout: HookLayout,
    changes: &mut Vec<BootstrapChange>,
) -> std::io::Result<()> {
    ensure_managed_block(
        root,
        "AGENTS.md",
        HOOK_MANAGER_BLOCK_START,
        HOOK_MANAGER_BLOCK_END,
        &hook_manager_agents_block(root, layout),
        changes,
    )
}

fn repo_intel_agents_block() -> &'static str {
    REPO_INTEL_AGENTS_BLOCK
}

fn hook_manager_agents_block(root: &Path, layout: HookLayout) -> String {
    let runtime_path = hook_runtime_path(root, layout);
    let runtime_guidance = if layout.source_dir == ".vite-hooks" {
        format!(
            "- Keep Git `core.hooksPath` set to `{runtime_path}`; if `.vite-hooks/_` wrappers are missing, regenerate them with `vp config`.\n"
        )
    } else {
        format!("- Keep Git `core.hooksPath` set to `{runtime_path}`.\n")
    };

    format!(
        "{HOOK_MANAGER_BLOCK_START}\n## Agent Toolkit Hook Manager\n\n- This repo uses {} hooks. Keep `{}/` as the active committed hook source.\n{}- This repo-specific hook manager rule overrides generic DotAgent hook guidance.\n- Do not introduce `.githooks` or another ad hoc hook folder.\n{HOOK_MANAGER_BLOCK_END}\n",
        layout.label, layout.source_dir, runtime_guidance
    )
}

fn ensure_managed_block(
    root: &Path,
    relative_path: &str,
    start_marker: &str,
    end_marker: &str,
    block: &str,
    changes: &mut Vec<BootstrapChange>,
) -> std::io::Result<()> {
    let path = root.join(relative_path);
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let updated = if let Some(start_index) = existing.find(start_marker) {
        let Some(end_relative_index) = existing[start_index..].find(end_marker) else {
            return Ok(());
        };
        let end_index = start_index + end_relative_index + end_marker.len();
        let mut next = String::new();
        next.push_str(&existing[..start_index]);
        next.push_str(block.trim_end());
        next.push_str(&existing[end_index..]);
        next
    } else {
        let mut next = existing.trim_end().to_string();
        if !next.is_empty() {
            next.push_str("\n\n");
        }
        next.push_str(block.trim_end());
        next
    };

    let mut updated = updated.trim_end().to_string();
    updated.push('\n');

    if updated != existing {
        fs::write(path, updated)?;
        changes.push(BootstrapChange::updated(relative_path));
    }

    Ok(())
}

fn ensure_gitignore_entries(
    root: &Path,
    changes: &mut Vec<BootstrapChange>,
) -> std::io::Result<()> {
    let path = root.join(".gitignore");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut updated = existing.trim_end().to_string();
    let mut changed = false;

    for entry in [".agents/local.json", ".agents/generated/"] {
        if has_gitignore_entry(&existing, entry) {
            continue;
        }
        if !updated.is_empty() {
            updated.push('\n');
        }
        updated.push_str(entry);
        changed = true;
    }

    if changed {
        updated.push('\n');
        fs::write(path, updated)?;
        changes.push(BootstrapChange::updated(".gitignore"));
    }

    Ok(())
}

fn has_gitignore_entry(contents: &str, entry: &str) -> bool {
    contents
        .lines()
        .map(str::trim)
        .any(|line| line == entry || line == format!("/{entry}"))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if path.exists() {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn configure_git_hooks_path(root: &Path, hook_layout: HookLayout) {
    if !root.join(".git").exists() {
        return;
    }
    let hooks_path = hook_runtime_path(root, hook_layout);
    let _ = std::process::Command::new("git")
        .args(["config", "core.hooksPath", hooks_path.as_str()])
        .current_dir(root)
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "agent-toolkit-hooks-{}-{}",
            std::process::id(),
            TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn commit_msg_hook_enforces_conventional_commits() {
        let hook = commit_msg_hook();

        assert!(hook.contains("grep -Eq"));
        assert!(hook.contains("(,[a-z0-9._/-]+)*"));
        assert!(hook.contains("Conventional Commit"));
    }

    #[test]
    fn bootstrap_repo_creates_agent_files_and_hooks() {
        let root = temp_dir();
        let changes = bootstrap_repo(&root).unwrap();

        assert!(has_change(
            &changes,
            BootstrapChangeKind::Created,
            "AGENTS.md"
        ));
        assert!(has_change(
            &changes,
            BootstrapChangeKind::Created,
            ".agents/agents.json"
        ));
        assert!(has_change(
            &changes,
            BootstrapChangeKind::Created,
            ".husky/pre-commit"
        ));
        assert!(has_change(
            &changes,
            BootstrapChangeKind::Created,
            ".husky/pre-push"
        ));
        assert!(has_change(
            &changes,
            BootstrapChangeKind::Created,
            ".husky/commit-msg"
        ));
        assert!(has_change(
            &changes,
            BootstrapChangeKind::Updated,
            ".gitignore"
        ));
        assert!(root.join(".husky/commit-msg").exists());
        assert!(!root.join(".husky/post-checkout").exists());
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains("Before broad exploration, read `.agents/intel/summary.md`"));
        assert!(agents.contains("repo.json`, `database.md`, `imports.md`, and `calls.md`"));
        assert!(agents.contains("This repo uses Husky hooks"));
        assert!(agents.contains("overrides generic DotAgent hook guidance"));
        assert_eq!(
            agents
                .matches("<!-- AGENT-TOOLKIT:REPO-INTEL:START -->")
                .count(),
            1
        );
        assert_eq!(
            agents
                .matches("<!-- AGENT-TOOLKIT:HOOK-MANAGER:START -->")
                .count(),
            1
        );
        let agents_readme = fs::read_to_string(root.join(".agents/README.md")).unwrap();
        assert!(agents_readme.contains("Agents should start at `intel/summary.md`"));
        assert!(agents_readme.contains("deep-read only"));
        let agents_json = fs::read_to_string(root.join(".agents/agents.json")).unwrap();
        assert!(agents_json.contains("\"claude\""));
        assert!(agents_json.contains("\"gemini\""));
        assert!(agents_json.contains("\"junie\""));
        assert!(!agents_json.contains("\"**/.cursor\""));
        assert!(!agents_json.contains("\"**/.gemini\""));
        let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(gitignore.contains(".agents/local.json"));
        assert!(gitignore.contains(".agents/generated/"));
        assert!(!gitignore.contains(".agents/intel/"));
        assert!(!gitignore.contains("/CLAUDE.md"));
        assert!(!gitignore.contains("/.cursor/"));
        assert!(!gitignore.contains("/.gemini/"));
        let agent_check = fs::read_to_string(root.join("scripts/agent-check")).unwrap();
        assert!(agent_check.contains("Skipping agent-check in CI pre-commit"));
        let pre_commit = fs::read_to_string(root.join(".husky/pre-commit")).unwrap();
        assert!(!pre_commit.contains("Skipping agent-check in CI pre-commit"));
    }

    #[test]
    fn bootstrap_repo_uses_vite_plus_hooks_when_project_uses_vite_plus() {
        let root = temp_dir();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"prepare":"is-ci || vp config"},"devDependencies":{"vite-plus":"latest"}}"#,
        )
        .unwrap();

        let changes = bootstrap_repo(&root).unwrap();

        assert!(has_change(
            &changes,
            BootstrapChangeKind::Created,
            ".vite-hooks/pre-commit"
        ));
        assert!(has_change(
            &changes,
            BootstrapChangeKind::Created,
            ".vite-hooks/pre-push"
        ));
        assert!(has_change(
            &changes,
            BootstrapChangeKind::Created,
            ".vite-hooks/commit-msg"
        ));
        assert!(!root.join(".husky/pre-commit").exists());
        assert!(root.join(".vite-hooks/commit-msg").exists());
        assert!(!root.join(".vite-hooks/post-checkout").exists());
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains("This repo uses Vite+ hooks"));
        assert!(agents.contains("Keep `.vite-hooks/` as the active committed hook source"));
        assert!(agents.contains("overrides generic DotAgent hook guidance"));
    }

    #[test]
    fn bootstrap_repo_respects_existing_husky_hooks_for_vite_plus_project() {
        let root = temp_dir();
        fs::create_dir_all(root.join(".husky")).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"prepare":"is-ci || vp config"},"devDependencies":{"vite-plus":"latest"}}"#,
        )
        .unwrap();

        let changes = bootstrap_repo(&root).unwrap();

        assert!(has_change(
            &changes,
            BootstrapChangeKind::Created,
            ".husky/pre-commit"
        ));
        assert!(root.join(".husky/commit-msg").exists());
        assert!(!root.join(".husky/post-checkout").exists());
        assert!(!root.join(".vite-hooks/pre-commit").exists());
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains("This repo uses Husky hooks"));
        assert!(!agents.contains("This repo uses Vite+ hooks"));
    }

    #[test]
    fn bootstrap_repo_uses_vite_plus_hooks_when_git_config_has_stale_husky_path() {
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
        fs::create_dir_all(root.join(".vite-hooks/_")).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"prepare":"is-ci || vp config"},"devDependencies":{"vite-plus":"latest"}}"#,
        )
        .unwrap();

        let changes = bootstrap_repo(&root).unwrap();
        let hooks_path = Command::new("git")
            .args(["config", "--local", "--get", "core.hooksPath"])
            .current_dir(&root)
            .output()
            .unwrap();

        assert!(has_change(
            &changes,
            BootstrapChangeKind::Created,
            ".vite-hooks/pre-commit"
        ));
        assert!(!root.join(".husky/pre-commit").exists());
        assert_eq!(
            String::from_utf8(hooks_path.stdout).unwrap().trim(),
            ".vite-hooks/_"
        );
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains("This repo uses Vite+ hooks"));
        assert!(agents.contains("core.hooksPath` set to `.vite-hooks/_`"));
    }

    #[test]
    fn bootstrap_repo_fills_existing_empty_integrations() {
        let root = temp_dir();
        fs::create_dir_all(root.join(".agents")).unwrap();
        fs::write(root.join(".agents/agents.json"), "{}\n").unwrap();

        let first_changes = bootstrap_repo(&root).unwrap();
        let second_changes = bootstrap_repo(&root).unwrap();

        let agents_json = fs::read_to_string(root.join(".agents/agents.json")).unwrap();
        assert!(agents_json.contains("\"codex\""));
        assert!(agents_json.contains("\"claude\""));
        assert!(agents_json.contains("\"gemini\""));
        assert!(agents_json.contains("\"opencode\""));
        assert!(agents_json.contains("\"schemaVersion\""));
        assert!(agents_json.contains("\"path\": \"AGENTS.md\""));
        assert!(agents_json.contains("\"syncMode\""));
        assert!(has_change(
            &first_changes,
            BootstrapChangeKind::Updated,
            ".agents/agents.json"
        ));
        assert!(!has_change(
            &second_changes,
            BootstrapChangeKind::Updated,
            ".agents/agents.json"
        ));
    }

    #[test]
    fn bootstrap_repo_preserves_gitignore_and_deduplicates_agent_entries() {
        let root = temp_dir();
        fs::write(root.join(".gitignore"), "node_modules/\n").unwrap();

        let first_changes = bootstrap_repo(&root).unwrap();
        let second_changes = bootstrap_repo(&root).unwrap();

        let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(gitignore.contains("node_modules/"));
        assert_eq!(gitignore.matches(".agents/local.json").count(), 1);
        assert_eq!(gitignore.matches(".agents/generated/").count(), 1);
        assert!(has_change(
            &first_changes,
            BootstrapChangeKind::Updated,
            ".gitignore"
        ));
        assert!(!has_change(
            &second_changes,
            BootstrapChangeKind::Updated,
            ".gitignore"
        ));
    }

    #[test]
    fn bootstrap_repo_updates_existing_agents_with_intel_block() {
        let root = temp_dir();
        fs::write(
            root.join("AGENTS.md"),
            "# Existing Instructions\n\nKeep this repo-specific guidance.\n",
        )
        .unwrap();

        let first_changes = bootstrap_repo(&root).unwrap();
        let second_changes = bootstrap_repo(&root).unwrap();

        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains("Keep this repo-specific guidance."));
        assert!(agents.contains("Before broad exploration, read `.agents/intel/summary.md`"));
        assert!(agents.contains("Do not bulk-read them during normal startup."));
        assert_eq!(
            agents
                .matches("<!-- AGENT-TOOLKIT:REPO-INTEL:START -->")
                .count(),
            1
        );
        assert!(has_change(
            &first_changes,
            BootstrapChangeKind::Updated,
            "AGENTS.md"
        ));
        assert!(!has_change(
            &second_changes,
            BootstrapChangeKind::Updated,
            "AGENTS.md"
        ));
    }

    #[test]
    fn bootstrap_repo_integrates_existing_hooks_without_overwriting() {
        let root = temp_dir();
        fs::create_dir_all(root.join(".husky")).unwrap();
        fs::write(
            root.join(".husky/pre-commit"),
            "#!/bin/sh\nset -eu\n\nbun lint-staged --allow-empty\n",
        )
        .unwrap();

        let first_changes = bootstrap_repo(&root).unwrap();
        let second_changes = bootstrap_repo(&root).unwrap();

        let hook = fs::read_to_string(root.join(".husky/pre-commit")).unwrap();
        assert!(hook.contains("bun lint-staged --allow-empty"));
        assert_eq!(hook.matches("scripts/agent-check --staged").count(), 1);
        assert_eq!(hook.matches("# agent-toolkit:start").count(), 1);
        assert!(has_change(
            &first_changes,
            BootstrapChangeKind::Updated,
            ".husky/pre-commit"
        ));
        assert!(!has_change(
            &second_changes,
            BootstrapChangeKind::Updated,
            ".husky/pre-commit"
        ));
    }

    #[test]
    fn bootstrap_repo_removes_managed_post_checkout_install_hook() {
        let root = temp_dir();
        fs::create_dir_all(root.join(".husky")).unwrap();
        fs::write(
            root.join(".husky/post-checkout"),
            old_post_checkout_install_hook(),
        )
        .unwrap();

        let first_changes = bootstrap_repo(&root).unwrap();
        let second_changes = bootstrap_repo(&root).unwrap();

        assert!(!root.join(".husky/post-checkout").exists());
        assert!(has_change(
            &first_changes,
            BootstrapChangeKind::Updated,
            ".husky/post-checkout"
        ));
        assert!(!has_change(
            &second_changes,
            BootstrapChangeKind::Updated,
            ".husky/post-checkout"
        ));
    }

    #[test]
    fn bootstrap_repo_removes_managed_post_checkout_install_block_without_overwriting() {
        let root = temp_dir();
        fs::create_dir_all(root.join(".husky")).unwrap();
        fs::write(
            root.join(".husky/post-checkout"),
            "#!/bin/sh\nset -eu\n\necho checked out\n\n# agent-toolkit:start\nbun install\n# agent-toolkit:end\n",
        )
        .unwrap();

        let first_changes = bootstrap_repo(&root).unwrap();
        let second_changes = bootstrap_repo(&root).unwrap();

        let hook = fs::read_to_string(root.join(".husky/post-checkout")).unwrap();
        assert!(hook.contains("echo checked out"));
        assert!(!hook.contains("bun install"));
        assert!(!hook.contains("# agent-toolkit:start"));
        assert!(has_change(
            &first_changes,
            BootstrapChangeKind::Updated,
            ".husky/post-checkout"
        ));
        assert!(!has_change(
            &second_changes,
            BootstrapChangeKind::Updated,
            ".husky/post-checkout"
        ));
    }

    fn has_change(changes: &[BootstrapChange], kind: BootstrapChangeKind, path: &str) -> bool {
        changes
            .iter()
            .any(|change| change.kind == kind && change.path == path)
    }
}
