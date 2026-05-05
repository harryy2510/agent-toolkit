use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::hooks::{BootstrapChange, BootstrapChangeKind};

const DOTAGENT_BLOCK_START: &str = "<!-- AGENT-TOOLKIT:DOTAGENT:START -->";
const DOTAGENT_BLOCK_END: &str = "<!-- AGENT-TOOLKIT:DOTAGENT:END -->";
const DOTAGENT_REPOSITORY: &str = "https://github.com/harryy2510/dotagent.git";
const DOTAGENT_PLUGIN_PATH: &str = "plugins/dotagent";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoDotAgentSkippedSkill {
    pub name: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoDotAgentResult {
    pub changes: Vec<BootstrapChange>,
    pub installed_skills: Vec<String>,
    pub revision: Option<String>,
    pub skipped_skills: Vec<RepoDotAgentSkippedSkill>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RepoDotAgentOptions {
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExistingDotAgentLock {
    revision: Option<String>,
    skills: Vec<String>,
    version: Option<String>,
}

pub fn install_repo_dotagent(
    root: &Path,
    dotagent_repo: &Path,
) -> std::io::Result<RepoDotAgentResult> {
    install_repo_dotagent_with_options(root, dotagent_repo, RepoDotAgentOptions::default())
}

pub fn install_repo_dotagent_with_options(
    root: &Path,
    dotagent_repo: &Path,
    options: RepoDotAgentOptions,
) -> std::io::Result<RepoDotAgentResult> {
    let plugin_root = dotagent_repo.join(DOTAGENT_PLUGIN_PATH);
    let rules_path = plugin_root.join("AGENTS.md");
    let skills_source = plugin_root.join("skills");

    if !rules_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} was not found", rules_path.display()),
        ));
    }
    if !skills_source.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} was not found", skills_source.display()),
        ));
    }

    let version = read_dotagent_version(&plugin_root);
    let revision = git_revision(dotagent_repo);
    if !options.force {
        if let Some(existing_lock) = existing_dotagent_lock(root) {
            if dotagent_lock_matches(&existing_lock, version.as_deref(), revision.as_deref())
                && dotagent_snapshot_exists(root, &plugin_root, &existing_lock)
            {
                return Ok(RepoDotAgentResult {
                    changes: Vec::new(),
                    installed_skills: existing_lock.skills,
                    revision,
                    skipped_skills: Vec::new(),
                    version,
                });
            }
        }
    }

    let mut changes = Vec::new();
    let rules = fs::read_to_string(rules_path)?;
    upsert_managed_block(
        root,
        "AGENTS.md",
        DOTAGENT_BLOCK_START,
        DOTAGENT_BLOCK_END,
        &managed_dotagent_block(&rules),
        &mut changes,
    )?;

    fs::create_dir_all(root.join(".agents/skills"))?;
    let managed_skills = existing_managed_skills(root);
    let mut installed_skills = Vec::new();
    let mut skipped_skills = Vec::new();

    for source in sorted_child_dirs(&skills_source)? {
        let Some(skill_name) = source
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let relative_path = format!(".agents/skills/{skill_name}");
        let destination = root.join(&relative_path);

        if destination.exists() && !managed_skills.contains(&skill_name) {
            skipped_skills.push(RepoDotAgentSkippedSkill {
                name: skill_name,
                path: relative_path,
                reason: "existing project skill is not managed by DotAgent".to_string(),
            });
            continue;
        }

        if paths_equal(&source, &destination)? {
            installed_skills.push(skill_name);
            continue;
        }

        let change_kind = if destination.exists() {
            remove_path(&destination)?;
            BootstrapChangeKind::Updated
        } else {
            BootstrapChangeKind::Created
        };
        copy_dir_recursive(&source, &destination)?;
        changes.push(BootstrapChange {
            kind: change_kind,
            path: relative_path,
        });
        installed_skills.push(skill_name);
    }

    write_lock_file(
        root,
        &installed_skills,
        version.as_deref(),
        revision.as_deref(),
        &mut changes,
    )?;
    sync_optional_reference_dir(
        root,
        &plugin_root.join("agents"),
        ".agents/dotagent/agents",
        &mut changes,
    )?;
    sync_optional_reference_dir(
        root,
        &plugin_root.join("commands"),
        ".agents/dotagent/commands",
        &mut changes,
    )?;

    Ok(RepoDotAgentResult {
        changes,
        installed_skills,
        revision,
        skipped_skills,
        version,
    })
}

fn dotagent_lock_matches(
    lock: &ExistingDotAgentLock,
    version: Option<&str>,
    revision: Option<&str>,
) -> bool {
    lock.version.as_deref() == version && lock.revision.as_deref() == revision
}

fn dotagent_snapshot_exists(root: &Path, plugin_root: &Path, lock: &ExistingDotAgentLock) -> bool {
    let Ok(agents) = fs::read_to_string(root.join("AGENTS.md")) else {
        return false;
    };
    if !agents.contains(DOTAGENT_BLOCK_START) || !agents.contains(DOTAGENT_BLOCK_END) {
        return false;
    }
    for skill in &lock.skills {
        if !root.join(".agents/skills").join(skill).exists() {
            return false;
        }
    }
    if plugin_root.join("agents").exists() && !root.join(".agents/dotagent/agents").exists() {
        return false;
    }
    if plugin_root.join("commands").exists() && !root.join(".agents/dotagent/commands").exists() {
        return false;
    }
    true
}

fn managed_dotagent_block(rules: &str) -> String {
    format!(
        "{DOTAGENT_BLOCK_START}\n## DotAgent Project Snapshot\n\n- This repository vendors DotAgent for shared team use; do not rely on a user-global DotAgent install for repo behavior.\n- DotAgent skills live in `.agents/skills/` and are bridged to supported tools with `agents sync --path .`.\n- DotAgent role profiles and command references live under `.agents/dotagent/` for agents that need file-based guidance.\n- The pinned snapshot is recorded in `.agents/dotagent.lock.json`.\n\n{}\n{DOTAGENT_BLOCK_END}\n",
        rules.trim()
    )
}

fn upsert_managed_block(
    root: &Path,
    relative_path: &str,
    start_marker: &str,
    end_marker: &str,
    block: &str,
    changes: &mut Vec<BootstrapChange>,
) -> std::io::Result<()> {
    let path = root.join(relative_path);
    let existed = path.exists();
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
        changes.push(BootstrapChange {
            kind: if existed {
                BootstrapChangeKind::Updated
            } else {
                BootstrapChangeKind::Created
            },
            path: relative_path.to_string(),
        });
    }

    Ok(())
}

fn existing_managed_skills(root: &Path) -> BTreeSet<String> {
    existing_dotagent_lock(root)
        .map(|lock| lock.skills.into_iter().collect())
        .unwrap_or_default()
}

fn existing_dotagent_lock(root: &Path) -> Option<ExistingDotAgentLock> {
    let contents = fs::read_to_string(root.join(".agents/dotagent.lock.json")).ok()?;
    let lock = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    if lock
        .get("schemaVersion")
        .and_then(|version| version.as_u64())
        != Some(1)
    {
        return None;
    }
    if lock.get("plugin").and_then(|plugin| plugin.as_str()) != Some("dotagent") {
        return None;
    }
    let skills = lock
        .get("skills")
        .and_then(|skills| skills.as_array())?
        .iter()
        .filter_map(|skill| skill.as_str())
        .map(str::to_string)
        .collect();
    Some(ExistingDotAgentLock {
        revision: lock
            .get("revision")
            .and_then(|revision| revision.as_str())
            .map(str::to_string),
        skills,
        version: lock
            .get("version")
            .and_then(|version| version.as_str())
            .map(str::to_string),
    })
}

fn sorted_child_dirs(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn paths_equal(source: &Path, destination: &Path) -> std::io::Result<bool> {
    if !destination.exists() {
        return Ok(false);
    }
    let source_metadata = fs::metadata(source)?;
    let destination_metadata = fs::metadata(destination)?;
    if source_metadata.is_file() || destination_metadata.is_file() {
        return Ok(source_metadata.is_file()
            && destination_metadata.is_file()
            && fs::read(source)? == fs::read(destination)?);
    }
    if !source_metadata.is_dir() || !destination_metadata.is_dir() {
        return Ok(false);
    }

    let source_entries = sorted_child_paths(source)?;
    let destination_entries = sorted_child_paths(destination)?;
    if source_entries.len() != destination_entries.len() {
        return Ok(false);
    }

    for (source_entry, destination_entry) in source_entries.iter().zip(destination_entries.iter()) {
        if source_entry.file_name() != destination_entry.file_name() {
            return Ok(false);
        }
        if !paths_equal(source_entry, destination_entry)? {
            return Ok(false);
        }
    }

    Ok(true)
}

fn sorted_child_paths(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root)? {
        paths.push(entry?.path());
    }
    paths.sort();
    Ok(paths)
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    if fs::metadata(path)?.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_name = entry.file_name();
        if file_name.to_string_lossy().starts_with(".env") {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(file_name);
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(source_path, destination_path)?;
        }
    }
    Ok(())
}

fn sync_optional_reference_dir(
    root: &Path,
    source: &Path,
    relative_destination: &str,
    changes: &mut Vec<BootstrapChange>,
) -> std::io::Result<()> {
    if !source.exists() {
        return Ok(());
    }
    let destination = root.join(relative_destination);
    if paths_equal(source, &destination)? {
        return Ok(());
    }
    let kind = if destination.exists() {
        remove_path(&destination)?;
        BootstrapChangeKind::Updated
    } else {
        BootstrapChangeKind::Created
    };
    copy_dir_recursive(source, &destination)?;
    changes.push(BootstrapChange {
        kind,
        path: relative_destination.to_string(),
    });
    Ok(())
}

fn read_dotagent_version(plugin_root: &Path) -> Option<String> {
    let manifest = fs::read_to_string(plugin_root.join(".codex-plugin/plugin.json")).ok()?;
    let manifest = serde_json::from_str::<serde_json::Value>(&manifest).ok()?;
    manifest
        .get("version")
        .and_then(|version| version.as_str())
        .map(str::to_string)
}

fn git_revision(repo: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let revision = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if revision.is_empty() {
        None
    } else {
        Some(revision)
    }
}

fn write_lock_file(
    root: &Path,
    installed_skills: &[String],
    version: Option<&str>,
    revision: Option<&str>,
    changes: &mut Vec<BootstrapChange>,
) -> std::io::Result<()> {
    let path = root.join(".agents/dotagent.lock.json");
    let existed = path.exists();
    let lock = serde_json::json!({
        "schemaVersion": 1,
        "plugin": "dotagent",
        "source": {
            "type": "git",
            "repository": DOTAGENT_REPOSITORY,
            "pluginPath": DOTAGENT_PLUGIN_PATH
        },
        "version": version,
        "revision": revision,
        "rulesPath": "AGENTS.md",
        "skillsPath": ".agents/skills",
        "roleProfilesPath": ".agents/dotagent/agents",
        "commandsPath": ".agents/dotagent/commands",
        "skills": installed_skills
    });
    let mut contents = to_tab_indented_json(&lock)?;
    contents.push('\n');
    if fs::read_to_string(&path).ok().as_deref() == Some(contents.as_str()) {
        return Ok(());
    }
    fs::write(path, contents)?;
    changes.push(BootstrapChange {
        kind: if existed {
            BootstrapChangeKind::Updated
        } else {
            BootstrapChangeKind::Created
        },
        path: ".agents/dotagent.lock.json".to_string(),
    });
    Ok(())
}

fn json_to_io_error(error: serde_json::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error)
}

fn to_tab_indented_json(value: &serde_json::Value) -> std::io::Result<String> {
    let spaces = serde_json::to_string_pretty(value).map_err(json_to_io_error)?;
    Ok(spaces
        .lines()
        .map(replace_json_indent_with_tabs)
        .collect::<Vec<_>>()
        .join("\n"))
}

fn replace_json_indent_with_tabs(line: &str) -> String {
    let space_count = line
        .chars()
        .take_while(|character| *character == ' ')
        .count();
    let tab_count = space_count / 2;
    let remaining_spaces = space_count % 2;
    let mut updated = String::new();
    updated.push_str(&"\t".repeat(tab_count));
    updated.push_str(&" ".repeat(remaining_spaces));
    updated.push_str(&line[space_count..]);
    updated
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_dotagent_source(root: &Path) {
        let plugin_root = root.join("plugins/dotagent");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
        fs::create_dir_all(plugin_root.join("agents")).unwrap();
        fs::create_dir_all(plugin_root.join("commands")).unwrap();
        fs::create_dir_all(plugin_root.join("skills/toolchain")).unwrap();
        fs::write(
            plugin_root.join("AGENTS.md"),
            "<!-- DOTAGENT:START -->\n# DotAgent Rules\n<!-- DOTAGENT:END -->\n",
        )
        .unwrap();
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"dotagent","version":"1.7.0"}"#,
        )
        .unwrap();
        fs::write(
            plugin_root.join("skills/toolchain/SKILL.md"),
            "---\nname: toolchain\n---\nUse Bun.\n",
        )
        .unwrap();
        fs::write(plugin_root.join("agents/worker.md"), "# Worker\n").unwrap();
        fs::write(plugin_root.join("commands/setup.md"), "# Setup\n").unwrap();
    }

    #[test]
    fn install_repo_dotagent_vendors_rules_and_skills() {
        let root = temp_dir("agent-toolkit-dotagent-root");
        let source = temp_dir("agent-toolkit-dotagent-source");
        fs::write(root.join("AGENTS.md"), "# Repo Rules\n").unwrap();
        write_dotagent_source(&source);

        let result = install_repo_dotagent(&root, &source).unwrap();
        let second_result = install_repo_dotagent(&root, &source).unwrap();

        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains(DOTAGENT_BLOCK_START));
        assert!(agents.contains("DotAgent Project Snapshot"));
        assert!(agents.contains("# DotAgent Rules"));
        assert!(root.join(".agents/skills/toolchain/SKILL.md").exists());
        assert!(root.join(".agents/dotagent/agents/worker.md").exists());
        assert!(root.join(".agents/dotagent/commands/setup.md").exists());
        let lock = fs::read_to_string(root.join(".agents/dotagent.lock.json")).unwrap();
        assert!(lock.contains("\n\t\""));
        assert!(lock.contains("\"version\": \"1.7.0\""));
        assert!(lock.contains("\"toolchain\""));
        assert!(lock.contains("\"roleProfilesPath\": \".agents/dotagent/agents\""));
        assert_eq!(result.installed_skills, vec!["toolchain"]);
        assert!(result
            .changes
            .iter()
            .any(|change| change.path == ".agents/skills/toolchain"));
        assert!(second_result.changes.is_empty());
    }

    #[test]
    fn install_repo_dotagent_uses_lock_as_normal_noop_gate() {
        let root = temp_dir("agent-toolkit-dotagent-lock-root");
        let source = temp_dir("agent-toolkit-dotagent-lock-source");
        fs::write(root.join("AGENTS.md"), "# Repo Rules\n").unwrap();
        write_dotagent_source(&source);

        install_repo_dotagent(&root, &source).unwrap();
        fs::write(
            root.join(".agents/skills/toolchain/SKILL.md"),
            "---\nname: toolchain\n---\nUse Bun, formatted by the repo.\n",
        )
        .unwrap();

        let second_result = install_repo_dotagent(&root, &source).unwrap();
        let formatted_skill =
            fs::read_to_string(root.join(".agents/skills/toolchain/SKILL.md")).unwrap();
        assert!(second_result.changes.is_empty());
        assert!(formatted_skill.contains("formatted by the repo"));

        let forced_result =
            install_repo_dotagent_with_options(&root, &source, RepoDotAgentOptions { force: true })
                .unwrap();
        let restored_skill =
            fs::read_to_string(root.join(".agents/skills/toolchain/SKILL.md")).unwrap();
        assert!(forced_result
            .changes
            .iter()
            .any(|change| change.path == ".agents/skills/toolchain"));
        assert_eq!(restored_skill, "---\nname: toolchain\n---\nUse Bun.\n");
    }

    #[test]
    fn install_repo_dotagent_does_not_overwrite_unmanaged_project_skill() {
        let root = temp_dir("agent-toolkit-dotagent-skip-root");
        let source = temp_dir("agent-toolkit-dotagent-skip-source");
        fs::create_dir_all(root.join(".agents/skills/toolchain")).unwrap();
        fs::write(
            root.join(".agents/skills/toolchain/SKILL.md"),
            "project-owned\n",
        )
        .unwrap();
        write_dotagent_source(&source);

        let result = install_repo_dotagent(&root, &source).unwrap();

        let skill = fs::read_to_string(root.join(".agents/skills/toolchain/SKILL.md")).unwrap();
        assert_eq!(skill, "project-owned\n");
        assert_eq!(result.installed_skills, Vec::<String>::new());
        assert_eq!(result.skipped_skills.len(), 1);
        assert_eq!(result.skipped_skills[0].name, "toolchain");
    }
}
