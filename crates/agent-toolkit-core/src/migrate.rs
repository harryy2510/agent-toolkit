use std::path::Path;

use crate::check::{check_repo, RepoIssue};
use crate::dotagent::{
    install_repo_dotagent_with_options, RepoDotAgentOptions, RepoDotAgentResult,
};
use crate::hooks::{bootstrap_repo, BootstrapChange};
use crate::intel::{write_repo_intel, RepoIntel};

pub struct RepoMigrationResult {
    pub changes: Vec<BootstrapChange>,
    pub intel: RepoIntel,
    pub issues: Vec<RepoIssue>,
}

pub struct RepoSetupResult {
    pub changes: Vec<BootstrapChange>,
    pub dotagent: RepoDotAgentResult,
    pub intel: RepoIntel,
    pub issues: Vec<RepoIssue>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RepoSetupOptions {
    pub force_dotagent: bool,
}

pub fn migrate_repo(root: &Path) -> std::io::Result<RepoMigrationResult> {
    let changes = bootstrap_repo(root)?;
    let intel = write_repo_intel(root)?;
    let issues = check_repo(root);

    Ok(RepoMigrationResult {
        changes,
        intel,
        issues,
    })
}

pub fn setup_repo(
    root: &Path,
    dotagent_repo: &Path,
    options: RepoSetupOptions,
) -> std::io::Result<RepoSetupResult> {
    let mut changes = bootstrap_repo(root)?;
    let intel = write_repo_intel(root)?;
    let dotagent = install_repo_dotagent_with_options(
        root,
        dotagent_repo,
        RepoDotAgentOptions {
            force: options.force_dotagent,
        },
    )?;
    changes.extend(dotagent.changes.clone());
    let issues = check_repo(root);

    Ok(RepoSetupResult {
        changes,
        dotagent,
        intel,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "agent-toolkit-migrate-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn migrate_repo_bootstraps_intel_and_checks_repo() {
        let root = temp_dir();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/index.ts"), "export const value = 1\n").unwrap();

        let result = migrate_repo(&root).unwrap();

        assert!(result
            .changes
            .iter()
            .any(|change| change.path == "AGENTS.md"));
        assert!(root.join(".agents/intel/summary.md").exists());
        assert_eq!(result.intel.file_count, 2);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn setup_repo_bootstraps_intel_vendors_dotagent_and_checks_repo() {
        let root = temp_dir();
        let source = temp_dir();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/index.ts"), "export const value = 1\n").unwrap();
        write_dotagent_source(&source);

        let result = setup_repo(&root, &source, RepoSetupOptions::default()).unwrap();

        assert!(result
            .changes
            .iter()
            .any(|change| change.path == "AGENTS.md"));
        assert!(root.join(".agents/intel/summary.md").exists());
        assert!(root.join(".agents/skills/toolchain/SKILL.md").exists());
        assert!(root.join(".agents/dotagent/agents/worker.md").exists());
        assert!(root.join(".agents/dotagent/commands/setup.md").exists());
        assert!(root.join(".agents/dotagent.lock.json").exists());
        assert_eq!(result.intel.file_count, 2);
        assert_eq!(result.dotagent.installed_skills, vec!["toolchain"]);
        assert!(result.issues.is_empty());
    }

    fn write_dotagent_source(root: &Path) {
        let plugin_root = root.join("plugins/dotagent");
        fs::create_dir_all(plugin_root.join("skills/toolchain")).unwrap();
        fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
        fs::create_dir_all(plugin_root.join("agents")).unwrap();
        fs::create_dir_all(plugin_root.join("commands")).unwrap();
        fs::write(plugin_root.join("AGENTS.md"), "# Shared Rules\n").unwrap();
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"dotagent","version":"1.7.0"}"#,
        )
        .unwrap();
        fs::write(
            plugin_root.join("skills/toolchain/SKILL.md"),
            "# Toolchain\n",
        )
        .unwrap();
        fs::write(plugin_root.join("agents/worker.md"), "# Worker\n").unwrap();
        fs::write(plugin_root.join("commands/setup.md"), "# Setup\n").unwrap();
    }
}
