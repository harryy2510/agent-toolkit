use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct GlobalSetupResult {
    pub updated_files: Vec<PathBuf>,
    pub updated_codex_marketplaces: Vec<PathBuf>,
    pub removed_legacy_extensions: Vec<PathBuf>,
    pub unchanged_extensions: Vec<PathBuf>,
    pub linked_extensions: Vec<PathBuf>,
    pub skipped_extensions: Vec<GlobalSetupExtensionSkip>,
}

pub struct GlobalTeardownResult {
    pub updated_files: Vec<PathBuf>,
    pub updated_codex_marketplaces: Vec<PathBuf>,
    pub removed_extensions: Vec<PathBuf>,
    pub skipped_extensions: Vec<GlobalSetupExtensionSkip>,
}

pub struct GlobalSetupExtensionSkip {
    pub source: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDetection {
    pub claude: bool,
    pub codex: bool,
    pub gemini: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSetupOptions {
    pub all: bool,
    pub include_gemini: bool,
    pub detection: AgentDetection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalTeardownOptions {
    pub include_gemini: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSetupPlan {
    pub dotagent_repo: PathBuf,
    pub actions: Vec<GlobalSetupAction>,
    pub skipped: Vec<GlobalSetupSkip>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSetupAction {
    pub agent: String,
    pub description: String,
    pub kind: GlobalSetupActionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalSetupActionKind {
    ManagedRules {
        path: PathBuf,
        block_id: String,
    },
    CodexMarketplaceRegistration {
        config_path: PathBuf,
        source: PathBuf,
    },
    LegacyGeminiExtensionRemoval {
        path: PathBuf,
    },
    GeminiExtensionAlreadyLinked {
        source: PathBuf,
    },
    GeminiExtensionLink {
        source: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSetupSkip {
    pub agent: String,
    pub reason: String,
}

pub fn install_global_rules(
    home: &Path,
    dotagent_repo: &Path,
) -> std::io::Result<GlobalSetupResult> {
    let plan = build_global_setup_plan(
        home,
        dotagent_repo,
        GlobalSetupOptions {
            all: true,
            include_gemini: true,
            detection: detect_installed_agents(home),
        },
    );
    apply_global_setup_plan(&plan)
}

pub fn detect_installed_agents(home: &Path) -> AgentDetection {
    AgentDetection {
        claude: home.join(".claude").exists() || command_exists("claude"),
        codex: home.join(".codex").exists() || command_exists("codex"),
        gemini: command_exists("gemini"),
    }
}

pub fn default_global_setup_options(home: &Path) -> GlobalSetupOptions {
    GlobalSetupOptions {
        all: false,
        include_gemini: true,
        detection: detect_installed_agents(home),
    }
}

pub fn build_global_setup_plan(
    home: &Path,
    dotagent_repo: &Path,
    options: GlobalSetupOptions,
) -> GlobalSetupPlan {
    let mut actions = Vec::new();
    let mut skipped = Vec::new();
    let include_codex = options.all || options.detection.codex;

    push_managed_rules_action(
        &mut actions,
        &mut skipped,
        options.all || options.detection.claude,
        "Claude",
        home.join(".claude/CLAUDE.md"),
        "Update ~/.claude/CLAUDE.md managed rules block",
    );
    push_managed_rules_action(
        &mut actions,
        &mut skipped,
        include_codex,
        "Codex",
        home.join(".codex/AGENTS.md"),
        "Update ~/.codex/AGENTS.md managed rules block",
    );
    if include_codex {
        let marketplace = dotagent_repo.join(".agents/plugins/marketplace.json");
        if marketplace.exists() {
            actions.push(GlobalSetupAction {
                agent: "Codex".to_string(),
                description: "Register DotAgent Codex local marketplace".to_string(),
                kind: GlobalSetupActionKind::CodexMarketplaceRegistration {
                    config_path: home.join(".codex/config.toml"),
                    source: dotagent_repo.to_path_buf(),
                },
            });
        } else {
            skipped.push(GlobalSetupSkip {
                agent: "Codex".to_string(),
                reason: "DotAgent Codex marketplace source was not found".to_string(),
            });
        }
    }

    if !options.include_gemini {
        skipped.push(GlobalSetupSkip {
            agent: "Gemini".to_string(),
            reason: "disabled by --skip-gemini".to_string(),
        });
    } else if options.all || options.detection.gemini {
        let source = dotagent_repo.join("plugins/dotagent/gemini-extension");
        if source.exists() {
            let legacy_extension = home.join(".gemini/extensions/dotclaude");
            if is_legacy_dotclaude_gemini_extension(&legacy_extension) {
                actions.push(GlobalSetupAction {
                    agent: "Gemini".to_string(),
                    description: "Remove legacy DotClaude Gemini extension".to_string(),
                    kind: GlobalSetupActionKind::LegacyGeminiExtensionRemoval {
                        path: legacy_extension,
                    },
                });
            }
            let dotagent_extension = home.join(".gemini/extensions/dotagent");
            if is_dotagent_gemini_extension_linked(&dotagent_extension, &source) {
                actions.push(GlobalSetupAction {
                    agent: "Gemini".to_string(),
                    description: "DotAgent Gemini extension already linked".to_string(),
                    kind: GlobalSetupActionKind::GeminiExtensionAlreadyLinked { source },
                });
            } else {
                actions.push(GlobalSetupAction {
                    agent: "Gemini".to_string(),
                    description: "Link DotAgent Gemini extension".to_string(),
                    kind: GlobalSetupActionKind::GeminiExtensionLink { source },
                });
            }
        } else {
            skipped.push(GlobalSetupSkip {
                agent: "Gemini".to_string(),
                reason: "DotAgent Gemini extension source was not found".to_string(),
            });
        }
    } else {
        skipped.push(GlobalSetupSkip {
            agent: "Gemini".to_string(),
            reason: "Gemini CLI was not detected".to_string(),
        });
    }

    GlobalSetupPlan {
        dotagent_repo: dotagent_repo.to_path_buf(),
        actions,
        skipped,
    }
}

pub fn apply_global_setup_plan(plan: &GlobalSetupPlan) -> std::io::Result<GlobalSetupResult> {
    apply_global_setup_plan_with_gemini_command(plan, "gemini")
}

pub fn teardown_global_rules(
    home: &Path,
    options: GlobalTeardownOptions,
) -> std::io::Result<GlobalTeardownResult> {
    let mut updated_files = Vec::new();
    let mut updated_codex_marketplaces = Vec::new();
    let mut removed_extensions = Vec::new();
    let mut skipped_extensions = Vec::new();

    for path in [
        home.join(".claude/CLAUDE.md"),
        home.join(".codex/AGENTS.md"),
    ] {
        if remove_file_block(&path, "DOTAGENT")? {
            updated_files.push(path);
        }
    }

    let codex_config = home.join(".codex/config.toml");
    if remove_codex_marketplace_config(&codex_config)? {
        updated_codex_marketplaces.push(codex_config);
    }

    if options.include_gemini {
        let extension = home.join(".gemini/extensions/dotagent");
        if extension.exists() {
            if is_any_dotagent_gemini_extension_linked(&extension) {
                fs::remove_dir_all(&extension)?;
                removed_extensions.push(extension);
            } else {
                skipped_extensions.push(GlobalSetupExtensionSkip {
                    source: extension,
                    reason: "Gemini extension was not installed as a DotAgent link".to_string(),
                });
            }
        }
    }

    Ok(GlobalTeardownResult {
        updated_files,
        updated_codex_marketplaces,
        removed_extensions,
        skipped_extensions,
    })
}

fn apply_global_setup_plan_with_gemini_command(
    plan: &GlobalSetupPlan,
    gemini_command: &str,
) -> std::io::Result<GlobalSetupResult> {
    let rules = fs::read_to_string(plan.dotagent_repo.join("plugins/dotagent/AGENTS.md"))?;
    let mut updated_files = Vec::new();
    let mut updated_codex_marketplaces = Vec::new();
    let mut removed_legacy_extensions = Vec::new();
    let mut unchanged_extensions = Vec::new();
    let mut linked_extensions = Vec::new();
    let mut skipped_extensions = Vec::new();

    for action in &plan.actions {
        match &action.kind {
            GlobalSetupActionKind::ManagedRules { path, block_id } => {
                upsert_file_block(path, block_id, &rules)?;
                updated_files.push(path.clone());
            }
            GlobalSetupActionKind::CodexMarketplaceRegistration {
                config_path,
                source,
            } => {
                upsert_codex_marketplace_config(config_path, source)?;
                updated_codex_marketplaces.push(config_path.clone());
            }
            GlobalSetupActionKind::LegacyGeminiExtensionRemoval { path } => {
                if path.exists() {
                    fs::remove_dir_all(path)?;
                    removed_legacy_extensions.push(path.clone());
                }
            }
            GlobalSetupActionKind::GeminiExtensionAlreadyLinked { source } => {
                unchanged_extensions.push(source.clone());
            }
            GlobalSetupActionKind::GeminiExtensionLink { source } => {
                match link_gemini_extension(gemini_command, source) {
                    Ok(true) => linked_extensions.push(source.clone()),
                    Ok(false) => skipped_extensions.push(GlobalSetupExtensionSkip {
                        source: source.clone(),
                        reason: "gemini extensions link exited unsuccessfully".to_string(),
                    }),
                    Err(error) => skipped_extensions.push(GlobalSetupExtensionSkip {
                        source: source.clone(),
                        reason: format!("failed to run gemini: {error}"),
                    }),
                }
            }
        }
    }

    Ok(GlobalSetupResult {
        updated_files,
        updated_codex_marketplaces,
        removed_legacy_extensions,
        unchanged_extensions,
        linked_extensions,
        skipped_extensions,
    })
}

fn is_legacy_dotclaude_gemini_extension(path: &Path) -> bool {
    let install_metadata = path.join(".gemini-extension-install.json");
    let Ok(contents) = fs::read_to_string(install_metadata) else {
        return false;
    };
    let normalized_contents = normalize_path_separators(&contents);

    contents.contains("\"type\": \"link\"")
        && normalized_contents.contains("/dotclaude/")
        && normalized_contents.contains("plugins/dotclaude/gemini-extension")
}

fn is_dotagent_gemini_extension_linked(path: &Path, source: &Path) -> bool {
    let install_metadata = path.join(".gemini-extension-install.json");
    let Ok(contents) = fs::read_to_string(install_metadata) else {
        return false;
    };

    contents.contains("\"type\": \"link\"") && contents_references_path(&contents, source)
}

fn is_any_dotagent_gemini_extension_linked(path: &Path) -> bool {
    let install_metadata = path.join(".gemini-extension-install.json");
    let Ok(contents) = fs::read_to_string(install_metadata) else {
        return false;
    };
    let normalized_contents = normalize_path_separators(&contents);

    contents.contains("\"type\": \"link\"")
        && normalized_contents.contains("plugins/dotagent/gemini-extension")
}

fn contents_references_path(contents: &str, path: &Path) -> bool {
    let path = path.to_string_lossy();
    let normalized_path = normalize_path_separators(&path);
    let escaped_path = path.replace('\\', "\\\\");
    let normalized_contents = normalize_path_separators(contents);

    contents.contains(&path[..])
        || contents.contains(&escaped_path)
        || normalized_contents.contains(&normalized_path)
}

fn normalize_path_separators(value: &str) -> String {
    value.replace("\\\\", "/").replace('\\', "/")
}

pub fn upsert_managed_block(existing: &str, id: &str, content: &str) -> String {
    let start = start_marker(id);
    let end = end_marker(id);
    let block = format!("{start}\n{}\n{end}", content.trim());

    if let Some(start_index) = existing.find(&start) {
        if let Some(relative_end_index) = existing[start_index..].find(&end) {
            let end_index = start_index + relative_end_index + end.len();
            let mut result = String::new();
            result.push_str(existing[..start_index].trim_end());
            result.push_str("\n\n");
            result.push_str(&block);
            result.push_str(existing[end_index..].trim_end_matches('\n'));
            result.push('\n');
            return result;
        }
    }

    let mut result = existing.trim_end().to_string();
    if !result.is_empty() {
        result.push_str("\n\n");
    }
    result.push_str(&block);
    result.push('\n');
    result
}

pub fn remove_managed_block(existing: &str, id: &str) -> String {
    let start = start_marker(id);
    let end = end_marker(id);

    let Some(start_index) = existing.find(&start) else {
        return existing.to_string();
    };
    let Some(relative_end_index) = existing[start_index..].find(&end) else {
        return existing.to_string();
    };
    let end_index = start_index + relative_end_index + end.len();
    join_removed_block_parts(&existing[..start_index], &existing[end_index..])
}

fn join_removed_block_parts(before: &str, after: &str) -> String {
    let before = before.trim_end();
    let after = after.trim_start_matches('\n').trim_end();
    if before.is_empty() && after.is_empty() {
        return String::new();
    }
    if before.is_empty() {
        return format!("{after}\n");
    }
    if after.is_empty() {
        return format!("{before}\n");
    }
    format!("{before}\n\n{after}\n")
}

fn upsert_file_block(path: &Path, id: &str, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(path).unwrap_or_default();
    remove_broken_symlink(path)?;
    fs::write(path, upsert_managed_block(&existing, id, content))
}

fn remove_file_block(path: &Path, id: &str) -> std::io::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let existing = fs::read_to_string(path)?;
    let updated = remove_managed_block(&existing, id);
    if updated == existing {
        return Ok(false);
    }
    remove_broken_symlink(path)?;
    fs::write(path, updated)?;
    Ok(true)
}

fn remove_broken_symlink(path: &Path) -> std::io::Result<()> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };

    if metadata.file_type().is_symlink() && fs::metadata(path).is_err() {
        fs::remove_file(path)?;
    }

    Ok(())
}

fn upsert_codex_marketplace_config(path: &Path, source: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let existing = fs::read_to_string(path).unwrap_or_default();
    remove_broken_symlink(path)?;
    fs::write(path, upsert_codex_marketplace(&existing, source))
}

fn remove_codex_marketplace_config(path: &Path) -> std::io::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let existing = fs::read_to_string(path)?;
    let updated = remove_codex_marketplace(&existing);
    if updated == existing {
        return Ok(false);
    }
    remove_broken_symlink(path)?;
    fs::write(path, updated)?;
    Ok(true)
}

pub fn upsert_codex_marketplace(existing: &str, source: &Path) -> String {
    let mut output = String::new();
    let mut skipping_dotagent = false;

    for line in existing.lines() {
        if line.trim() == "[marketplaces.dotagent]" {
            skipping_dotagent = true;
            continue;
        }

        if skipping_dotagent && line.trim_start().starts_with('[') {
            skipping_dotagent = false;
        }

        if !skipping_dotagent {
            output.push_str(line);
            output.push('\n');
        }
    }

    let source = source
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let trimmed = output.trim_end();
    let mut result = String::new();
    if !trimmed.is_empty() {
        result.push_str(trimmed);
        result.push_str("\n\n");
    }
    result.push_str("[marketplaces.dotagent]\n");
    result.push_str("source_type = \"local\"\n");
    result.push_str(&format!("source = \"{source}\"\n"));
    result
}

pub fn remove_codex_marketplace(existing: &str) -> String {
    let mut output = String::new();
    let mut skipping_dotagent = false;

    for line in existing.lines() {
        if line.trim() == "[marketplaces.dotagent]" {
            skipping_dotagent = true;
            continue;
        }

        if skipping_dotagent && line.trim_start().starts_with('[') {
            skipping_dotagent = false;
        }

        if !skipping_dotagent {
            output.push_str(line);
            output.push('\n');
        }
    }

    let trimmed = output.trim_end();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

fn push_managed_rules_action(
    actions: &mut Vec<GlobalSetupAction>,
    skipped: &mut Vec<GlobalSetupSkip>,
    include: bool,
    agent: &str,
    path: PathBuf,
    description: &str,
) {
    if include {
        actions.push(GlobalSetupAction {
            agent: agent.to_string(),
            description: description.to_string(),
            kind: GlobalSetupActionKind::ManagedRules {
                path,
                block_id: "DOTAGENT".to_string(),
            },
        });
    } else {
        skipped.push(GlobalSetupSkip {
            agent: agent.to_string(),
            reason: format!("{} was not detected", agent),
        });
    }
}

fn start_marker(id: &str) -> String {
    format!("<!-- AGENT-TOOLKIT:{id}:START -->")
}

fn end_marker(id: &str) -> String {
    format!("<!-- AGENT-TOOLKIT:{id}:END -->")
}

fn link_gemini_extension(command: &str, source: &Path) -> std::io::Result<bool> {
    let status = Command::new(command)
        .args(["extensions", "link"])
        .arg(source)
        .arg("--consent")
        .status()?;

    Ok(status.success())
}

fn command_exists(command: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&paths).any(|directory| {
        let candidate = directory.join(command);
        if candidate.is_file() {
            return true;
        }

        if cfg!(windows) {
            return ["exe", "cmd", "bat"]
                .iter()
                .any(|extension| directory.join(format!("{command}.{extension}")).is_file());
        }

        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_managed_block_preserves_user_content() {
        let result = upsert_managed_block("# User Rules\n\nKeep this.\n", "DOTAGENT", "# Shared\n");

        assert!(result.contains("# User Rules"));
        assert!(result.contains("Keep this."));
        assert!(result.contains("AGENT-TOOLKIT:DOTAGENT:START"));
        assert!(result.contains("# Shared"));
    }

    #[test]
    fn upsert_managed_block_replaces_existing_block() {
        let first = upsert_managed_block("prefix\n", "DOTAGENT", "old");
        let result = upsert_managed_block(&first, "DOTAGENT", "new");

        assert!(result.contains("prefix"));
        assert!(result.contains("new"));
        assert!(!result.contains("old"));
        assert_eq!(result.matches("AGENT-TOOLKIT:DOTAGENT:START").count(), 1);
    }

    #[test]
    fn upsert_codex_marketplace_replaces_dotagent_table() {
        let result = upsert_codex_marketplace(
            r#"approval_policy = "on-request"

[marketplaces.dotagent]
source_type = "local"
source = "/old"

[marketplaces.other]
source_type = "local"
source = "/other"
"#,
            Path::new("/new/dotagent"),
        );

        assert!(result.contains("approval_policy = \"on-request\""));
        assert!(result.contains("[marketplaces.other]"));
        assert!(result.contains("[marketplaces.dotagent]"));
        assert!(result.contains("source = \"/new/dotagent\""));
        assert!(!result.contains("/old"));
        assert_eq!(result.matches("[marketplaces.dotagent]").count(), 1);
    }

    #[test]
    fn upsert_codex_marketplace_preserves_table_after_indented_header() {
        let result = upsert_codex_marketplace(
            r#"approval_policy = "on-request"

[marketplaces.dotagent]
source_type = "local"
source = "/old"

  [projects."/repo"]
trust_level = "trusted"
"#,
            Path::new("/new/dotagent"),
        );

        assert!(result.contains("[projects.\"/repo\"]"));
        assert!(result.contains("trust_level = \"trusted\""));
        assert!(!result.contains("/old"));
    }

    #[test]
    fn remove_managed_block_preserves_user_content() {
        let existing =
            upsert_managed_block("# User Rules\n\nKeep this.\n", "DOTAGENT", "# Shared\n");
        let result = remove_managed_block(&existing, "DOTAGENT");

        assert!(result.contains("# User Rules"));
        assert!(result.contains("Keep this."));
        assert!(!result.contains("AGENT-TOOLKIT:DOTAGENT:START"));
        assert!(!result.contains("# Shared"));
    }

    #[test]
    fn remove_codex_marketplace_preserves_other_tables() {
        let existing = upsert_codex_marketplace(
            r#"approval_policy = "on-request"

[marketplaces.other]
source_type = "local"
source = "/other"
"#,
            Path::new("/dotagent"),
        );
        let result = remove_codex_marketplace(&existing);

        assert!(result.contains("approval_policy = \"on-request\""));
        assert!(result.contains("[marketplaces.other]"));
        assert!(!result.contains("[marketplaces.dotagent]"));
        assert!(!result.contains("/dotagent"));
    }

    #[test]
    fn teardown_global_rules_removes_only_managed_global_artifacts() {
        let root = temp_dir("agent-toolkit-global-teardown");
        let home = root.join("home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(home.join(".gemini/extensions/dotagent")).unwrap();
        fs::write(
            home.join(".claude/CLAUDE.md"),
            upsert_managed_block("# My Claude Rules\n", "DOTAGENT", "# Shared\n"),
        )
        .unwrap();
        fs::write(
            home.join(".codex/AGENTS.md"),
            upsert_managed_block("# My Codex Rules\n", "DOTAGENT", "# Shared\n"),
        )
        .unwrap();
        fs::write(
            home.join(".codex/config.toml"),
            upsert_codex_marketplace(
                "approval_policy = \"on-request\"\n",
                Path::new("/Users/example/.agent-toolkit/plugins/dotagent"),
            ),
        )
        .unwrap();
        fs::write(
            home.join(".gemini/extensions/dotagent/.gemini-extension-install.json"),
            r#"{
  "source": "/Users/example/.agent-toolkit/plugins/dotagent/plugins/dotagent/gemini-extension",
  "type": "link"
}"#,
        )
        .unwrap();

        let result = teardown_global_rules(
            &home,
            GlobalTeardownOptions {
                include_gemini: true,
            },
        )
        .unwrap();
        let claude = fs::read_to_string(home.join(".claude/CLAUDE.md")).unwrap();
        let codex = fs::read_to_string(home.join(".codex/AGENTS.md")).unwrap();
        let config = fs::read_to_string(home.join(".codex/config.toml")).unwrap();

        assert_eq!(
            result.updated_files,
            vec![
                home.join(".claude/CLAUDE.md"),
                home.join(".codex/AGENTS.md")
            ]
        );
        assert_eq!(
            result.updated_codex_marketplaces,
            vec![home.join(".codex/config.toml")]
        );
        assert_eq!(
            result.removed_extensions,
            vec![home.join(".gemini/extensions/dotagent")]
        );
        assert!(claude.contains("# My Claude Rules"));
        assert!(!claude.contains("AGENT-TOOLKIT:DOTAGENT"));
        assert!(codex.contains("# My Codex Rules"));
        assert!(!codex.contains("AGENT-TOOLKIT:DOTAGENT"));
        assert!(config.contains("approval_policy = \"on-request\""));
        assert!(!config.contains("[marketplaces.dotagent]"));
        assert!(!home.join(".gemini/extensions/dotagent").exists());
    }

    #[test]
    fn apply_global_setup_plan_registers_codex_dotagent_marketplace() {
        let root = temp_dir("agent-toolkit-global-codex-marketplace");
        let dotagent = root.join("dotagent");
        fs::create_dir_all(dotagent.join("plugins/dotagent")).unwrap();
        fs::create_dir_all(dotagent.join(".agents/plugins")).unwrap();
        fs::write(
            dotagent.join("plugins/dotagent/AGENTS.md"),
            "# Shared Rules\n",
        )
        .unwrap();
        fs::write(
            dotagent.join(".agents/plugins/marketplace.json"),
            r#"{"name":"dotagent","plugins":[]}"#,
        )
        .unwrap();
        let home = root.join("home");
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::write(
            home.join(".codex/config.toml"),
            "approval_policy = \"on-request\"\n",
        )
        .unwrap();

        let plan = build_global_setup_plan(
            &home,
            &dotagent,
            GlobalSetupOptions {
                all: false,
                include_gemini: false,
                detection: AgentDetection {
                    claude: false,
                    codex: true,
                    gemini: false,
                },
            },
        );

        assert!(plan.actions.iter().any(|action| matches!(
            action.kind,
            GlobalSetupActionKind::CodexMarketplaceRegistration { .. }
        )));

        let result = apply_global_setup_plan(&plan).unwrap();
        let config = fs::read_to_string(home.join(".codex/config.toml")).unwrap();

        assert_eq!(result.updated_files, vec![home.join(".codex/AGENTS.md")]);
        assert_eq!(
            result.updated_codex_marketplaces,
            vec![home.join(".codex/config.toml")]
        );
        assert!(config.contains("[marketplaces.dotagent]"));
        assert!(config.contains("source_type = \"local\""));
        assert!(config.contains(&format!("source = \"{}\"", dotagent.display())));
    }

    #[test]
    fn build_global_setup_plan_only_targets_detected_agents() {
        let root = temp_dir("agent-toolkit-global-plan");
        let dotagent = root.join("dotagent");
        fs::create_dir_all(dotagent.join("plugins/dotagent/gemini-extension")).unwrap();
        let plan = build_global_setup_plan(
            &root,
            &dotagent,
            GlobalSetupOptions {
                all: false,
                include_gemini: true,
                detection: AgentDetection {
                    claude: true,
                    codex: false,
                    gemini: true,
                },
            },
        );

        assert_eq!(plan.actions.len(), 2);
        assert!(plan.actions.iter().any(|action| action.agent == "Claude"));
        assert!(plan.actions.iter().any(|action| action.agent == "Gemini"));
        assert!(plan.skipped.iter().any(|skip| skip.agent == "Codex"));
    }

    #[test]
    fn apply_global_setup_plan_removes_legacy_dotclaude_gemini_extension() {
        let root = temp_dir("agent-toolkit-global-legacy-gemini");
        let dotagent = root.join("dotagent/plugins/dotagent");
        fs::create_dir_all(dotagent.join("gemini-extension")).unwrap();
        fs::write(dotagent.join("AGENTS.md"), "# Shared Rules\n").unwrap();
        let home = root.join("home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        let legacy_extension = home.join(".gemini/extensions/dotclaude");
        fs::create_dir_all(&legacy_extension).unwrap();
        fs::write(
            legacy_extension.join(".gemini-extension-install.json"),
            r#"{
  "source": "/Users/example/dotclaude/plugins/dotclaude/gemini-extension",
  "type": "link"
}"#,
        )
        .unwrap();
        let plan = build_global_setup_plan(
            &home,
            &root.join("dotagent"),
            GlobalSetupOptions {
                all: false,
                include_gemini: true,
                detection: AgentDetection {
                    claude: true,
                    codex: false,
                    gemini: true,
                },
            },
        );

        assert!(plan.actions.iter().any(|action| matches!(
            action.kind,
            GlobalSetupActionKind::LegacyGeminiExtensionRemoval { .. }
        )));

        let result = apply_global_setup_plan_with_gemini_command(
            &plan,
            "agent-toolkit-definitely-missing-gemini",
        )
        .unwrap();

        assert_eq!(result.removed_legacy_extensions, vec![legacy_extension]);
        assert!(!home.join(".gemini/extensions/dotclaude").exists());
        assert_eq!(result.skipped_extensions.len(), 1);
    }

    #[test]
    fn apply_global_setup_plan_skips_already_linked_dotagent_gemini_extension() {
        let root = temp_dir("agent-toolkit-global-linked-gemini");
        let dotagent = root.join("dotagent/plugins/dotagent");
        let source = dotagent.join("gemini-extension");
        fs::create_dir_all(&source).unwrap();
        fs::write(dotagent.join("AGENTS.md"), "# Shared Rules\n").unwrap();
        let home = root.join("home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        let linked_extension = home.join(".gemini/extensions/dotagent");
        fs::create_dir_all(&linked_extension).unwrap();
        fs::write(
            linked_extension.join(".gemini-extension-install.json"),
            format!(
                "{{\n  \"source\": \"{}\",\n  \"type\": \"link\"\n}}",
                source.display().to_string().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        let plan = build_global_setup_plan(
            &home,
            &root.join("dotagent"),
            GlobalSetupOptions {
                all: false,
                include_gemini: true,
                detection: AgentDetection {
                    claude: true,
                    codex: false,
                    gemini: true,
                },
            },
        );

        assert!(plan.actions.iter().any(|action| matches!(
            action.kind,
            GlobalSetupActionKind::GeminiExtensionAlreadyLinked { .. }
        )));
        assert!(!plan.actions.iter().any(|action| matches!(
            action.kind,
            GlobalSetupActionKind::GeminiExtensionLink { .. }
        )));

        let result = apply_global_setup_plan_with_gemini_command(
            &plan,
            "agent-toolkit-definitely-missing-gemini",
        )
        .unwrap();

        assert_eq!(result.unchanged_extensions, vec![source]);
        assert!(result.skipped_extensions.is_empty());
    }

    #[test]
    fn apply_global_setup_plan_preserves_user_rules_with_managed_blocks() {
        let root = temp_dir("agent-toolkit-global-apply");
        let dotagent = root.join("dotagent/plugins/dotagent");
        fs::create_dir_all(&dotagent).unwrap();
        fs::write(dotagent.join("AGENTS.md"), "# Shared Rules\n").unwrap();
        let home = root.join("home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::write(home.join(".claude/CLAUDE.md"), "# My Existing Rules\n").unwrap();
        let plan = build_global_setup_plan(
            &home,
            &root.join("dotagent"),
            GlobalSetupOptions {
                all: false,
                include_gemini: false,
                detection: AgentDetection {
                    claude: true,
                    codex: false,
                    gemini: false,
                },
            },
        );

        let result = apply_global_setup_plan(&plan).unwrap();
        let claude = fs::read_to_string(home.join(".claude/CLAUDE.md")).unwrap();

        assert_eq!(result.updated_files, vec![home.join(".claude/CLAUDE.md")]);
        assert!(result.linked_extensions.is_empty());
        assert!(result.skipped_extensions.is_empty());
        assert!(claude.contains("# My Existing Rules"));
        assert!(claude.contains("# Shared Rules"));
        assert!(claude.contains("AGENT-TOOLKIT:DOTAGENT:START"));
    }

    #[test]
    fn apply_global_setup_plan_skips_missing_gemini_command() {
        let root = temp_dir("agent-toolkit-global-gemini-missing");
        let dotagent = root.join("dotagent/plugins/dotagent");
        fs::create_dir_all(dotagent.join("gemini-extension")).unwrap();
        fs::write(dotagent.join("AGENTS.md"), "# Shared Rules\n").unwrap();
        let home = root.join("home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        let plan = build_global_setup_plan(
            &home,
            &root.join("dotagent"),
            GlobalSetupOptions {
                all: false,
                include_gemini: true,
                detection: AgentDetection {
                    claude: true,
                    codex: false,
                    gemini: true,
                },
            },
        );

        let result = apply_global_setup_plan_with_gemini_command(
            &plan,
            "agent-toolkit-definitely-missing-gemini",
        )
        .unwrap();
        let claude = fs::read_to_string(home.join(".claude/CLAUDE.md")).unwrap();

        assert_eq!(result.updated_files, vec![home.join(".claude/CLAUDE.md")]);
        assert!(result.linked_extensions.is_empty());
        assert_eq!(result.skipped_extensions.len(), 1);
        assert!(result.skipped_extensions[0]
            .reason
            .contains("failed to run gemini"));
        assert!(claude.contains("# Shared Rules"));
    }

    #[cfg(unix)]
    #[test]
    fn apply_global_setup_plan_replaces_broken_managed_file_symlink() {
        let root = temp_dir("agent-toolkit-global-broken-symlink");
        let dotagent = root.join("dotagent/plugins/dotagent");
        fs::create_dir_all(&dotagent).unwrap();
        fs::write(dotagent.join("AGENTS.md"), "# Shared Rules\n").unwrap();
        let home = root.join("home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        let managed_path = home.join(".claude/CLAUDE.md");
        std::os::unix::fs::symlink(root.join("missing/AGENTS.md"), &managed_path).unwrap();
        let plan = build_global_setup_plan(
            &home,
            &root.join("dotagent"),
            GlobalSetupOptions {
                all: false,
                include_gemini: false,
                detection: AgentDetection {
                    claude: true,
                    codex: false,
                    gemini: false,
                },
            },
        );

        let result = apply_global_setup_plan(&plan).unwrap();
        let claude = fs::read_to_string(&managed_path).unwrap();

        assert_eq!(result.updated_files, vec![managed_path]);
        assert!(claude.contains("# Shared Rules"));
    }

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "{prefix}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
