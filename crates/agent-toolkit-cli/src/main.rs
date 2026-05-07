use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};

use agent_toolkit_core::check::{check_repo, is_conventional_commit};
use agent_toolkit_core::dotagent::{install_repo_dotagent_with_options, RepoDotAgentOptions};
use agent_toolkit_core::fleet::discover_git_repos;
use agent_toolkit_core::global_setup::{
    apply_global_setup_plan, build_global_setup_plan, default_global_setup_options,
    teardown_global_rules, GlobalSetupActionKind, GlobalTeardownOptions,
};
use agent_toolkit_core::hooks::bootstrap_repo;
use agent_toolkit_core::intel::write_repo_intel;
use agent_toolkit_core::migrate::{migrate_repo, setup_repo, RepoSetupOptions};
use agent_toolkit_core::supabase::{db_lint_script, staged_db_lint_needed};

const DEFAULT_SUPABASE_DB_LINT_ARGS: [&str; 9] = [
    "db",
    "lint",
    "--local",
    "--schema",
    "public",
    "--level",
    "warning",
    "--fail-on",
    "warning",
];

fn main() {
    if let Err(error) = run() {
        eprintln!("agent-toolkit: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            print_help();
            Ok(())
        }
        [flag] if flag == "--help" || flag == "-h" => {
            print_help();
            Ok(())
        }
        [command] if command == "repo-intel" => {
            let root = env::current_dir().map_err(|error| error.to_string())?;
            let intel = write_repo_intel(&root).map_err(|error| error.to_string())?;
            println!(
                "wrote repo intelligence wiki to .agents/intel for {} source files",
                intel.file_count
            );
            print!("{}", intel.summary_markdown);
            Ok(())
        }
        [scope, command] if scope == "repo" && command == "intel" => {
            let root = env::current_dir().map_err(|error| error.to_string())?;
            let intel = write_repo_intel(&root).map_err(|error| error.to_string())?;
            println!(
                "wrote repo intelligence wiki to .agents/intel for {} source files",
                intel.file_count
            );
            print!("{}", intel.summary_markdown);
            Ok(())
        }
        [scope, command, flags @ ..] if scope == "repo" && command == "check" => {
            let root = env::current_dir().map_err(|error| error.to_string())?;
            let issues = check_repo(&root);
            if !issues.is_empty() {
                for issue in issues {
                    println!("{:?}: {}", issue.code, issue.message);
                }
                return Err("repo check failed".to_string());
            }

            if flags.iter().any(|flag| flag.as_str() == "--staged") && staged_db_lint_needed(&root)
            {
                run_supabase_db_lint(&root)?;
            }

            println!("agent-toolkit repo check passed");
            Ok(())
        }
        [scope, command] if scope == "repo" && command == "bootstrap" => {
            let root = env::current_dir().map_err(|error| error.to_string())?;
            let changes = bootstrap_repo(&root).map_err(|error| error.to_string())?;
            for change in changes {
                println!("{} {}", change.verb(), change.path);
            }
            Ok(())
        }
        [scope, command, flags @ ..]
            if scope == "repo" && (command == "setup" || command == "migrate") =>
        {
            run_repo_setup(command, flags)
        }
        [scope, command, flags @ ..] if scope == "repo" && command == "dotagent" => {
            let options = parse_repo_dotagent_args(flags)?;
            let root = env::current_dir().map_err(|error| error.to_string())?;
            let home = home_dir()?;
            let explicit_dotagent_source = options.dotagent_source.is_some();
            let dotagent_repo = options
                .dotagent_source
                .unwrap_or_else(|| home.join(".agent-toolkit/plugins/dotagent"));
            if !dotagent_repo.join("plugins/dotagent/AGENTS.md").exists() {
                if explicit_dotagent_source {
                    return Err(format!(
                        "DotAgent source was not found at {}",
                        dotagent_repo.display()
                    ));
                }
                ensure_dotagent_repo(&dotagent_repo)?;
            }
            let result = install_repo_dotagent_with_options(
                &root,
                &dotagent_repo,
                RepoDotAgentOptions {
                    force: options.force,
                },
            )
            .map_err(|error| error.to_string())?;
            for change in result.changes {
                println!("{} {}", change.verb(), change.path);
            }
            if let Some(version) = result.version {
                println!("installed DotAgent {version}");
            } else {
                println!("installed DotAgent");
            }
            println!("vendored {} skills", result.installed_skills.len());
            for skipped in result.skipped_skills {
                println!(
                    "skipped {}: {} ({})",
                    skipped.name, skipped.reason, skipped.path
                );
            }
            Ok(())
        }
        [scope, command, flags @ ..] if scope == "repo" && command == "sync" => {
            let options = parse_sync_args(flags)?;
            if !options.roots.is_empty() {
                return Err(
                    "repo sync does not accept path arguments; run it from the repo root"
                        .to_string(),
                );
            }
            let root = env::current_dir().map_err(|error| error.to_string())?;
            run_agents_sync(&root, options.check)
        }
        [command, rest @ ..] if command == "setup" => run_setup(rest),
        [command, rest @ ..] if command == "update" => run_update(rest),
        [command, rest @ ..] if command == "teardown" => run_teardown(rest),
        [scope, command, roots @ ..] if scope == "fleet" && command == "scan" => {
            let paths = if roots.is_empty() {
                vec![env::current_dir().map_err(|error| error.to_string())?]
            } else {
                roots.iter().map(PathBuf::from).collect()
            };
            let repos = discover_git_repos(&paths).map_err(|error| error.to_string())?;
            for repo in repos {
                println!("{}", repo.display());
            }
            Ok(())
        }
        [scope, command, roots @ ..] if scope == "fleet" && command == "bootstrap" => {
            let paths = if roots.is_empty() {
                vec![env::current_dir().map_err(|error| error.to_string())?]
            } else {
                roots.iter().map(PathBuf::from).collect()
            };
            let repos = discover_git_repos(&paths).map_err(|error| error.to_string())?;
            for repo in repos {
                let changes = bootstrap_repo(&repo).map_err(|error| error.to_string())?;
                if changes.is_empty() {
                    println!("UNCHANGED {}", repo.display());
                } else {
                    println!("BOOTSTRAPPED {}", repo.display());
                    for change in changes {
                        println!("  {} {}", change.verb(), change.path);
                    }
                }
            }
            Ok(())
        }
        [scope, command, roots @ ..] if scope == "fleet" && command == "migrate" => {
            let paths = if roots.is_empty() {
                vec![env::current_dir().map_err(|error| error.to_string())?]
            } else {
                roots.iter().map(PathBuf::from).collect()
            };
            let repos = discover_git_repos(&paths).map_err(|error| error.to_string())?;
            let mut failed = false;
            for repo in repos {
                let result = migrate_repo(&repo).map_err(|error| error.to_string())?;
                if result.issues.is_empty() {
                    println!(
                        "MIGRATED {} ({} source files)",
                        repo.display(),
                        result.intel.file_count
                    );
                    for change in result.changes {
                        println!("  {} {}", change.verb(), change.path);
                    }
                } else {
                    failed = true;
                    println!("FAIL {}", repo.display());
                    for issue in result.issues {
                        println!("  {:?}: {}", issue.code, issue.message);
                    }
                }
            }
            if failed {
                Err("fleet migrate finished with check failures".to_string())
            } else {
                Ok(())
            }
        }
        [scope, command, args @ ..] if scope == "fleet" && command == "sync" => {
            let options = parse_sync_args(args)?;
            let paths = if options.roots.is_empty() {
                vec![env::current_dir().map_err(|error| error.to_string())?]
            } else {
                options.roots
            };
            let repos = discover_git_repos(&paths).map_err(|error| error.to_string())?;
            let mut failed = false;
            for repo in repos {
                match run_agents_sync(&repo, options.check) {
                    Ok(()) => println!("SYNC {}", repo.display()),
                    Err(error) => {
                        failed = true;
                        println!("FAIL {}: {error}", repo.display());
                    }
                }
            }
            if failed {
                Err("fleet sync failed".to_string())
            } else {
                Ok(())
            }
        }
        [scope, command, roots @ ..] if scope == "fleet" && command == "check" => {
            let paths = if roots.is_empty() {
                vec![env::current_dir().map_err(|error| error.to_string())?]
            } else {
                roots.iter().map(PathBuf::from).collect()
            };
            let repos = discover_git_repos(&paths).map_err(|error| error.to_string())?;
            let mut failed = false;
            for repo in repos {
                let issues = check_repo(&repo);
                if issues.is_empty() {
                    println!("PASS {}", repo.display());
                } else {
                    failed = true;
                    println!("FAIL {}", repo.display());
                    for issue in issues {
                        println!("  {:?}: {}", issue.code, issue.message);
                    }
                }
            }
            if failed {
                Err("fleet check failed".to_string())
            } else {
                Ok(())
            }
        }
        [command, message_file] if command == "commit-msg" => {
            let message =
                std::fs::read_to_string(message_file).map_err(|error| error.to_string())?;
            if is_conventional_commit(&message) {
                Ok(())
            } else {
                Err("commit message must use Conventional Commit format, for example feat: add repo intelligence".to_string())
            }
        }
        _ => Err(format!("unknown command: {}", args.join(" "))),
    }
}

fn print_help() {
    println!(
		"agent-toolkit\n\nCommands:\n  setup [flags]       Install personal managed agent rules\n  update [flags]      Update DotAgent source and reapply personal managed rules\n  teardown [flags]    Remove personal managed agent rules\n  repo setup [flags]  Bootstrap, write repo intelligence, install DotAgent, sync, and check\n  repo migrate [flags] Alias for repo setup\n  repo intel          Build repository intelligence wiki\n  repo check          Run agent/tooling enforcement checks\n  repo bootstrap      Add AGENTS.md, .agents config, and git hooks\n  repo dotagent [flags] Pin DotAgent rules and skills into the current repo\n  repo sync [--check] Run agents sync for the current repo\n  repo-intel          Alias for repo intel\n  fleet scan [dir]    Find git repositories\n  fleet check [dir]   Run repo checks across discovered git repositories\n  fleet bootstrap     Bootstrap every discovered git repository\n  fleet migrate       Migrate every discovered git repository\n  fleet sync          Run agents sync across discovered git repositories\n  commit-msg <file>   Validate Conventional Commit message\n\nSetup/update flags:\n  --dry-run                  Print the setup plan without changing files\n  --yes, -y                  Apply without an interactive confirmation\n  --all                     Configure all supported agents\n  --skip-gemini             Do not link the Gemini extension\n  --dotagent-source <path>  Use an existing local DotAgent checkout\n\nTeardown flags:\n  --dry-run                  Print the teardown plan without changing files\n  --yes, -y                  Apply without an interactive confirmation\n  --skip-gemini             Do not remove the Gemini extension link\n\nRepo setup flags:\n  --dotagent-source <path>  Use an existing local DotAgent checkout\n  --force                   Recopy DotAgent files even when the locked version/revision matches\n  --skip-sync               Do not run agents sync after writing repo files\n\nRepo DotAgent flags:\n  --dotagent-source <path>  Use an existing local DotAgent checkout\n  --force                   Recopy DotAgent files even when the locked version/revision matches"
	);
}

fn run_supabase_db_lint(root: &Path) -> Result<(), String> {
    if let Some(script) = db_lint_script(root) {
        println!("running Supabase db lint via bun {script}");
        return run_check_command(root, "bun", &["run", &script])
            .map_err(CheckCommandError::into_message);
    }

    match run_check_command(root, "supabase", &DEFAULT_SUPABASE_DB_LINT_ARGS) {
        Ok(()) => Ok(()),
        Err(CheckCommandError::NotFound(_)) => {
            let mut args = vec!["supabase"];
            args.extend(DEFAULT_SUPABASE_DB_LINT_ARGS);
            run_check_command(root, "bunx", &args).map_err(|error| match error {
                CheckCommandError::NotFound(_) => {
                    "Supabase project has staged database changes, but no db lint runner was found. Add a db:lint package script or install the Supabase CLI.".to_string()
                }
                error => error.into_message(),
            })
        }
        Err(error) => Err(error.into_message()),
    }
}

#[derive(Debug)]
enum CheckCommandError {
    NotFound(String),
    Failed(String),
}

impl CheckCommandError {
    fn into_message(self) -> String {
        match self {
            Self::NotFound(program) => format!("{program} not found"),
            Self::Failed(message) => message,
        }
    }
}

fn run_check_command(root: &Path, program: &str, args: &[&str]) -> Result<(), CheckCommandError> {
    let status = std::process::Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                CheckCommandError::NotFound(program.to_string())
            } else {
                CheckCommandError::Failed(format!("failed to run {program}: {error}"))
            }
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(CheckCommandError::Failed(format!(
            "{program} {} failed",
            args.join(" ")
        )))
    }
}

fn run_setup(args: &[String]) -> Result<(), String> {
    let cli_options = parse_setup_args(args)?;
    let home = home_dir()?;
    let dotagent_repo = cli_options
        .dotagent_source
        .clone()
        .unwrap_or_else(|| home.join(".agent-toolkit/plugins/dotagent"));

    if cli_options.dry_run {
        if cli_options.dotagent_source.is_none() {
            println!(
                "would clone or update DotAgent source at {}",
                dotagent_repo.display()
            );
        }
    } else if cli_options.dotagent_source.is_none() {
        ensure_dotagent_repo(&dotagent_repo)?;
    }

    let mut setup_options = default_global_setup_options(&home);
    setup_options.all = cli_options.all;
    setup_options.include_gemini = !cli_options.skip_gemini;
    let plan = build_global_setup_plan(&home, &dotagent_repo, setup_options);
    print_setup_plan(&plan);

    if cli_options.dry_run {
        return Ok(());
    }

    if plan.actions.is_empty() {
        return Err("no setup actions found; install an agent CLI or rerun with --all".to_string());
    }

    if !cli_options.yes && !confirm_setup()? {
        println!("setup aborted");
        return Ok(());
    }

    let result = apply_global_setup_plan(&plan).map_err(|error| error.to_string())?;
    for path in result.updated_files {
        println!("updated {}", path.display());
    }
    for path in result.updated_codex_marketplaces {
        println!("registered Codex marketplace in {}", path.display());
    }
    for path in result.removed_legacy_extensions {
        println!("removed legacy Gemini extension {}", path.display());
    }
    for source in result.unchanged_extensions {
        println!("Gemini extension already linked {}", source.display());
    }
    for source in result.linked_extensions {
        println!("linked Gemini extension {}", source.display());
    }
    for skipped in result.skipped_extensions {
        println!(
            "skipped Gemini extension {}: {}",
            skipped.source.display(),
            skipped.reason
        );
    }
    println!("setup complete");
    Ok(())
}

fn run_update(args: &[String]) -> Result<(), String> {
    let mut setup_args = args.to_vec();
    if !setup_args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--yes" | "-y" | "--dry-run"))
    {
        setup_args.push("--yes".to_string());
    }
    run_setup(&setup_args)
}

fn run_teardown(args: &[String]) -> Result<(), String> {
    let options = parse_teardown_args(args)?;
    let home = home_dir()?;
    print_teardown_plan(&home, !options.skip_gemini);

    if options.dry_run {
        return Ok(());
    }

    if !options.yes && !confirm_teardown()? {
        println!("teardown aborted");
        return Ok(());
    }

    let result = teardown_global_rules(
        &home,
        GlobalTeardownOptions {
            include_gemini: !options.skip_gemini,
        },
    )
    .map_err(|error| error.to_string())?;
    for path in result.updated_files {
        println!("removed managed rules from {}", path.display());
    }
    for path in result.updated_codex_marketplaces {
        println!("removed Codex marketplace from {}", path.display());
    }
    for path in result.removed_extensions {
        println!("removed Gemini extension {}", path.display());
    }
    for skipped in result.skipped_extensions {
        println!(
            "skipped Gemini extension {}: {}",
            skipped.source.display(),
            skipped.reason
        );
    }
    println!("teardown complete");
    Ok(())
}

fn run_repo_setup(command: &str, args: &[String]) -> Result<(), String> {
    let options = parse_repo_setup_args(args)?;
    let root = env::current_dir().map_err(|error| error.to_string())?;
    let home = home_dir()?;
    let explicit_dotagent_source = options.dotagent_source.is_some();
    let dotagent_repo = options
        .dotagent_source
        .unwrap_or_else(|| home.join(".agent-toolkit/plugins/dotagent"));
    if !dotagent_repo.join("plugins/dotagent/AGENTS.md").exists() {
        if explicit_dotagent_source {
            return Err(format!(
                "DotAgent source was not found at {}",
                dotagent_repo.display()
            ));
        }
        ensure_dotagent_repo(&dotagent_repo)?;
    }

    let result = setup_repo(
        &root,
        &dotagent_repo,
        RepoSetupOptions {
            force_dotagent: options.force_dotagent,
        },
    )
    .map_err(|error| error.to_string())?;
    for change in result.changes {
        println!("{} {}", change.verb(), change.path);
    }
    println!(
        "wrote repo intelligence for {} source files",
        result.intel.file_count
    );
    if let Some(version) = result.dotagent.version {
        println!("installed DotAgent {version}");
    } else {
        println!("installed DotAgent");
    }
    println!("vendored {} skills", result.dotagent.installed_skills.len());
    for skipped in result.dotagent.skipped_skills {
        println!(
            "skipped {}: {} ({})",
            skipped.name, skipped.reason, skipped.path
        );
    }
    if !options.skip_sync {
        run_agents_sync(&root, false)?;
    }
    if result.issues.is_empty() {
        println!("agent-toolkit repo {command} passed");
        Ok(())
    } else {
        for issue in result.issues {
            println!("{:?}: {}", issue.code, issue.message);
        }
        Err(format!("repo {command} finished with check failures"))
    }
}

#[derive(Debug, Default)]
struct SetupCliOptions {
    yes: bool,
    dry_run: bool,
    all: bool,
    skip_gemini: bool,
    dotagent_source: Option<PathBuf>,
}

#[derive(Debug, Default)]
struct TeardownCliOptions {
    yes: bool,
    dry_run: bool,
    skip_gemini: bool,
}

fn parse_setup_args(args: &[String]) -> Result<SetupCliOptions, String> {
    let mut options = SetupCliOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--yes" | "-y" => options.yes = true,
            "--dry-run" => options.dry_run = true,
            "--all" => options.all = true,
            "--skip-gemini" => options.skip_gemini = true,
            "--dotagent-source" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err("--dotagent-source requires a path".to_string());
                };
                options.dotagent_source = Some(PathBuf::from(path));
            }
            flag => return Err(format!("unknown setup flag: {flag}")),
        }
        index += 1;
    }
    Ok(options)
}

fn parse_teardown_args(args: &[String]) -> Result<TeardownCliOptions, String> {
    let mut options = TeardownCliOptions::default();
    for arg in args {
        match arg.as_str() {
            "--yes" | "-y" => options.yes = true,
            "--dry-run" => options.dry_run = true,
            "--skip-gemini" => options.skip_gemini = true,
            flag => return Err(format!("unknown teardown flag: {flag}")),
        }
    }
    Ok(options)
}

#[derive(Debug)]
struct SyncCliOptions {
    check: bool,
    roots: Vec<PathBuf>,
}

#[derive(Debug, Default)]
struct RepoDotAgentCliOptions {
    dotagent_source: Option<PathBuf>,
    force: bool,
}

#[derive(Debug, Default)]
struct RepoSetupCliOptions {
    dotagent_source: Option<PathBuf>,
    force_dotagent: bool,
    skip_sync: bool,
}

fn parse_repo_setup_args(args: &[String]) -> Result<RepoSetupCliOptions, String> {
    let mut options = RepoSetupCliOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--force" => {
                options.force_dotagent = true;
            }
            "--skip-sync" => {
                options.skip_sync = true;
            }
            "--dotagent-source" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err("--dotagent-source requires a path".to_string());
                };
                options.dotagent_source = Some(PathBuf::from(path));
            }
            flag => return Err(format!("unknown repo setup flag: {flag}")),
        }
        index += 1;
    }
    Ok(options)
}

fn parse_repo_dotagent_args(args: &[String]) -> Result<RepoDotAgentCliOptions, String> {
    let mut options = RepoDotAgentCliOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--force" => {
                options.force = true;
            }
            "--dotagent-source" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err("--dotagent-source requires a path".to_string());
                };
                options.dotagent_source = Some(PathBuf::from(path));
            }
            flag => return Err(format!("unknown repo dotagent flag: {flag}")),
        }
        index += 1;
    }
    Ok(options)
}

fn parse_sync_args(args: &[String]) -> Result<SyncCliOptions, String> {
    let mut check = false;
    let mut roots = Vec::new();
    for arg in args {
        if arg == "--check" {
            check = true;
        } else if arg.starts_with('-') {
            return Err(format!("unknown sync flag: {arg}"));
        } else {
            roots.push(PathBuf::from(arg));
        }
    }
    Ok(SyncCliOptions { check, roots })
}

fn run_agents_sync(root: &std::path::Path, check: bool) -> Result<(), String> {
    let mut command = std::process::Command::new("agents");
    command.args(["sync", "--path"]).arg(root);
    if check {
        command.arg("--check");
    }
    let status = command.status().map_err(|error| {
        format!(
            "failed to run agents CLI ({error}); install @agents-dev/cli or remove this sync step"
        )
    })?;
    if status.success() {
        Ok(())
    } else {
        Err("agents sync failed".to_string())
    }
}

fn print_setup_plan(plan: &agent_toolkit_core::global_setup::GlobalSetupPlan) {
    println!("Setup plan");
    println!("source {}", plan.dotagent_repo.display());
    if plan.actions.is_empty() {
        println!("actions none");
    } else {
        for action in &plan.actions {
            match &action.kind {
                GlobalSetupActionKind::ManagedRules { path, .. } => {
                    println!(
                        "action {}: {} -> {}",
                        action.agent,
                        action.description,
                        path.display()
                    );
                }
                GlobalSetupActionKind::CodexMarketplaceRegistration {
                    config_path,
                    source,
                } => {
                    println!(
                        "action {}: {} -> {} ({})",
                        action.agent,
                        action.description,
                        config_path.display(),
                        source.display()
                    );
                }
                GlobalSetupActionKind::LegacyGeminiExtensionRemoval { path } => {
                    println!(
                        "action {}: {} -> {}",
                        action.agent,
                        action.description,
                        path.display()
                    );
                }
                GlobalSetupActionKind::GeminiExtensionAlreadyLinked { source } => {
                    println!(
                        "action {}: {} -> {}",
                        action.agent,
                        action.description,
                        source.display()
                    );
                }
                GlobalSetupActionKind::GeminiExtensionLink { source } => {
                    println!(
                        "action {}: {} -> {}",
                        action.agent,
                        action.description,
                        source.display()
                    );
                }
            }
        }
    }
    for skip in &plan.skipped {
        println!("skip {}: {}", skip.agent, skip.reason);
    }
}

fn print_teardown_plan(home: &Path, include_gemini: bool) {
    println!("Teardown plan");
    println!(
        "action Claude: Remove managed rules block -> {}",
        home.join(".claude/CLAUDE.md").display()
    );
    println!(
        "action Codex: Remove managed rules block -> {}",
        home.join(".codex/AGENTS.md").display()
    );
    println!(
        "action Codex: Remove DotAgent local marketplace -> {}",
        home.join(".codex/config.toml").display()
    );
    if include_gemini {
        println!(
            "action Gemini: Remove DotAgent extension link -> {}",
            home.join(".gemini/extensions/dotagent").display()
        );
    } else {
        println!("skip Gemini: disabled by --skip-gemini");
    }
}

fn confirm_setup() -> Result<bool, String> {
    print!("Proceed with setup? [y/N] ");
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|error| error.to_string())?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

fn confirm_teardown() -> Result<bool, String> {
    print!("Proceed with teardown? [y/N] ");
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|error| error.to_string())?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

fn ensure_dotagent_repo(path: &std::path::Path) -> Result<(), String> {
    if path.join("plugins/dotagent/AGENTS.md").exists() {
        let status = std::process::Command::new("git")
            .args(["-C", path.to_string_lossy().as_ref(), "pull", "--ff-only"])
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err("failed to update dotagent source repo".to_string());
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let status = std::process::Command::new("git")
        .args([
            "clone",
            "https://github.com/harryy2510/dotagent.git",
            path.to_string_lossy().as_ref(),
        ])
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("failed to clone dotagent source repo".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_repo_setup_args_supports_dotagent_source_force_and_skip_sync() {
        let options = parse_repo_setup_args(&[
            "--dotagent-source".to_string(),
            "/tmp/dotagent".to_string(),
            "--force".to_string(),
            "--skip-sync".to_string(),
        ])
        .unwrap();

        assert_eq!(
            options.dotagent_source,
            Some(PathBuf::from("/tmp/dotagent"))
        );
        assert!(options.force_dotagent);
        assert!(options.skip_sync);
    }

    #[test]
    fn parse_teardown_args_supports_yes_dry_run_and_skip_gemini() {
        let options = parse_teardown_args(&[
            "--yes".to_string(),
            "--dry-run".to_string(),
            "--skip-gemini".to_string(),
        ])
        .unwrap();

        assert!(options.yes);
        assert!(options.dry_run);
        assert!(options.skip_gemini);
    }
}
