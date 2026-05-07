use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, CallExpression, Class, Declaration, ExportDefaultDeclaration,
    ExportDefaultDeclarationKind, Expression, Function, ImportDeclaration, ImportExpression,
    ModuleDeclaration, ModuleExportName, StaticMemberExpression, VariableDeclaration,
};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::{ParseOptions, Parser};
use oxc_span::SourceType;

pub struct RepoIntel {
    pub summary_markdown: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntelReadTier {
    Entrypoint,
    ShortIndex,
    TargetedContext,
    DeepContext,
    MachineDeepContext,
}

impl IntelReadTier {
    fn as_str(self) -> &'static str {
        match self {
            Self::Entrypoint => "entrypoint",
            Self::ShortIndex => "short-index",
            Self::TargetedContext => "targeted-context",
            Self::DeepContext => "deep-context",
            Self::MachineDeepContext => "machine-deep-context",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Entrypoint => "Entrypoints",
            Self::ShortIndex => "Short Indexes",
            Self::TargetedContext => "Targeted Context",
            Self::DeepContext => "Deep Context",
            Self::MachineDeepContext => "Machine Deep Context",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct IntelFileMeta {
    path: &'static str,
    description: &'static str,
    tier: IntelReadTier,
    read_policy: &'static str,
}

const INTEL_FILE_META: [IntelFileMeta; 21] = [
    IntelFileMeta {
        path: "summary.md",
        description: "tiny startup index and read policy",
        tier: IntelReadTier::Entrypoint,
        read_policy: "Read first. Keep this as the small startup entrypoint before opening task-specific articles.",
    },
    IntelFileMeta {
        path: "index.md",
        description: "same startup index as summary.md",
        tier: IntelReadTier::Entrypoint,
        read_policy: "Read first. Mirrors summary.md for tools that prefer index-style filenames.",
    },
    IntelFileMeta {
        path: "overview.md",
        description: "architecture, scale, high-impact files",
        tier: IntelReadTier::ShortIndex,
        read_policy: "Read early for repo shape, stack signals, hotspots, generated files, and large modules.",
    },
    IntelFileMeta {
        path: "tasks.md",
        description: "task-oriented read paths so agents know where to start",
        tier: IntelReadTier::ShortIndex,
        read_policy: "Read early to pick the smallest task-specific article set instead of scanning broadly.",
    },
    IntelFileMeta {
        path: "tooling.md",
        description: "scripts, configs, package dependencies",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read when running commands or changing package, tooling, CI, or config surfaces.",
    },
    IntelFileMeta {
        path: "routes.md",
        description: "framework route files and route-like modules",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read only for route, navigation, page, loader, action, or framework routing work.",
    },
    IntelFileMeta {
        path: "api.md",
        description: "API handlers, server functions, endpoint declarations",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read only for API, server function, RPC, endpoint, or backend behavior work.",
    },
    IntelFileMeta {
        path: "components.md",
        description: "UI components and prop surfaces",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read only for component or screen work; large repos may make this file substantial.",
    },
    IntelFileMeta {
        path: "data.md",
        description: "SQL schema, migrations, Supabase/data files",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read only for data-layer, migration, seed, schema, or Supabase work.",
    },
    IntelFileMeta {
        path: "database.md",
        description: "pg_query-backed static database design, relationships, RLS, RPCs",
        tier: IntelReadTier::DeepContext,
        read_policy: "Deep-read only. Open when the task requires database design, RLS, RPC, migration timeline, or relationship detail.",
    },
    IntelFileMeta {
        path: "graph.md",
        description: "import graph, blast radius, central modules",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read before broad refactors or changes to high-impact shared modules.",
    },
    IntelFileMeta {
        path: "impact.md",
        description: "change-impact read plans by high-risk file",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read when editing a high-impact file or planning a shared-module change.",
    },
    IntelFileMeta {
        path: "boundaries.md",
        description: "client/server/data/generated boundary signals",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read when a task involves runtime boundaries, generated files, env usage, or client/server splits.",
    },
    IntelFileMeta {
        path: "imports.md",
        description: "local import adjacency grouped by source file",
        tier: IntelReadTier::DeepContext,
        read_policy: "Deep-read only. Open for exact dependency tracing after summary, tasks, graph, or impact points to a path.",
    },
    IntelFileMeta {
        path: "calls.md",
        description: "AST-derived function and method call sites by source file",
        tier: IntelReadTier::DeepContext,
        read_policy: "Deep-read only. Open for exact API/call-site tracing after a focused target is known.",
    },
    IntelFileMeta {
        path: "dependencies.md",
        description: "external imports and package usage",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read when package usage, dependency cleanup, or external API surface matters.",
    },
    IntelFileMeta {
        path: "symbols.md",
        description: "exported symbols by source file",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read when public exports or symbol ownership matter; avoid using it as startup context.",
    },
    IntelFileMeta {
        path: "files.md",
        description: "full source-like file inventory with tags",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read when the task needs a full file inventory; avoid using it as startup context.",
    },
    IntelFileMeta {
        path: "env.md",
        description: "environment variable usage by file",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read for env-name usage only. It must not contain or require live secret values.",
    },
    IntelFileMeta {
        path: "testing.md",
        description: "tests, coverage signals, test-adjacent files",
        tier: IntelReadTier::TargetedContext,
        read_policy: "Read when selecting or changing tests and verification commands.",
    },
    IntelFileMeta {
        path: "repo.json",
        description: "machine-readable full repo intelligence payload",
        tier: IntelReadTier::MachineDeepContext,
        read_policy: "Machine/deep-read only. Do not open during normal startup; use only for tooling or exact structured lookups.",
    },
];

#[derive(Debug, Clone)]
struct RepoAnalysis {
    files: Vec<FileIntel>,
    package: Option<PackageInfo>,
    frameworks: Vec<String>,
    path_aliases: Vec<PathAlias>,
    local_edges: Vec<ImportEdge>,
    reverse_import_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
struct FileIntel {
    path: PathBuf,
    normalized_path: String,
    extension: String,
    line_count: usize,
    imports: Vec<String>,
    exports: Vec<String>,
    call_sites: Vec<String>,
    components: Vec<ComponentIntel>,
    routes: Vec<RouteIntel>,
    api_endpoints: Vec<ApiEndpoint>,
    sql_objects: Vec<SqlObject>,
    sql_tables: Vec<SqlTable>,
    sql_actions: Vec<SqlAction>,
    env_vars: Vec<String>,
    is_test: bool,
    is_generated: bool,
    used_ast: bool,
    parse_error_count: usize,
}

#[derive(Debug, Clone)]
struct PackageInfo {
    scripts: Vec<String>,
    dependencies: Vec<String>,
    dev_dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
struct ComponentIntel {
    name: String,
    props: Vec<String>,
}

#[derive(Debug, Clone)]
struct RouteIntel {
    framework: String,
    route: String,
    kind: String,
}

#[derive(Debug, Clone)]
struct ApiEndpoint {
    method: String,
    route: String,
}

#[derive(Debug, Clone)]
struct SqlObject {
    kind: String,
    name: String,
}

#[derive(Debug, Clone)]
struct SqlTable {
    name: String,
    columns: Vec<SqlColumn>,
}

#[derive(Debug, Clone)]
struct SqlColumn {
    name: String,
    data_type: String,
    flags: Vec<String>,
    references: Option<String>,
}

#[derive(Debug, Clone)]
struct SqlAction {
    kind: String,
    name: String,
    target: Option<String>,
    detail: Option<String>,
    line: usize,
}

#[derive(Debug, Clone, Default)]
struct DatabaseTable {
    name: String,
    columns: Vec<SqlColumn>,
    indexes: Vec<String>,
    triggers: Vec<String>,
    policies: Vec<String>,
    grants: Vec<String>,
    touched_by: Vec<String>,
    rls_enabled: bool,
}

#[derive(Debug, Clone)]
struct ImportEdge {
    from: String,
    to: String,
}

#[derive(Debug, Clone)]
struct PathAlias {
    prefix: String,
    target_prefix: String,
}

#[derive(Debug, Default)]
struct AstFileIntel {
    imports: Vec<String>,
    exports: Vec<String>,
    component_names: Vec<String>,
    env_vars: Vec<String>,
    call_sites: Vec<String>,
    route_declarations: Vec<String>,
    http_routes: Vec<ApiEndpoint>,
    parse_error_count: usize,
}

pub fn write_repo_intel(root: &Path) -> std::io::Result<RepoIntel> {
    let analysis = analyze_repo(root)?;
    let summary_markdown = render_index(&analysis);
    let intel_dir = root.join(".agents/intel");
    fs::create_dir_all(&intel_dir)?;

    let mut rendered_files = vec![("summary.md".to_string(), summary_markdown.clone())];
    rendered_files.extend(
        render_articles(&analysis)
            .into_iter()
            .map(|(filename, contents)| (filename.to_string(), contents)),
    );
    rendered_files.push(("repo.json".to_string(), render_repo_json(&analysis)));

    let manifest = render_manifest_json(&rendered_files);
    for (filename, contents) in &rendered_files {
        fs::write(intel_dir.join(filename), contents)?;
    }
    fs::write(intel_dir.join("manifest.json"), manifest)?;

    Ok(RepoIntel {
        summary_markdown,
        file_count: analysis.files.len(),
    })
}

pub fn build_repo_intel(root: &Path) -> std::io::Result<RepoIntel> {
    let analysis = analyze_repo(root)?;
    let summary_markdown = render_index(&analysis);

    Ok(RepoIntel {
        summary_markdown,
        file_count: analysis.files.len(),
    })
}

fn analyze_repo(root: &Path) -> std::io::Result<RepoAnalysis> {
    let files = collect_source_files(root)?;
    let package = fs::read_to_string(root.join("package.json"))
        .ok()
        .map(|contents| parse_package_info(&contents));
    let path_aliases = parse_path_aliases(root);
    let mut analyzed = Vec::with_capacity(files.len());
    for file in files {
        let full_path = root.join(&file);
        let contents = fs::read_to_string(&full_path).unwrap_or_default();
        let normalized_path = normalize_path(&file);
        let extension = file
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("")
            .to_string();
        let ast = extract_ast_file_intel(&file, &contents, &extension);
        let imports = ast
            .as_ref()
            .map(|ast| ast.imports.clone())
            .unwrap_or_else(|| extract_imports(&contents, &extension));
        let exports = merge_strings(
            ast.as_ref()
                .map(|ast| ast.exports.clone())
                .unwrap_or_default(),
            extract_exports(&contents, &extension),
            32,
        );
        let components = merge_components(
            ast.as_ref()
                .map(|ast| components_from_ast_names(&ast.component_names, &contents))
                .unwrap_or_default(),
            extract_components(&contents, &extension),
        );
        let routes = merge_routes(
            detect_routes(&normalized_path, &contents),
            ast.as_ref()
                .map(|ast| routes_from_ast(&ast.route_declarations))
                .unwrap_or_default(),
        );
        let api_endpoints = merge_api_endpoints(
            detect_api_endpoints(&normalized_path, &contents, &exports),
            ast.as_ref()
                .map(|ast| ast.http_routes.clone())
                .unwrap_or_default(),
        );
        let sql_objects = extract_sql_objects(&contents, &extension);
        let sql_tables = extract_sql_tables(&contents, &extension);
        let sql_actions = extract_sql_actions(&contents, &extension);
        let env_vars = merge_strings(
            ast.as_ref()
                .map(|ast| ast.env_vars.clone())
                .unwrap_or_default(),
            extract_env_vars(&contents),
            80,
        );
        let call_sites = ast
            .as_ref()
            .map(|ast| ast.call_sites.clone())
            .unwrap_or_default();
        let parse_error_count = ast
            .as_ref()
            .map(|ast| ast.parse_error_count)
            .unwrap_or_default();

        analyzed.push(FileIntel {
            path: file,
            normalized_path: normalized_path.clone(),
            extension: extension.clone(),
            line_count: contents.lines().count(),
            imports,
            exports,
            call_sites,
            components,
            routes,
            api_endpoints,
            sql_objects,
            sql_tables,
            sql_actions,
            env_vars,
            is_test: is_test_file(&normalized_path),
            is_generated: is_generated_file(&normalized_path),
            used_ast: ast.is_some(),
            parse_error_count,
        });
    }

    analyzed.sort_by(|left, right| left.normalized_path.cmp(&right.normalized_path));

    let local_edges = build_local_edges(&analyzed, &path_aliases);
    let mut reverse_import_counts: BTreeMap<String, usize> = BTreeMap::new();
    for edge in &local_edges {
        *reverse_import_counts.entry(edge.to.clone()).or_default() += 1;
    }
    let frameworks = detect_frameworks(root, package.as_ref(), &analyzed);

    Ok(RepoAnalysis {
        files: analyzed,
        package,
        frameworks,
        path_aliases,
        local_edges,
        reverse_import_counts,
    })
}

fn render_articles(analysis: &RepoAnalysis) -> Vec<(&'static str, String)> {
    vec![
        ("index.md", render_index(analysis)),
        ("overview.md", render_overview(analysis)),
        ("tasks.md", render_tasks(analysis)),
        ("tooling.md", render_tooling(analysis)),
        ("routes.md", render_routes(analysis)),
        ("api.md", render_api(analysis)),
        ("components.md", render_components(analysis)),
        ("data.md", render_data(analysis)),
        ("database.md", render_database(analysis)),
        ("graph.md", render_graph(analysis)),
        ("impact.md", render_impact(analysis)),
        ("boundaries.md", render_boundaries(analysis)),
        ("imports.md", render_imports(analysis)),
        ("calls.md", render_calls(analysis)),
        ("dependencies.md", render_dependencies(analysis)),
        ("symbols.md", render_symbols(analysis)),
        ("files.md", render_files(analysis)),
        ("env.md", render_env(analysis)),
        ("testing.md", render_testing(analysis)),
    ]
}

fn intel_file_meta(path: &str) -> IntelFileMeta {
    INTEL_FILE_META
        .iter()
        .copied()
        .find(|meta| meta.path == path)
        .unwrap_or(IntelFileMeta {
            path: "unknown",
            description: "generated repo intelligence file",
            tier: IntelReadTier::TargetedContext,
            read_policy: "Read only when the current task requires this generated context.",
        })
}

fn render_manifest_json(files: &[(String, String)]) -> String {
    let mut output = String::from("{\n");
    output.push_str("\t\"schemaVersion\": 1,\n");
    output.push_str("\t\"entrypoint\": \"summary.md\",\n");
    output.push_str("\t\"guidance\": \"Read summary.md first. Open targeted context only for the current task. Treat deep-context files as explicit opt-in reads.\",\n");
    output.push_str("\t\"files\": [\n");
    for (index, (path, contents)) in files.iter().enumerate() {
        let meta = intel_file_meta(path);
        output.push_str("\t\t{\n");
        output.push_str(&format!("\t\t\t\"path\": \"{}\",\n", json_escape(path)));
        output.push_str(&format!("\t\t\t\"tier\": \"{}\",\n", meta.tier.as_str()));
        output.push_str(&format!("\t\t\t\"bytes\": {},\n", contents.len()));
        output.push_str(&format!(
            "\t\t\t\"description\": \"{}\",\n",
            json_escape(meta.description)
        ));
        output.push_str(&format!(
            "\t\t\t\"readPolicy\": \"{}\"\n",
            json_escape(meta.read_policy)
        ));
        output.push_str("\t\t}");
        if index + 1 != files.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("\t]\n");
    output.push_str("}\n");
    output
}

fn render_index(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Repository Intelligence\n\n");
    output.push_str("## Preferred Context\n\n");
    output.push_str(
        "- Start here and keep startup context small; this file is an index, not a source dump.\n",
    );
    output.push_str("- Open `overview.md` and `tasks.md` first, then only the targeted article needed for the task.\n");
    output.push_str(
        "- Treat `database.md`, `imports.md`, `calls.md`, and `repo.json` as deep-read only.\n",
    );
    output.push_str("- This index is deterministic and local-only. It is a map, not a substitute for reading implementation before editing.\n\n");

    output.push_str("## Read Policy\n\n");
    for tier in [
        IntelReadTier::Entrypoint,
        IntelReadTier::ShortIndex,
        IntelReadTier::TargetedContext,
        IntelReadTier::DeepContext,
        IntelReadTier::MachineDeepContext,
    ] {
        output.push_str(&format!("### {}\n\n", tier.label()));
        for meta in INTEL_FILE_META.iter().filter(|meta| meta.tier == tier) {
            output.push_str(&format!(
                "- [`{}`](./{}) — {}\n",
                meta.path, meta.path, meta.description
            ));
        }
        output.push('\n');
    }

    output.push_str("\n## Quick Stats\n\n");
    output.push_str(&format!("- Source-like files: {}\n", analysis.files.len()));
    output.push_str(&format!(
        "- Frameworks/signals: {}\n",
        display_or_none(&analysis.frameworks)
    ));
    output.push_str(&format!("- Routes: {}\n", count_routes(analysis)));
    output.push_str(&format!(
        "- API endpoints/modules: {}\n",
        count_api(analysis)
    ));
    output.push_str(&format!("- Components: {}\n", count_components(analysis)));
    output.push_str(&format!("- SQL objects: {}\n", count_sql_objects(analysis)));
    output.push_str(&format!("- Env vars: {}\n", env_var_index(analysis).len()));
    output.push_str(&format!(
        "- Local import edges: {}\n",
        analysis.local_edges.len()
    ));
    output.push_str(&format!(
        "- AST-parsed JS/TS files: {}\n",
        analysis.files.iter().filter(|file| file.used_ast).count()
    ));
    output.push_str(&format!(
        "- Call-site files: {}\n\n",
        analysis
            .files
            .iter()
            .filter(|file| !file.call_sites.is_empty())
            .count()
    ));

    push_read_first(&mut output);
    output
}

fn render_overview(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Overview\n\n");
    output.push_str("## Stack Signals\n\n");
    for framework in &analysis.frameworks {
        output.push_str(&format!("- {framework}\n"));
    }
    if analysis.frameworks.is_empty() {
        output.push_str("- No framework signals detected from package metadata or file layout.\n");
    }

    output.push_str("\n## Scale\n\n");
    output.push_str(&format!("- Source-like files: {}\n", analysis.files.len()));
    output.push_str(&format!(
        "- UI components: {}\n",
        count_components(analysis)
    ));
    output.push_str(&format!("- Routes: {}\n", count_routes(analysis)));
    output.push_str(&format!(
        "- API endpoints/modules: {}\n",
        count_api(analysis)
    ));
    output.push_str(&format!("- SQL objects: {}\n", count_sql_objects(analysis)));
    output.push_str(&format!("- Tests: {}\n", count_tests(analysis)));

    output.push_str("\n## Source Areas\n\n");
    for (area, count) in top_areas(analysis, 20) {
        output.push_str(&format!("- `{area}`: {count}\n"));
    }

    output.push_str("\n## High-Impact Files\n\n");
    for (path, count) in top_reverse_imports(analysis, 20) {
        output.push_str(&format!("- `{path}` — imported by {count} local files\n"));
    }

    output.push_str("\n## Largest Non-Generated Files\n\n");
    for file in largest_files(analysis, 20) {
        output.push_str(&format!(
            "- `{}` — {} lines\n",
            file.normalized_path, file.line_count
        ));
    }

    output.push_str("\n## Exported Symbols Sample\n\n");
    let mut exported = 0usize;
    for file in analysis
        .files
        .iter()
        .filter(|file| !file.exports.is_empty())
        .take(30)
    {
        output.push_str(&format!(
            "- `{}` — {}\n",
            file.normalized_path,
            file.exports.join(", ")
        ));
        exported += 1;
    }
    if exported == 0 {
        output.push_str("- No exported symbols detected.\n");
    }

    let generated: Vec<&FileIntel> = analysis
        .files
        .iter()
        .filter(|file| file.is_generated)
        .take(20)
        .collect();
    if !generated.is_empty() {
        output.push_str("\n## Generated Files Detected\n\n");
        for file in generated {
            output.push_str(&format!("- `{}`\n", file.normalized_path));
        }
    }

    output
}

fn render_tasks(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Task Read Paths\n\n");
    output.push_str("Use this file to avoid broad project scans. Pick the task type, read the listed intel article, then read the source files named there.\n\n");

    output.push_str("## Universal Start\n\n");
    output.push_str("- Read `overview.md` for stack, scale, high-impact files, generated files, and large modules.\n");
    output.push_str(
        "- Read `tooling.md` before running commands or changing package/config surfaces.\n",
    );
    output.push_str("- Read `graph.md` before touching a high-impact shared module.\n");
    output.push_str("- Do not open `repo.json`, `database.md`, `imports.md`, or `calls.md` unless the current task specifically needs deep structured context.\n\n");

    output.push_str("## By Task Type\n\n");
    for (task, article, reason) in [
        (
            "UI component or screen work",
            "components.md, routes.md, graph.md",
            "Find component ownership, route entrypoints, props, and shared UI dependencies.",
        ),
        (
            "Route behavior or navigation",
            "routes.md, api.md, graph.md",
            "Find route files, server/API dependencies, and shared modules used by the route.",
        ),
        (
            "API/server function work",
            "api.md, data.md, env.md, graph.md",
            "Find server modules, schema/data dependencies, env requirements, and blast radius.",
        ),
        (
            "Database or migration work",
            "data.md, env.md, testing.md",
            "Find SQL objects, migration files, data helpers, and test scripts.",
        ),
        (
            "Auth/config/secrets work",
            "env.md, tooling.md, api.md",
            "Find env names and files that reference them without exposing values.",
        ),
        (
            "Refactor/shared helper work",
            "impact.md, graph.md, symbols.md, imports.md, files.md",
            "Find import fan-in/fan-out, exported symbols, and all files in the affected area.",
        ),
        (
            "Test work",
            "testing.md, files.md, graph.md",
            "Find existing tests, test scripts, and nearby source/test boundaries.",
        ),
    ] {
        output.push_str(&format!("- **{task}**: `{article}` — {reason}\n"));
    }

    output.push_str("\n## Highest Blast-Radius Files\n\n");
    for (path, count) in top_reverse_imports(analysis, 12) {
        output.push_str(&format!(
            "- `{path}` — read before changing; imported by {count} local files\n"
        ));
    }

    output
}

fn render_tooling(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Tooling\n\n");
    if let Some(package) = &analysis.package {
        if !package.scripts.is_empty() {
            output.push_str("## Package Scripts\n\n");
            for script in &package.scripts {
                output.push_str(&format!("- `bun {script}`\n"));
            }
            output.push('\n');
        }
        if !package.dependencies.is_empty() {
            output.push_str("## Dependencies\n\n");
            for dependency in package.dependencies.iter().take(80) {
                output.push_str(&format!("- `{dependency}`\n"));
            }
            output.push('\n');
        }
        if !package.dev_dependencies.is_empty() {
            output.push_str("## Dev Dependencies\n\n");
            for dependency in package.dev_dependencies.iter().take(80) {
                output.push_str(&format!("- `{dependency}`\n"));
            }
            output.push('\n');
        }
    } else {
        output.push_str("No `package.json` detected.\n\n");
    }

    output.push_str("## Config Files\n\n");
    for file in analysis.files.iter().filter(|file| is_config_file(file)) {
        output.push_str(&format!("- `{}`\n", file.normalized_path));
    }

    output
}

fn render_routes(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Routes\n\n");
    let mut routes: Vec<(&FileIntel, &RouteIntel)> = analysis
        .files
        .iter()
        .flat_map(|file| file.routes.iter().map(move |route| (file, route)))
        .collect();
    routes.sort_by(|left, right| {
        left.1
            .route
            .cmp(&right.1.route)
            .then_with(|| left.0.normalized_path.cmp(&right.0.normalized_path))
    });

    if routes.is_empty() {
        output.push_str("No framework route files detected.\n");
        return output;
    }

    for (file, route) in routes {
        output.push_str(&format!(
            "- `{}` — {} `{}` ({})\n",
            file.normalized_path, route.framework, route.route, route.kind
        ));
    }
    output
}

fn render_api(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# API And Server Surface\n\n");
    let mut endpoint_count = 0usize;
    for file in &analysis.files {
        if file.api_endpoints.is_empty() && !is_api_module(&file.normalized_path) {
            continue;
        }
        output.push_str(&format!("- `{}`", file.normalized_path));
        if !file.api_endpoints.is_empty() {
            let endpoints: Vec<String> = file
                .api_endpoints
                .iter()
                .map(|endpoint| format!("{} {}", endpoint.method, endpoint.route))
                .collect();
            output.push_str(&format!(" — {}", endpoints.join(", ")));
            endpoint_count += endpoints.len();
        }
        output.push('\n');
    }

    if endpoint_count == 0
        && !analysis
            .files
            .iter()
            .any(|file| is_api_module(&file.normalized_path))
    {
        output.push_str("No API handlers or API-like modules detected.\n");
    }
    output
}

fn render_components(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Components\n\n");
    let mut components: Vec<(&FileIntel, &ComponentIntel)> = analysis
        .files
        .iter()
        .flat_map(|file| {
            file.components
                .iter()
                .map(move |component| (file, component))
        })
        .collect();
    components.sort_by(|left, right| {
        left.1
            .name
            .cmp(&right.1.name)
            .then_with(|| left.0.normalized_path.cmp(&right.0.normalized_path))
    });

    if components.is_empty() {
        output.push_str("No UI component exports detected.\n");
        return output;
    }

    for (file, component) in components {
        if component.props.is_empty() {
            output.push_str(&format!(
                "- **{}** — `{}`\n",
                component.name, file.normalized_path
            ));
        } else {
            output.push_str(&format!(
                "- **{}** — props: {} — `{}`\n",
                component.name,
                component.props.join(", "),
                file.normalized_path
            ));
        }
    }
    output
}

fn render_data(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Data And Database\n\n");
    let mut sql_files = 0usize;
    for file in &analysis.files {
        if file.extension != "sql" && !is_data_file(&file.normalized_path) {
            continue;
        }
        if file.extension == "sql" {
            sql_files += 1;
        }
        output.push_str(&format!("- `{}`", file.normalized_path));
        if !file.sql_objects.is_empty() {
            let objects: Vec<String> = file
                .sql_objects
                .iter()
                .take(8)
                .map(|object| format!("{} {}", object.kind, object.name))
                .collect();
            output.push_str(&format!(" — {}", objects.join(", ")));
        }
        output.push('\n');
    }
    if sql_files == 0 {
        output.push_str("No SQL files detected.\n");
    }
    output
}

fn render_database(analysis: &RepoAnalysis) -> String {
    let tables = database_tables(analysis);
    let actions = database_actions(analysis);
    let sql_files: Vec<&FileIntel> = analysis
        .files
        .iter()
        .filter(|file| file.extension == "sql")
        .collect();
    let mut output = String::from("# Database Design\n\n");
    push_article_read_policy(&mut output, "database.md");
    output.push_str("pg_query-backed ordered static database map for agents. This reduces migration files in path order for common schema changes without requiring a live database; procedural or dynamic SQL may still need source inspection.\n\n");

    output.push_str("## Summary\n\n");
    output.push_str(&format!("- SQL files: {}\n", sql_files.len()));
    output.push_str(&format!(
        "- Migration files: {}\n",
        sql_files
            .iter()
            .filter(|file| file.normalized_path.starts_with("supabase/migrations/"))
            .count()
    ));
    output.push_str(&format!("- Tables discovered: {}\n", tables.len()));
    output.push_str(&format!(
        "- Functions/RPCs: {}\n",
        actions
            .iter()
            .filter(|action| action.kind == "function")
            .count()
    ));
    output.push_str(&format!(
        "- Policies: {}\n",
        actions
            .iter()
            .filter(|action| action.kind == "policy")
            .count()
    ));
    output.push_str(&format!(
        "- Indexes: {}\n",
        actions
            .iter()
            .filter(|action| action.kind == "index")
            .count()
    ));
    output.push_str(&format!(
        "- Triggers: {}\n",
        actions
            .iter()
            .filter(|action| action.kind == "trigger")
            .count()
    ));
    output.push_str(&format!(
        "- Relationship edges: {}\n\n",
        database_relationships(&tables).len()
    ));

    output.push_str("## SQL Timeline\n\n");
    for file in sql_files.iter().take(180) {
        let actions = &file.sql_actions;
        output.push_str(&format!(
            "- `{}` - {} lines, {} database actions",
            file.normalized_path,
            file.line_count,
            actions.len()
        ));
        let highlights: Vec<String> = actions
            .iter()
            .filter(|action| {
                matches!(
                    action.kind.as_str(),
                    "table" | "function" | "policy" | "trigger" | "drop"
                )
            })
            .take(5)
            .map(|action| format!("{} {}", action.kind, action.name))
            .collect();
        if !highlights.is_empty() {
            output.push_str(&format!(" - {}", highlights.join(", ")));
        }
        output.push('\n');
    }

    output.push_str("\n## Relationship Map\n\n");
    let relationships = database_relationships(&tables);
    if relationships.is_empty() {
        output.push_str("- No foreign-key style references detected.\n");
    } else {
        for (from, to) in relationships.iter().take(240) {
            output.push_str(&format!("- `{from}` -> `{to}`\n"));
        }
    }

    output.push_str("\n## Tables\n\n");
    for table in tables.values().take(160) {
        output.push_str(&format!("### `{}`\n\n", table.name));
        output.push_str(&format!(
            "- Last touched by: `{}`\n",
            display_or_none(&table.touched_by)
        ));
        output.push_str(&format!(
            "- RLS: {}\n",
            if table.rls_enabled {
                "enabled"
            } else {
                "not detected"
            }
        ));
        if !table.columns.is_empty() {
            output.push_str("- Columns:\n");
            for column in table.columns.iter().take(80) {
                output.push_str(&format!(
                    "  - `{}`: {}{}{}\n",
                    column.name,
                    column.data_type,
                    display_column_flags(&column.flags),
                    column
                        .references
                        .as_ref()
                        .map(|target| format!(" -> `{target}`"))
                        .unwrap_or_default()
                ));
            }
        }
        push_named_list(&mut output, "Indexes", &table.indexes, 24);
        push_named_list(&mut output, "Triggers", &table.triggers, 16);
        push_named_list(&mut output, "Policies", &table.policies, 32);
        push_named_list(&mut output, "Grants", &table.grants, 16);
        output.push('\n');
    }

    output.push_str("## Functions And RPCs\n\n");
    for action in actions
        .iter()
        .filter(|action| action.kind == "function")
        .take(220)
    {
        output.push_str(&format!(
            "- `{}` - `{}`:{}{}\n",
            action.name,
            action
                .target
                .as_ref()
                .map(String::as_str)
                .unwrap_or("unknown"),
            action.line,
            action
                .detail
                .as_ref()
                .map(|detail| format!(" - {detail}"))
                .unwrap_or_default()
        ));
    }

    output.push_str("\n## Types And Views\n\n");
    for action in actions
        .iter()
        .filter(|action| matches!(action.kind.as_str(), "type" | "view"))
        .take(120)
    {
        output.push_str(&format!(
            "- {} `{}` - `{}`:{}\n",
            action.kind,
            action.name,
            action
                .target
                .as_ref()
                .map(String::as_str)
                .unwrap_or("unknown"),
            action.line
        ));
    }

    output.push_str("\n## Drops And Replacements\n\n");
    for action in actions
        .iter()
        .filter(|action| matches!(action.kind.as_str(), "drop" | "alter"))
        .take(180)
    {
        output.push_str(&format!(
            "- {} `{}` - `{}`:{}{}\n",
            action.kind,
            action.name,
            action
                .target
                .as_ref()
                .map(String::as_str)
                .unwrap_or("unknown"),
            action.line,
            action
                .detail
                .as_ref()
                .map(|detail| format!(" - {detail}"))
                .unwrap_or_default()
        ));
    }

    output
}

fn render_graph(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Import Graph\n\n");
    if !analysis.path_aliases.is_empty() {
        output.push_str("## Path Aliases\n\n");
        for alias in &analysis.path_aliases {
            output.push_str(&format!(
                "- `{}` -> `{}`\n",
                alias.prefix, alias.target_prefix
            ));
        }
        output.push('\n');
    }

    output.push_str("## High-Impact Files\n\n");
    for (path, count) in top_reverse_imports(analysis, 40) {
        output.push_str(&format!("- `{path}` — imported by {count} local files\n"));
    }

    output.push_str("\n## Import-Heavy Files\n\n");
    let mut import_heavy: Vec<&FileIntel> = analysis
        .files
        .iter()
        .filter(|file| !file.imports.is_empty())
        .collect();
    import_heavy.sort_by(|left, right| {
        right
            .imports
            .len()
            .cmp(&left.imports.len())
            .then_with(|| left.normalized_path.cmp(&right.normalized_path))
    });
    for file in import_heavy.into_iter().take(40) {
        output.push_str(&format!(
            "- `{}` — {} imports\n",
            file.normalized_path,
            file.imports.len()
        ));
    }

    output.push_str("\n## Local Edges Sample\n\n");
    for edge in analysis.local_edges.iter().take(120) {
        output.push_str(&format!("- `{}` -> `{}`\n", edge.from, edge.to));
    }
    output
}

fn render_impact(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Change Impact Read Plans\n\n");
    output.push_str("Use this before editing high-impact files. Each plan lists the file itself, direct dependencies, direct dependents, related tests, and the intel articles that usually matter.\n\n");
    let reverse_edges = reverse_edges(analysis);
    let file_by_path = file_map(analysis);

    for (path, count) in top_reverse_imports(analysis, 80) {
        let Some(file) = file_by_path.get(&path) else {
            continue;
        };
        output.push_str(&format!("## `{path}`\n\n"));
        output.push_str(&format!("- Imported by: {count} local files\n"));
        output.push_str(&format!("- Tags:{}\n", display_file_tags_or_none(file)));
        output.push_str("- Read first:\n");
        output.push_str(&format!("  - `{path}`\n"));
        for imported in direct_imports(analysis, &path).into_iter().take(12) {
            output.push_str(&format!("  - `{imported}`\n"));
        }
        if let Some(importers) = reverse_edges.get(&path) {
            output.push_str("- Check direct dependents:\n");
            for importer in importers.iter().take(12) {
                output.push_str(&format!("  - `{importer}`\n"));
            }
        }
        let related_tests = related_tests(analysis, &path);
        if !related_tests.is_empty() {
            output.push_str("- Related tests:\n");
            for test in related_tests.into_iter().take(8) {
                output.push_str(&format!("  - `{test}`\n"));
            }
        }
        output.push_str("- Relevant intel: `graph.md`, `symbols.md`, `files.md`");
        if !file.routes.is_empty() {
            output.push_str(", `routes.md`");
        }
        if !file.api_endpoints.is_empty() || is_api_module(&file.normalized_path) {
            output.push_str(", `api.md`");
        }
        if file.extension == "sql" || is_data_file(&file.normalized_path) {
            output.push_str(", `data.md`");
        }
        if !file.env_vars.is_empty() {
            output.push_str(", `env.md`");
        }
        output.push_str("\n\n");
    }

    output
}

fn render_boundaries(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Runtime And Ownership Boundaries\n\n");
    output.push_str("These are heuristic boundary signals. Use them to avoid crossing client/server/data/generated lines accidentally.\n\n");

    for (title, predicate) in [
        ("Server And API Files", BoundaryKind::Server),
        ("Client And UI Files", BoundaryKind::Client),
        ("Data And Migration Files", BoundaryKind::Data),
        ("Generated Files", BoundaryKind::Generated),
        ("Environment-Touching Files", BoundaryKind::Env),
    ] {
        output.push_str(&format!("## {title}\n\n"));
        let mut any = false;
        for file in analysis
            .files
            .iter()
            .filter(|file| boundary_matches(file, predicate))
            .take(160)
        {
            output.push_str(&format!(
                "- `{}` — {} lines{}\n",
                file.normalized_path,
                file.line_count,
                display_file_tags(file)
            ));
            any = true;
        }
        if !any {
            output.push_str("- None detected.\n");
        }
        output.push('\n');
    }

    output
}

fn render_imports(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Local Imports\n\n");
    push_article_read_policy(&mut output, "imports.md");
    output.push_str("Local import adjacency grouped by source file. Use this to follow dependencies without scanning the project.\n\n");
    let mut by_from: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &analysis.local_edges {
        by_from
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
    }

    for (from, mut targets) in by_from {
        targets.sort();
        targets.dedup();
        output.push_str(&format!("## `{from}`\n\n"));
        for target in targets {
            output.push_str(&format!("- `{target}`\n"));
        }
        output.push('\n');
    }

    output
}

fn render_calls(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Call Sites\n\n");
    push_article_read_policy(&mut output, "calls.md");
    output.push_str("Function and method call names extracted from the JS/TS AST. Use this to find framework APIs, server helpers, route declarations, and shared utility usage before scanning source.\n\n");

    let mut usage: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in &analysis.files {
        for call in &file.call_sites {
            usage
                .entry(call.clone())
                .or_default()
                .push(file.normalized_path.clone());
        }
    }

    output.push_str("## Top Calls\n\n");
    let mut top: Vec<(String, Vec<String>)> = usage.into_iter().collect();
    top.sort_by(|left, right| {
        right
            .1
            .len()
            .cmp(&left.1.len())
            .then_with(|| left.0.cmp(&right.0))
    });
    for (call, files) in top.iter().take(80) {
        output.push_str(&format!("- `{call}` — {} files\n", files.len()));
    }

    output.push_str("\n## By File\n\n");
    for file in analysis
        .files
        .iter()
        .filter(|file| !file.call_sites.is_empty())
        .take(300)
    {
        output.push_str(&format!("### `{}`\n\n", file.normalized_path));
        for call in file.call_sites.iter().take(80) {
            output.push_str(&format!("- `{call}`\n"));
        }
        output.push('\n');
    }

    output
}

fn render_dependencies(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Dependencies\n\n");
    if let Some(package) = &analysis.package {
        output.push_str("## Package Manifest\n\n");
        if !package.dependencies.is_empty() {
            output.push_str("### Dependencies\n\n");
            for dependency in &package.dependencies {
                output.push_str(&format!("- `{dependency}`\n"));
            }
            output.push('\n');
        }
        if !package.dev_dependencies.is_empty() {
            output.push_str("### Dev Dependencies\n\n");
            for dependency in &package.dev_dependencies {
                output.push_str(&format!("- `{dependency}`\n"));
            }
            output.push('\n');
        }
    }

    output.push_str("## External Import Usage\n\n");
    let external = external_import_usage(analysis);
    if external.is_empty() {
        output.push_str("No external imports detected.\n");
        return output;
    }

    for (package, files) in external.into_iter().take(80) {
        output.push_str(&format!("- `{package}` — used by {} files\n", files.len()));
        for file in files.iter().take(8) {
            output.push_str(&format!("  - `{file}`\n"));
        }
    }

    output
}

fn render_symbols(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Exported Symbols\n\n");
    let mut files: Vec<&FileIntel> = analysis
        .files
        .iter()
        .filter(|file| !file.exports.is_empty())
        .collect();
    files.sort_by(|left, right| left.normalized_path.cmp(&right.normalized_path));

    if files.is_empty() {
        output.push_str("No exported symbols detected.\n");
        return output;
    }

    for file in files {
        output.push_str(&format!(
            "- `{}` — {}\n",
            file.normalized_path,
            file.exports.join(", ")
        ));
    }
    output
}

fn render_files(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# File Inventory\n\n");
    output.push_str("Every source-like file discovered by the repo intelligence scanner. Tags are heuristic and meant to guide read paths.\n\n");

    let mut by_area: BTreeMap<String, Vec<&FileIntel>> = BTreeMap::new();
    for file in &analysis.files {
        by_area.entry(area_key(&file.path)).or_default().push(file);
    }

    for (area, mut files) in by_area {
        files.sort_by(|left, right| left.normalized_path.cmp(&right.normalized_path));
        output.push_str(&format!("## `{area}`\n\n"));
        for file in files {
            output.push_str(&format!(
                "- `{}` — {} lines{}\n",
                file.normalized_path,
                file.line_count,
                display_file_tags(file)
            ));
        }
        output.push('\n');
    }

    output
}

fn render_env(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Environment Variables\n\n");
    let env = env_var_index(analysis);
    if env.is_empty() {
        output.push_str("No environment variable names detected in source.\n");
        return output;
    }

    for (var, files) in env {
        output.push_str(&format!("- `{var}`\n"));
        for file in files.iter().take(12) {
            output.push_str(&format!("  - `{file}`\n"));
        }
    }
    output
}

fn render_testing(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("# Testing\n\n");
    let tests: Vec<&FileIntel> = analysis.files.iter().filter(|file| file.is_test).collect();
    if tests.is_empty() {
        output.push_str("No test files detected.\n");
    } else {
        output.push_str(&format!("Detected {} test files.\n\n", tests.len()));
        for file in tests {
            output.push_str(&format!("- `{}`\n", file.normalized_path));
        }
    }

    output.push_str("\n## Test Scripts\n\n");
    if let Some(package) = &analysis.package {
        for script in package.scripts.iter().filter(|script| {
            script.contains("test")
                || script.contains("check")
                || script.contains("lint")
                || script.contains("format")
                || script.contains("type")
        }) {
            output.push_str(&format!("- `bun {script}`\n"));
        }
    }
    output
}

fn render_repo_json(analysis: &RepoAnalysis) -> String {
    let mut output = String::from("{\n");
    output.push_str("\t\"schemaVersion\": 5,\n");
    output.push_str(&format!("\t\"fileCount\": {},\n", analysis.files.len()));
    output.push_str(&format!(
        "\t\"frameworks\": {},\n",
        json_string_array(&analysis.frameworks)
    ));
    output.push_str(&format!("\t\"routeCount\": {},\n", count_routes(analysis)));
    output.push_str(&format!(
        "\t\"componentCount\": {},\n",
        count_components(analysis)
    ));
    output.push_str(&format!("\t\"apiCount\": {},\n", count_api(analysis)));
    output.push_str(&format!(
        "\t\"sqlObjectCount\": {},\n",
        count_sql_objects(analysis)
    ));
    output.push_str(&format!(
        "\t\"databaseSummary\": {},\n",
        render_database_summary_json(analysis)
    ));
    output.push_str(&format!(
        "\t\"envVarCount\": {},\n",
        env_var_index(analysis).len()
    ));
    output.push_str(&format!(
        "\t\"localImportEdgeCount\": {},\n",
        analysis.local_edges.len()
    ));
    output.push_str(&format!(
        "\t\"astParsedFileCount\": {},\n",
        analysis.files.iter().filter(|file| file.used_ast).count()
    ));
    output.push_str(&format!(
        "\t\"astParseErrorFileCount\": {},\n",
        analysis
            .files
            .iter()
            .filter(|file| file.parse_error_count > 0)
            .count()
    ));
    output.push_str(&format!(
        "\t\"pathAliases\": {},\n",
        render_path_aliases_json(&analysis.path_aliases)
    ));
    output.push_str(&format!(
        "\t\"packageScripts\": {},\n",
        json_string_array(
            &analysis
                .package
                .as_ref()
                .map(|package| package.scripts.clone())
                .unwrap_or_default()
        )
    ));
    output.push_str(&format!(
        "\t\"topAreas\": {},\n",
        render_json_pairs(top_areas(analysis, 40), "area", "fileCount")
    ));
    output.push_str(&format!(
        "\t\"highImpactFiles\": {},\n",
        render_json_pairs(top_reverse_imports(analysis, 80), "path", "importedBy")
    ));
    output.push_str(&format!(
        "\t\"localImportEdges\": {},\n",
        render_import_edges_json(analysis, 3000)
    ));
    output.push_str(&format!(
        "\t\"impactPlans\": {},\n",
        render_impact_json(analysis, 100)
    ));
    output.push_str(&format!(
        "\t\"callSites\": {},\n",
        render_call_sites_json(analysis, 500)
    ));
    output.push_str(&format!(
        "\t\"generatedFiles\": {},\n",
        json_string_array(
            &analysis
                .files
                .iter()
                .filter(|file| file.is_generated)
                .map(|file| file.normalized_path.clone())
                .collect::<Vec<_>>()
        )
    ));
    output.push_str(&format!(
        "\t\"testFiles\": {},\n",
        json_string_array(
            &analysis
                .files
                .iter()
                .filter(|file| file.is_test)
                .map(|file| file.normalized_path.clone())
                .collect::<Vec<_>>()
        )
    ));
    output.push_str(&format!(
        "\t\"envVars\": {},\n",
        json_string_array(&env_var_index(analysis).keys().cloned().collect::<Vec<_>>())
    ));
    output.push_str(&format!(
        "\t\"routes\": {},\n",
        render_routes_json(analysis, 200)
    ));
    output.push_str(&format!(
        "\t\"apiModules\": {},\n",
        render_api_json(analysis, 200)
    ));
    output.push_str(&format!(
        "\t\"components\": {},\n",
        render_components_json(analysis, 300)
    ));
    output.push_str(&format!(
        "\t\"sqlObjects\": {},\n",
        render_sql_json(analysis, 300)
    ));
    output.push_str(&format!(
        "\t\"databaseTables\": {},\n",
        render_database_tables_json(analysis, 200)
    ));
    output.push_str(&format!(
        "\t\"files\": {}\n",
        render_files_json(analysis, 1000)
    ));
    output.push_str("}\n");
    output
}

fn collect_source_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
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

            if is_source_file(&entry_path) {
                files.push(
                    entry_path
                        .strip_prefix(root)
                        .unwrap_or(&entry_path)
                        .to_path_buf(),
                );
            }
        }
    }

    files.sort();
    Ok(files)
}

fn should_skip_dir(name: &str) -> bool {
    if name.starts_with('.') && name != ".github" {
        return true;
    }

    matches!(
        name,
        ".git"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | ".nuxt"
            | ".svelte-kit"
            | ".turbo"
            | ".cache"
            | ".wrangler"
    )
}

fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(
            "ts" | "tsx"
                | "js"
                | "jsx"
                | "mjs"
                | "cjs"
                | "mts"
                | "cts"
                | "vue"
                | "svelte"
                | "astro"
                | "rs"
                | "go"
                | "py"
                | "sql"
                | "md"
                | "json"
                | "toml"
                | "yml"
                | "yaml"
        )
    )
}

fn parse_package_info(contents: &str) -> PackageInfo {
    PackageInfo {
        scripts: filter_external_intel_tooling(extract_object_keys(contents, "scripts")),
        dependencies: filter_external_intel_tooling(extract_object_keys(contents, "dependencies")),
        dev_dependencies: filter_external_intel_tooling(extract_object_keys(
            contents,
            "devDependencies",
        )),
    }
}

fn filter_external_intel_tooling(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .filter(|item| {
            let lower = item.to_ascii_lowercase();
            !(lower.contains("code") && lower.contains("sight"))
        })
        .collect()
}

fn parse_path_aliases(root: &Path) -> Vec<PathAlias> {
    let mut aliases = Vec::new();

    if let Ok(tsconfig) = fs::read_to_string(root.join("tsconfig.json")) {
        for (alias, targets) in extract_tsconfig_paths(&tsconfig) {
            if let Some(target) = targets.first() {
                aliases.push(PathAlias {
                    prefix: normalize_alias_prefix(&alias),
                    target_prefix: normalize_alias_target(target),
                });
            }
        }
    }

    if root.join("src").exists() {
        for prefix in ["@/", "~/"] {
            if !aliases.iter().any(|alias| alias.prefix == prefix) {
                aliases.push(PathAlias {
                    prefix: prefix.to_string(),
                    target_prefix: "src/".to_string(),
                });
            }
        }
    }

    aliases.sort_by(|left, right| left.prefix.cmp(&right.prefix));
    aliases.dedup_by(|left, right| left.prefix == right.prefix);
    aliases
}

fn extract_tsconfig_paths(contents: &str) -> Vec<(String, Vec<String>)> {
    let Some(paths_index) = contents.find("\"paths\"") else {
        return Vec::new();
    };
    let Some(open_offset) = contents[paths_index..].find('{') else {
        return Vec::new();
    };
    let object = &contents[paths_index + open_offset..];
    let Some(object_body) = balanced_object_body(object) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    let mut remaining = object_body.as_str();

    while let Some((key, rest)) = parse_next_json_key(remaining) {
        let Some(colon_index) = rest.find(':') else {
            break;
        };
        let value_rest = &rest[colon_index + 1..];
        let values = extract_json_string_array(value_rest);
        if !values.is_empty() {
            entries.push((key, values));
        }
        let Some(array_end) = value_rest.find(']') else {
            break;
        };
        remaining = &value_rest[array_end + 1..];
    }

    entries
}

fn balanced_object_body(raw: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut body = String::new();
    let mut started = false;

    for character in raw.chars() {
        if in_string {
            body.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        match character {
            '"' => {
                in_string = true;
                if started {
                    body.push(character);
                }
            }
            '{' => {
                depth += 1;
                if started {
                    body.push(character);
                }
                started = true;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(body);
                }
                body.push(character);
            }
            _ if started => body.push(character),
            _ => {}
        }
    }

    None
}

fn parse_next_json_key(raw: &str) -> Option<(String, &str)> {
    let (_, rest) = raw.split_once('"')?;
    let (key, rest) = rest.split_once('"')?;
    Some((key.to_string(), rest))
}

fn extract_json_string_array(raw: &str) -> Vec<String> {
    let Some(start) = raw.find('[') else {
        return Vec::new();
    };
    let Some(end) = raw[start..].find(']') else {
        return Vec::new();
    };
    raw[start + 1..start + end]
        .split(',')
        .filter_map(|part| {
            let part = part.trim().trim_matches('"').trim_matches('\'');
            if part.is_empty() {
                None
            } else {
                Some(part.to_string())
            }
        })
        .collect()
}

fn normalize_alias_prefix(alias: &str) -> String {
    alias.trim_end_matches('*').to_string()
}

fn normalize_alias_target(target: &str) -> String {
    let target = target
        .trim_start_matches("./")
        .trim_end_matches('*')
        .trim_start_matches('/');
    if target.is_empty() {
        String::new()
    } else if target.ends_with('/') {
        target.to_string()
    } else {
        format!("{target}/")
    }
}

fn extract_ast_file_intel(path: &Path, contents: &str, extension: &str) -> Option<AstFileIntel> {
    if !is_ast_parseable_js_extension(extension) {
        return None;
    }

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).ok()?;
    let parsed = Parser::new(&allocator, contents, source_type)
        .with_options(ParseOptions {
            parse_regular_expression: true,
            ..ParseOptions::default()
        })
        .parse();
    if parsed.panicked {
        return None;
    }

    let mut collector = AstCollector::default();
    collector.visit_program(&parsed.program);
    let mut intel = collector.finish();
    intel.parse_error_count = parsed.errors.len();
    Some(intel)
}

fn is_ast_parseable_js_extension(extension: &str) -> bool {
    matches!(
        extension,
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts"
    )
}

#[derive(Default)]
struct AstCollector {
    imports: BTreeSet<String>,
    exports: BTreeSet<String>,
    component_names: BTreeSet<String>,
    env_vars: BTreeSet<String>,
    call_sites: BTreeSet<String>,
    route_declarations: BTreeSet<String>,
    http_routes: BTreeSet<(String, String)>,
}

impl AstCollector {
    fn finish(self) -> AstFileIntel {
        AstFileIntel {
            imports: self.imports.into_iter().collect(),
            exports: self.exports.into_iter().take(32).collect(),
            component_names: self.component_names.into_iter().collect(),
            env_vars: self.env_vars.into_iter().take(80).collect(),
            call_sites: self.call_sites.into_iter().take(160).collect(),
            route_declarations: self.route_declarations.into_iter().collect(),
            http_routes: self
                .http_routes
                .into_iter()
                .map(|(method, route)| ApiEndpoint { method, route })
                .collect(),
            parse_error_count: 0,
        }
    }

    fn record_exported_declaration<'a>(&mut self, declaration: &Declaration<'a>) {
        match declaration {
            Declaration::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    for name in binding_names(&declarator.id) {
                        self.exports.insert(name);
                    }
                }
            }
            Declaration::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    self.exports.insert(id.name.to_string());
                }
            }
            Declaration::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    self.exports.insert(id.name.to_string());
                }
            }
            Declaration::TSTypeAliasDeclaration(declaration) => {
                self.exports.insert(declaration.id.name.to_string());
            }
            Declaration::TSInterfaceDeclaration(declaration) => {
                self.exports.insert(declaration.id.name.to_string());
            }
            Declaration::TSEnumDeclaration(declaration) => {
                self.exports.insert(declaration.id.name.to_string());
            }
            Declaration::TSModuleDeclaration(declaration) => {
                self.exports.insert(declaration.id.name().to_string());
            }
            Declaration::TSGlobalDeclaration(_) | Declaration::TSImportEqualsDeclaration(_) => {}
        }
    }
}

impl<'a> Visit<'a> for AstCollector {
    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        self.imports.insert(declaration.source.value.to_string());
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_import_expression(&mut self, expression: &ImportExpression<'a>) {
        if let Expression::StringLiteral(source) = &expression.source {
            self.imports.insert(source.value.to_string());
        }
        walk::walk_import_expression(self, expression);
    }

    fn visit_module_declaration(&mut self, declaration: &ModuleDeclaration<'a>) {
        match declaration {
            ModuleDeclaration::ExportAllDeclaration(declaration) => {
                self.imports.insert(declaration.source.value.to_string());
                if let Some(exported) = &declaration.exported {
                    self.exports.insert(module_export_name(exported));
                }
            }
            ModuleDeclaration::ExportNamedDeclaration(declaration) => {
                if let Some(source) = &declaration.source {
                    self.imports.insert(source.value.to_string());
                }
                if let Some(inner) = &declaration.declaration {
                    self.record_exported_declaration(inner);
                }
                for specifier in &declaration.specifiers {
                    self.exports.insert(module_export_name(&specifier.exported));
                }
            }
            ModuleDeclaration::ExportDefaultDeclaration(declaration) => {
                self.exports.insert(default_export_name(declaration));
            }
            ModuleDeclaration::ImportDeclaration(_)
            | ModuleDeclaration::TSExportAssignment(_)
            | ModuleDeclaration::TSNamespaceExportDeclaration(_) => {}
        }
        walk::walk_module_declaration(self, declaration);
    }

    fn visit_declaration(&mut self, declaration: &Declaration<'a>) {
        match declaration {
            Declaration::FunctionDeclaration(function) => record_function_component(function, self),
            Declaration::ClassDeclaration(class) => record_class_component(class, self),
            Declaration::VariableDeclaration(declaration) => {
                record_variable_components(declaration, self);
            }
            _ => {}
        }
        walk::walk_declaration(self, declaration);
    }

    fn visit_call_expression(&mut self, expression: &CallExpression<'a>) {
        if let Some(name) = expression_name(&expression.callee) {
            self.call_sites.insert(name.clone());
            if matches!(name.as_str(), "createFileRoute" | "createLazyFileRoute") {
                if let Some(route) = first_string_argument(&expression.arguments) {
                    self.route_declarations.insert(route);
                }
            }
            if let Some((object, method)) = name.rsplit_once('.') {
                if matches!(object, "app" | "router" | "server")
                    && matches!(
                        method,
                        "get" | "post" | "put" | "patch" | "delete" | "options" | "head" | "all"
                    )
                {
                    if let Some(route) = first_string_argument(&expression.arguments) {
                        self.http_routes
                            .insert((method.to_ascii_uppercase(), route));
                    }
                }
            }
        }
        walk::walk_call_expression(self, expression);
    }

    fn visit_static_member_expression(&mut self, expression: &StaticMemberExpression<'a>) {
        if let Some(object) = expression_name(&expression.object) {
            if matches!(object.as_str(), "process.env" | "import.meta.env") {
                self.env_vars.insert(expression.property.name.to_string());
            }
        }
        walk::walk_static_member_expression(self, expression);
    }
}

fn module_export_name(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
    }
}

fn default_export_name(declaration: &ExportDefaultDeclaration<'_>) -> String {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => function
            .id
            .as_ref()
            .map(|id| id.name.to_string())
            .unwrap_or_else(|| "default".to_string()),
        ExportDefaultDeclarationKind::ClassDeclaration(class) => class
            .id
            .as_ref()
            .map(|id| id.name.to_string())
            .unwrap_or_else(|| "default".to_string()),
        _ => "default".to_string(),
    }
}

fn record_function_component(function: &Function<'_>, collector: &mut AstCollector) {
    if let Some(id) = &function.id {
        let name = id.name.to_string();
        if is_component_name(&name) {
            collector.component_names.insert(name);
        }
    }
}

fn record_class_component(class: &Class<'_>, collector: &mut AstCollector) {
    if let Some(id) = &class.id {
        let name = id.name.to_string();
        if is_component_name(&name) {
            collector.component_names.insert(name);
        }
    }
}

fn record_variable_components(declaration: &VariableDeclaration<'_>, collector: &mut AstCollector) {
    for declarator in &declaration.declarations {
        let names = binding_names(&declarator.id);
        if let Some(init) = &declarator.init {
            let component_like_init = expression_is_component_like(init);
            for name in names {
                if is_component_name(&name) && component_like_init {
                    collector.component_names.insert(name);
                }
            }
        }
    }
}

fn binding_names(pattern: &oxc_ast::ast::BindingPattern<'_>) -> Vec<String> {
    match pattern {
        oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) => {
            vec![identifier.name.to_string()]
        }
        oxc_ast::ast::BindingPattern::ObjectPattern(pattern) => pattern
            .properties
            .iter()
            .flat_map(|property| binding_names(&property.value))
            .chain(
                pattern
                    .rest
                    .iter()
                    .flat_map(|rest| binding_names(&rest.argument)),
            )
            .collect(),
        oxc_ast::ast::BindingPattern::ArrayPattern(pattern) => pattern
            .elements
            .iter()
            .flatten()
            .flat_map(binding_names)
            .chain(
                pattern
                    .rest
                    .iter()
                    .flat_map(|rest| binding_names(&rest.argument)),
            )
            .collect(),
        oxc_ast::ast::BindingPattern::AssignmentPattern(pattern) => binding_names(&pattern.left),
    }
}

fn expression_is_component_like(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => true,
        Expression::CallExpression(call) => expression_name(&call.callee)
            .map(|name| {
                matches!(
                    name.as_str(),
                    "forwardRef" | "memo" | "React.forwardRef" | "React.memo"
                )
            })
            .unwrap_or(false),
        _ => false,
    }
}

fn expression_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::MetaProperty(meta) => {
            Some(format!("{}.{}", meta.meta.name, meta.property.name))
        }
        Expression::StaticMemberExpression(member) => {
            let object = expression_name(&member.object)?;
            Some(format!("{}.{}", object, member.property.name))
        }
        Expression::ParenthesizedExpression(expression) => expression_name(&expression.expression),
        Expression::TSAsExpression(expression) => expression_name(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => expression_name(&expression.expression),
        Expression::TSNonNullExpression(expression) => expression_name(&expression.expression),
        Expression::TSInstantiationExpression(expression) => {
            expression_name(&expression.expression)
        }
        _ => None,
    }
}

fn first_string_argument(arguments: &[Argument<'_>]) -> Option<String> {
    arguments.first().and_then(|argument| match argument {
        Argument::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    })
}

fn components_from_ast_names(names: &[String], contents: &str) -> Vec<ComponentIntel> {
    let props_by_type = extract_props_by_type(contents);
    names
        .iter()
        .map(|name| ComponentIntel {
            name: name.clone(),
            props: props_by_type
                .get(&format!("{name}Props"))
                .cloned()
                .unwrap_or_default(),
        })
        .collect()
}

fn routes_from_ast(routes: &[String]) -> Vec<RouteIntel> {
    routes
        .iter()
        .map(|route| RouteIntel {
            framework: "TanStack Router".to_string(),
            route: route.clone(),
            kind: "declared route".to_string(),
        })
        .collect()
}

fn merge_strings(primary: Vec<String>, fallback: Vec<String>, limit: usize) -> Vec<String> {
    let mut set = BTreeSet::new();
    for item in primary.into_iter().chain(fallback) {
        if !item.is_empty() {
            set.insert(item);
        }
    }
    set.into_iter().take(limit).collect()
}

fn merge_components(
    primary: Vec<ComponentIntel>,
    fallback: Vec<ComponentIntel>,
) -> Vec<ComponentIntel> {
    let mut by_name: BTreeMap<String, ComponentIntel> = BTreeMap::new();
    for component in primary.into_iter().chain(fallback) {
        by_name
            .entry(component.name.clone())
            .and_modify(|existing| {
                if existing.props.is_empty() && !component.props.is_empty() {
                    existing.props = component.props.clone();
                }
            })
            .or_insert(component);
    }
    by_name.into_values().collect()
}

fn merge_routes(primary: Vec<RouteIntel>, fallback: Vec<RouteIntel>) -> Vec<RouteIntel> {
    let mut seen = BTreeSet::new();
    let mut routes = Vec::new();
    for route in primary.into_iter().chain(fallback) {
        let key = format!("{}:{}:{}", route.framework, route.route, route.kind);
        if seen.insert(key) {
            routes.push(route);
        }
    }
    routes
}

fn merge_api_endpoints(primary: Vec<ApiEndpoint>, fallback: Vec<ApiEndpoint>) -> Vec<ApiEndpoint> {
    let mut seen = BTreeSet::new();
    let mut endpoints = Vec::new();
    for endpoint in primary.into_iter().chain(fallback) {
        let key = format!("{}:{}", endpoint.method, endpoint.route);
        if seen.insert(key) {
            endpoints.push(endpoint);
        }
    }
    endpoints
}

fn detect_frameworks(
    root: &Path,
    package: Option<&PackageInfo>,
    files: &[FileIntel],
) -> Vec<String> {
    let mut frameworks = BTreeSet::new();
    let deps: BTreeSet<&str> = package
        .map(|package| {
            package
                .dependencies
                .iter()
                .chain(package.dev_dependencies.iter())
                .map(String::as_str)
                .collect()
        })
        .unwrap_or_default();

    for (dependency, framework) in [
        ("next", "Next.js"),
        ("@tanstack/react-start", "TanStack Start"),
        ("@tanstack/react-router", "TanStack Router"),
        ("@remix-run/react", "Remix"),
        ("astro", "Astro"),
        ("@sveltejs/kit", "SvelteKit"),
        ("nuxt", "Nuxt"),
        ("vue", "Vue"),
        ("react", "React"),
        ("solid-js", "Solid"),
        ("@angular/core", "Angular"),
        ("vite", "Vite"),
        ("hono", "Hono"),
        ("express", "Express"),
        ("fastify", "Fastify"),
        ("@trpc/server", "tRPC"),
        ("@supabase/supabase-js", "Supabase"),
        ("prisma", "Prisma"),
        ("drizzle-orm", "Drizzle"),
        ("wrangler", "Cloudflare Workers"),
    ] {
        if deps.contains(dependency) {
            frameworks.insert(framework.to_string());
        }
    }

    for (path, framework) in [
        ("next.config.js", "Next.js"),
        ("next.config.ts", "Next.js"),
        ("remix.config.js", "Remix"),
        ("astro.config.mjs", "Astro"),
        ("svelte.config.js", "SvelteKit"),
        ("nuxt.config.ts", "Nuxt"),
        ("vite.config.ts", "Vite"),
        ("wrangler.toml", "Cloudflare Workers"),
        ("supabase/config.toml", "Supabase"),
    ] {
        if root.join(path).exists() {
            frameworks.insert(framework.to_string());
        }
    }

    if files
        .iter()
        .any(|file| file.normalized_path.starts_with("src/routes/"))
    {
        frameworks.insert("File-based routes".to_string());
    }

    frameworks.into_iter().collect()
}

fn extract_imports(contents: &str, extension: &str) -> Vec<String> {
    let mut imports = Vec::new();
    match extension {
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => {
            for statement in javascript_like_statements(contents) {
                let statement = statement.trim();
                if statement.starts_with("import ")
                    || statement.starts_with("export ") && statement.contains(" from ")
                {
                    if let Some(specifier) = extract_quoted_specifier(statement) {
                        imports.push(specifier);
                    }
                }
                if statement.contains("import(") {
                    if let Some(specifier) = extract_quoted_specifier(statement) {
                        imports.push(specifier);
                    }
                }
            }
        }
        "rs" => {
            for line in contents.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("use ") {
                    imports.push(rest.trim_end_matches(';').to_string());
                }
            }
        }
        _ => {}
    }
    imports.sort();
    imports.dedup();
    imports
}

fn javascript_like_statements(contents: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        if current.is_empty()
            && !(trimmed.starts_with("import ")
                || trimmed.starts_with("export ")
                || trimmed.contains("import("))
        {
            continue;
        }

        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(trimmed);

        if trimmed.ends_with(';')
            || trimmed.ends_with(");")
            || trimmed.starts_with("import ") && trimmed.contains(" from ")
            || trimmed.starts_with("export ") && trimmed.contains(" from ")
            || trimmed.contains("import(") && trimmed.contains(')')
        {
            statements.push(current.clone());
            current.clear();
        }
    }

    if !current.is_empty() {
        statements.push(current);
    }

    statements
}

fn extract_exports(contents: &str, extension: &str) -> Vec<String> {
    let mut exports = Vec::new();
    for line in contents.lines() {
        let line = line.trim_start();
        match extension {
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "mts" | "cts" | "vue" | "svelte" | "astro" => {
                extract_typescript_export(line, &mut exports)
            }
            "rs" => extract_rust_export(line, &mut exports),
            _ => {}
        }
        if exports.len() >= 16 {
            break;
        }
    }
    exports
}

fn extract_components(contents: &str, extension: &str) -> Vec<ComponentIntel> {
    if !matches!(extension, "tsx" | "jsx" | "vue" | "svelte" | "astro") {
        return Vec::new();
    }

    let props_by_type = extract_props_by_type(contents);
    let mut components = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim_start();
        for prefix in ["export function ", "function "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name = read_symbol(rest);
                if is_component_name(&name) {
                    components.push(ComponentIntel {
                        props: component_props(&name, rest, &props_by_type),
                        name,
                    });
                }
            }
        }
        for prefix in ["export const ", "const "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name = read_symbol(rest);
                if is_component_name(&name)
                    && (rest.contains("=>")
                        || rest.contains("forwardRef")
                        || rest.contains("memo(")
                        || rest.contains("React."))
                {
                    components.push(ComponentIntel {
                        props: component_props(&name, rest, &props_by_type),
                        name,
                    });
                }
            }
        }
    }

    components.sort_by(|left, right| left.name.cmp(&right.name));
    components.dedup_by(|left, right| left.name == right.name);
    components
}

fn detect_routes(path: &str, contents: &str) -> Vec<RouteIntel> {
    let mut routes = Vec::new();

    if path.starts_with("src/routes/") || path.starts_with("app/routes/") {
        routes.push(RouteIntel {
            framework: if path.starts_with("src/routes/") {
                "TanStack/File Routes".to_string()
            } else {
                "Remix".to_string()
            },
            route: route_from_file_path(path),
            kind: "route file".to_string(),
        });
    }
    if path.starts_with("app/") && path.ends_with("/page.tsx") {
        routes.push(RouteIntel {
            framework: "Next.js App Router".to_string(),
            route: next_app_route(path, "/page.tsx"),
            kind: "page".to_string(),
        });
    }
    if path.starts_with("app/") && path.ends_with("/route.ts") {
        routes.push(RouteIntel {
            framework: "Next.js App Router".to_string(),
            route: next_app_route(path, "/route.ts"),
            kind: "route handler".to_string(),
        });
    }
    if path.starts_with("pages/") && matches_route_extension(path) {
        routes.push(RouteIntel {
            framework: "Next.js Pages Router".to_string(),
            route: pages_route(path, "pages/"),
            kind: if path.starts_with("pages/api/") {
                "api route".to_string()
            } else {
                "page".to_string()
            },
        });
    }
    if path.starts_with("src/pages/") && matches_route_extension(path) {
        routes.push(RouteIntel {
            framework: "Astro".to_string(),
            route: pages_route(path, "src/pages/"),
            kind: "page".to_string(),
        });
    }
    if path.starts_with("src/routes/")
        && (path.ends_with("+page.svelte") || path.ends_with("+server.ts"))
    {
        routes.push(RouteIntel {
            framework: "SvelteKit".to_string(),
            route: sveltekit_route(path),
            kind: if path.ends_with("+server.ts") {
                "server route".to_string()
            } else {
                "page".to_string()
            },
        });
    }

    if contents.contains("createFileRoute(") || contents.contains("createRootRoute(") {
        let route = extract_route_literal(contents).unwrap_or_else(|| route_from_file_path(path));
        routes.push(RouteIntel {
            framework: "TanStack Router".to_string(),
            route,
            kind: "declared route".to_string(),
        });
    }

    routes
}

fn detect_api_endpoints(path: &str, contents: &str, exports: &[String]) -> Vec<ApiEndpoint> {
    let mut endpoints = Vec::new();
    for method in ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"] {
        if exports.iter().any(|export| export == method)
            || contents.contains(&format!("export const {method}"))
            || contents.contains(&format!("export async function {method}"))
            || contents.contains(&format!("export function {method}"))
        {
            endpoints.push(ApiEndpoint {
                method: method.to_string(),
                route: route_from_file_path(path),
            });
        }
    }

    for method in ["get", "post", "put", "patch", "delete", "options", "all"] {
        for line in contents.lines() {
            let line = line.trim();
            for prefix in [
                &format!("app.{method}("),
                &format!("router.{method}("),
                &format!("server.{method}("),
            ] {
                if line.contains(prefix) {
                    if let Some(route) = extract_quoted_specifier(line) {
                        endpoints.push(ApiEndpoint {
                            method: method.to_uppercase(),
                            route,
                        });
                    }
                }
            }
        }
    }

    if endpoints.is_empty() && is_api_module(path) {
        endpoints.push(ApiEndpoint {
            method: "MODULE".to_string(),
            route: path.to_string(),
        });
    }

    endpoints
}

fn extract_sql_objects(contents: &str, extension: &str) -> Vec<SqlObject> {
    if extension != "sql" {
        return Vec::new();
    }
    let mut objects = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        let lower = line.to_ascii_lowercase();
        for (needle, kind) in [
            ("create table", "table"),
            ("create or replace function", "function"),
            ("create function", "function"),
            ("create policy", "policy"),
            ("create index", "index"),
            ("create trigger", "trigger"),
            ("create view", "view"),
        ] {
            if lower.starts_with(needle) {
                let name = line
                    .split_whitespace()
                    .skip(needle.split_whitespace().count())
                    .find(|part| {
                        !matches!(part.to_ascii_lowercase().as_str(), "if" | "not" | "exists")
                    })
                    .unwrap_or("")
                    .trim_matches(|character: char| {
                        character == '"' || character == '(' || character == ';'
                    })
                    .to_string();
                if !name.is_empty() {
                    objects.push(SqlObject {
                        kind: kind.to_string(),
                        name,
                    });
                }
            }
        }
        if objects.len() >= 20 {
            break;
        }
    }
    objects
}

fn extract_sql_tables(contents: &str, extension: &str) -> Vec<SqlTable> {
    if extension != "sql" {
        return Vec::new();
    }
    if let Some(tables) = extract_sql_tables_from_pg_query(contents) {
        return tables;
    }
    extract_sql_tables_manual(contents)
}

fn extract_sql_tables_manual(contents: &str) -> Vec<SqlTable> {
    let mut tables = Vec::new();
    let lower = contents.to_ascii_lowercase();
    let mut offset = 0usize;

    while let Some(index) = lower[offset..].find("create table") {
        let start = offset + index;
        let statement = &contents[start..];
        let Some(open_paren) = statement.find('(') else {
            offset = start + "create table".len();
            continue;
        };
        let name = normalize_sql_name(
            statement["create table".len()..open_paren]
                .split_whitespace()
                .filter(|part| {
                    !matches!(part.to_ascii_lowercase().as_str(), "if" | "not" | "exists")
                })
                .collect::<Vec<_>>()
                .first()
                .copied()
                .unwrap_or(""),
        );
        let Some(close_paren) = find_matching_paren(statement, open_paren) else {
            offset = start + open_paren + 1;
            continue;
        };
        let body = &statement[open_paren + 1..close_paren];
        if !name.is_empty() {
            let mut table = SqlTable {
                name,
                columns: extract_sql_columns(body),
            };
            apply_table_level_references(body, &mut table);
            tables.push(table);
        }
        offset = start + close_paren + 1;
    }

    tables
}

#[derive(Debug, Clone)]
struct PgParsedStatement {
    node: pg_query::protobuf::Node,
    line: usize,
    text: String,
}

fn extract_sql_tables_from_pg_query(contents: &str) -> Option<Vec<SqlTable>> {
    let statements = pg_parsed_statements(contents)?;
    let mut tables = Vec::new();
    for statement in statements {
        if let Some(pg_query::protobuf::node::Node::CreateStmt(create)) =
            statement.node.node.as_ref()
        {
            let name = pg_relation_name(create.relation.as_ref());
            if name.is_empty() {
                continue;
            }
            let mut table = SqlTable {
                name,
                columns: Vec::new(),
            };
            for element in &create.table_elts {
                match element.node.as_ref() {
                    Some(pg_query::protobuf::node::Node::ColumnDef(column)) => {
                        table.columns.push(pg_column_def(column));
                    }
                    Some(pg_query::protobuf::node::Node::Constraint(constraint)) => {
                        apply_pg_table_constraint(constraint, &mut table);
                    }
                    _ => {}
                }
            }
            for constraint in &create.constraints {
                if let Some(pg_query::protobuf::node::Node::Constraint(constraint)) =
                    constraint.node.as_ref()
                {
                    apply_pg_table_constraint(constraint, &mut table);
                }
            }
            tables.push(table);
        }
    }
    Some(tables)
}

fn extract_sql_columns(body: &str) -> Vec<SqlColumn> {
    split_sql_top_level(body, ',')
        .into_iter()
        .filter_map(|part| {
            let part = strip_sql_comment_lines(&part);
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let lower = part.to_ascii_lowercase();
            if [
                "constraint",
                "primary key",
                "foreign key",
                "unique",
                "check",
                "exclude",
            ]
            .iter()
            .any(|prefix| lower.starts_with(prefix))
            {
                return None;
            }
            let mut pieces = part.split_whitespace();
            let name = normalize_sql_name(pieces.next().unwrap_or(""));
            if name.is_empty() {
                return None;
            }
            let rest = pieces.collect::<Vec<_>>().join(" ");
            let data_type = sql_column_type(&rest);
            let mut flags = Vec::new();
            if lower.contains("primary key") {
                flags.push("pk".to_string());
            }
            if lower.contains("not null") {
                flags.push("required".to_string());
            }
            if lower.contains("unique") {
                flags.push("unique".to_string());
            }
            if lower.contains("default ") {
                flags.push("default".to_string());
            }
            Some(SqlColumn {
                name,
                data_type,
                flags,
                references: sql_reference_target(part),
            })
        })
        .collect()
}

fn apply_table_level_references(body: &str, table: &mut SqlTable) {
    for part in split_sql_top_level(body, ',') {
        let part = strip_sql_comment_lines(&part);
        let lower = part.to_ascii_lowercase();
        if !lower.contains("foreign key") || !lower.contains("references") {
            continue;
        }
        let Some(target) = sql_reference_target(&part) else {
            continue;
        };
        let Some(open) = lower.find("foreign key") else {
            continue;
        };
        let after = &part[open + "foreign key".len()..];
        let Some(columns_open) = after.find('(') else {
            continue;
        };
        let Some(columns_close) = after[columns_open..].find(')') else {
            continue;
        };
        for column_name in after[columns_open + 1..columns_open + columns_close].split(',') {
            let column_name = normalize_sql_name(column_name);
            if let Some(column) = table
                .columns
                .iter_mut()
                .find(|column| column.name == column_name)
            {
                column.references = Some(target.clone());
            }
        }
    }
}

fn pg_parsed_statements(contents: &str) -> Option<Vec<PgParsedStatement>> {
    if let Ok(parsed) = pg_query::parse(contents) {
        let statements: Vec<PgParsedStatement> = parsed
            .protobuf
            .stmts
            .into_iter()
            .filter_map(|statement| {
                let node = statement.stmt.map(|node| *node)?;
                let text = pg_statement_text(
                    contents,
                    statement.stmt_location.max(0) as usize,
                    statement.stmt_len.max(0) as usize,
                );
                Some(PgParsedStatement {
                    node,
                    line: line_number_for_offset(contents, statement.stmt_location.max(0) as usize),
                    text,
                })
            })
            .collect();
        if !statements.is_empty() {
            return Some(statements);
        }
    }

    let mut statements = Vec::new();
    for (statement, line) in split_sql_statements(contents) {
        if let Ok(parsed) = pg_query::parse(&statement) {
            for raw in parsed.protobuf.stmts {
                if let Some(node) = raw.stmt {
                    statements.push(PgParsedStatement {
                        node: *node,
                        line,
                        text: statement.clone(),
                    });
                }
            }
        }
    }
    (!statements.is_empty()).then_some(statements)
}

fn pg_statement_text(contents: &str, start: usize, len: usize) -> String {
    if len == 0 || start >= contents.len() {
        return String::new();
    }
    let end = start.saturating_add(len).min(contents.len());
    contents
        .get(start..end)
        .unwrap_or_default()
        .trim()
        .trim_end_matches(';')
        .to_string()
}

fn line_number_for_offset(contents: &str, offset: usize) -> usize {
    contents
        .get(..offset.min(contents.len()))
        .unwrap_or_default()
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn pg_relation_name(relation: Option<&pg_query::protobuf::RangeVar>) -> String {
    let Some(relation) = relation else {
        return String::new();
    };
    if relation.relname.is_empty() {
        String::new()
    } else if relation.schemaname.is_empty() {
        relation.relname.clone()
    } else {
        format!("{}.{}", relation.schemaname, relation.relname)
    }
}

fn pg_column_def(column: &pg_query::protobuf::ColumnDef) -> SqlColumn {
    let mut flags = Vec::new();
    if column.is_not_null {
        push_unique(&mut flags, "required".to_string());
    }
    if column.raw_default.is_some() {
        push_unique(&mut flags, "default".to_string());
    }
    let mut references = None;
    for constraint in &column.constraints {
        if let Some(pg_query::protobuf::node::Node::Constraint(constraint)) =
            constraint.node.as_ref()
        {
            apply_pg_column_constraint(constraint, &mut flags, &mut references);
        }
    }
    SqlColumn {
        name: column.colname.clone(),
        data_type: pg_type_name(column.type_name.as_ref()),
        flags,
        references,
    }
}

fn apply_pg_column_constraint(
    constraint: &pg_query::protobuf::Constraint,
    flags: &mut Vec<String>,
    references: &mut Option<String>,
) {
    match pg_query::protobuf::ConstrType::try_from(constraint.contype).ok() {
        Some(pg_query::protobuf::ConstrType::ConstrPrimary) => {
            push_unique(flags, "pk".to_string());
        }
        Some(pg_query::protobuf::ConstrType::ConstrUnique) => {
            push_unique(flags, "unique".to_string());
        }
        Some(pg_query::protobuf::ConstrType::ConstrNotnull) => {
            push_unique(flags, "required".to_string());
        }
        Some(pg_query::protobuf::ConstrType::ConstrDefault) => {
            push_unique(flags, "default".to_string());
        }
        Some(pg_query::protobuf::ConstrType::ConstrForeign) => {
            *references = Some(pg_relation_name(constraint.pktable.as_ref()));
        }
        _ => {}
    }
}

fn apply_pg_table_constraint(constraint: &pg_query::protobuf::Constraint, table: &mut SqlTable) {
    match pg_query::protobuf::ConstrType::try_from(constraint.contype).ok() {
        Some(pg_query::protobuf::ConstrType::ConstrPrimary) => {
            for key in pg_node_names(&constraint.keys) {
                if let Some(column) = table.columns.iter_mut().find(|column| column.name == key) {
                    push_unique(&mut column.flags, "pk".to_string());
                }
            }
        }
        Some(pg_query::protobuf::ConstrType::ConstrUnique) => {
            for key in pg_node_names(&constraint.keys) {
                if let Some(column) = table.columns.iter_mut().find(|column| column.name == key) {
                    push_unique(&mut column.flags, "unique".to_string());
                }
            }
        }
        Some(pg_query::protobuf::ConstrType::ConstrForeign) => {
            let target = pg_relation_name(constraint.pktable.as_ref());
            if target.is_empty() {
                return;
            }
            for key in pg_node_names(&constraint.fk_attrs) {
                if let Some(column) = table.columns.iter_mut().find(|column| column.name == key) {
                    column.references = Some(target.clone());
                }
            }
        }
        _ => {}
    }
}

fn pg_type_name(type_name: Option<&pg_query::protobuf::TypeName>) -> String {
    let Some(type_name) = type_name else {
        return String::new();
    };
    let mut name = pg_node_name(&type_name.names);
    if !name.is_empty() && !type_name.array_bounds.is_empty() {
        name.push_str("[]");
    }
    name
}

fn pg_node_name(nodes: &[pg_query::protobuf::Node]) -> String {
    pg_node_names(nodes).join(".")
}

fn pg_node_names(nodes: &[pg_query::protobuf::Node]) -> Vec<String> {
    nodes
        .iter()
        .filter_map(|node| match node.node.as_ref() {
            Some(pg_query::protobuf::node::Node::String(value)) => Some(value.sval.clone()),
            _ => None,
        })
        .collect()
}

fn strip_sql_comment_lines(part: &str) -> String {
    part.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("--"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_sql_statements(contents: &str) -> Vec<(String, usize)> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut statement_line = 1usize;
    let mut line_number = 1usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut dollar_quote: Option<String> = None;
    let mut previous = '\0';
    let mut chars = contents.chars().peekable();

    while let Some(character) = chars.next() {
        if current.trim().is_empty() && !character.is_whitespace() {
            statement_line = line_number;
        }

        if character == '\n' {
            line_number += 1;
        }

        if let Some(tag) = &dollar_quote {
            current.push(character);
            if character == '$' && current.ends_with(tag) {
                dollar_quote = None;
            }
            previous = character;
            continue;
        }

        if character == '$' && !in_single && !in_double {
            let mut tag = String::from("$");
            let mut lookahead = chars.clone();
            while let Some(next) = lookahead.next() {
                tag.push(next);
                if next == '$' {
                    dollar_quote = Some(tag.clone());
                    break;
                }
                if !(next == '_' || next.is_ascii_alphanumeric()) {
                    break;
                }
            }
            if dollar_quote.is_some() {
                current.push_str(&tag);
                for _ in 1..tag.chars().count() {
                    chars.next();
                }
                previous = '$';
                continue;
            }
        } else if character == '\'' && !in_double && previous != '\\' {
            in_single = !in_single;
        } else if character == '"' && !in_single && previous != '\\' {
            in_double = !in_double;
        }

        if character == ';' && !in_single && !in_double && dollar_quote.is_none() {
            let statement = strip_sql_comment_lines(&current);
            if !statement.trim().is_empty() {
                statements.push((statement.trim().to_string(), statement_line));
            }
            current.clear();
        } else {
            current.push(character);
        }
        previous = character;
    }

    let statement = strip_sql_comment_lines(&current);
    if !statement.trim().is_empty() {
        statements.push((statement.trim().to_string(), statement_line));
    }

    statements
}

fn extract_sql_actions(contents: &str, extension: &str) -> Vec<SqlAction> {
    if extension != "sql" {
        return Vec::new();
    }
    let mut actions = extract_sql_actions_from_pg_query(contents).unwrap_or_default();
    for action in extract_sql_actions_manual(contents) {
        push_sql_action_unique(&mut actions, action);
    }
    actions
}

fn extract_sql_actions_manual(contents: &str) -> Vec<SqlAction> {
    let mut actions = Vec::new();
    for (trimmed, line_number) in split_sql_statements(contents) {
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("create table") {
            push_sql_action(
                &mut actions,
                "table",
                sql_name_after(&trimmed, "create table"),
                None,
                None,
                line_number,
            );
        } else if lower.starts_with("create type") {
            push_sql_action(
                &mut actions,
                "type",
                sql_name_after(&trimmed, "create type"),
                None,
                sql_enum_values(&trimmed),
                line_number,
            );
        } else if lower.starts_with("create or replace function") {
            push_sql_action(
                &mut actions,
                "function",
                sql_name_after(&trimmed, "create or replace function"),
                None,
                sql_function_signature(&trimmed),
                line_number,
            );
        } else if lower.starts_with("create function") {
            push_sql_action(
                &mut actions,
                "function",
                sql_name_after(&trimmed, "create function"),
                None,
                sql_function_signature(&trimmed),
                line_number,
            );
        } else if lower.starts_with("create policy") {
            push_sql_action(
                &mut actions,
                "policy",
                sql_policy_name(&trimmed),
                sql_on_target(&trimmed),
                None,
                line_number,
            );
        } else if lower.starts_with("create index") || lower.starts_with("create unique index") {
            push_sql_action(
                &mut actions,
                "index",
                sql_index_name(&trimmed),
                sql_on_target(&trimmed),
                None,
                line_number,
            );
        } else if lower.starts_with("create trigger") {
            push_sql_action(
                &mut actions,
                "trigger",
                sql_name_after(&trimmed, "create trigger"),
                sql_on_target(&trimmed),
                None,
                line_number,
            );
        } else if lower.starts_with("create view") || lower.starts_with("create or replace view") {
            let prefix = if lower.starts_with("create or replace view") {
                "create or replace view"
            } else {
                "create view"
            };
            push_sql_action(
                &mut actions,
                "view",
                sql_name_after(&trimmed, prefix),
                None,
                None,
                line_number,
            );
        } else if lower.starts_with("alter table") {
            let table_name = sql_name_after(&trimmed, "alter table");
            push_sql_action(
                &mut actions,
                "alter",
                table_name.clone(),
                None,
                Some(trimmed.clone()),
                line_number,
            );
            if let Some(column) = sql_alter_add_column(&trimmed) {
                push_sql_action(
                    &mut actions,
                    "add_column",
                    column.name,
                    Some(table_name.clone()),
                    Some(trimmed.clone()),
                    line_number,
                );
            }
            if let Some(column_name) = sql_alter_drop_column(&trimmed) {
                push_sql_action(
                    &mut actions,
                    "drop_column",
                    column_name,
                    Some(table_name.clone()),
                    Some(trimmed.clone()),
                    line_number,
                );
            }
            if let Some((from, to)) = sql_alter_rename_column(&trimmed) {
                push_sql_action(
                    &mut actions,
                    "rename_column",
                    from,
                    Some(table_name.clone()),
                    Some(to),
                    line_number,
                );
            }
            if let Some(to) = sql_alter_rename_table(&trimmed) {
                push_sql_action(
                    &mut actions,
                    "rename_table",
                    table_name.clone(),
                    Some(to),
                    Some(trimmed.clone()),
                    line_number,
                );
            }
            if lower.contains("enable row level security") {
                push_sql_action(
                    &mut actions,
                    "rls",
                    table_name,
                    None,
                    Some("enabled".to_string()),
                    line_number,
                );
            }
        } else if lower.starts_with("grant ") || lower.starts_with("revoke ") {
            push_sql_action(
                &mut actions,
                "grant",
                sql_grant_target(&trimmed),
                None,
                Some(trimmed.clone()),
                line_number,
            );
        } else if lower.starts_with("drop ") {
            push_sql_action(
                &mut actions,
                "drop",
                sql_drop_name(&trimmed),
                sql_on_target(&trimmed),
                Some(trimmed.clone()),
                line_number,
            );
        }
    }
    actions
}

fn extract_sql_actions_from_pg_query(contents: &str) -> Option<Vec<SqlAction>> {
    let statements = pg_parsed_statements(contents)?;
    let mut actions = Vec::new();
    for statement in statements {
        let node = statement.node.node.as_ref();
        match node {
            Some(pg_query::protobuf::node::Node::CreateStmt(create)) => {
                push_sql_action(
                    &mut actions,
                    "table",
                    pg_relation_name(create.relation.as_ref()),
                    None,
                    None,
                    statement.line,
                );
            }
            Some(pg_query::protobuf::node::Node::DefineStmt(define)) => {
                if matches!(
                    pg_query::protobuf::ObjectType::try_from(define.kind).ok(),
                    Some(pg_query::protobuf::ObjectType::ObjectType)
                ) {
                    push_sql_action(
                        &mut actions,
                        "type",
                        pg_node_name(&define.defnames),
                        None,
                        None,
                        statement.line,
                    );
                }
            }
            Some(pg_query::protobuf::node::Node::CreateFunctionStmt(function)) => {
                push_sql_action(
                    &mut actions,
                    "function",
                    pg_node_name(&function.funcname),
                    None,
                    Some(statement.text.clone()),
                    statement.line,
                );
            }
            Some(pg_query::protobuf::node::Node::CreatePolicyStmt(policy)) => {
                push_sql_action(
                    &mut actions,
                    "policy",
                    policy.policy_name.clone(),
                    Some(pg_relation_name(policy.table.as_ref())),
                    None,
                    statement.line,
                );
            }
            Some(pg_query::protobuf::node::Node::IndexStmt(index)) => {
                push_sql_action(
                    &mut actions,
                    "index",
                    index.idxname.clone(),
                    Some(pg_relation_name(index.relation.as_ref())),
                    None,
                    statement.line,
                );
            }
            Some(pg_query::protobuf::node::Node::CreateTrigStmt(trigger)) => {
                push_sql_action(
                    &mut actions,
                    "trigger",
                    trigger.trigname.clone(),
                    Some(pg_relation_name(trigger.relation.as_ref())),
                    None,
                    statement.line,
                );
            }
            Some(pg_query::protobuf::node::Node::ViewStmt(view)) => {
                push_sql_action(
                    &mut actions,
                    "view",
                    pg_relation_name(view.view.as_ref()),
                    None,
                    None,
                    statement.line,
                );
            }
            Some(pg_query::protobuf::node::Node::AlterTableStmt(alter)) => {
                let table_name = pg_relation_name(alter.relation.as_ref());
                push_sql_action(
                    &mut actions,
                    "alter",
                    table_name.clone(),
                    None,
                    Some(statement.text.clone()),
                    statement.line,
                );
                for command in &alter.cmds {
                    if let Some(pg_query::protobuf::node::Node::AlterTableCmd(command)) =
                        command.node.as_ref()
                    {
                        push_pg_alter_action(
                            &mut actions,
                            command,
                            &table_name,
                            &statement.text,
                            statement.line,
                        );
                    }
                }
            }
            Some(pg_query::protobuf::node::Node::RenameStmt(rename)) => {
                push_pg_rename_action(&mut actions, rename, statement.line);
            }
            Some(pg_query::protobuf::node::Node::DropStmt(drop)) => {
                for name in pg_drop_names(drop) {
                    push_sql_action(
                        &mut actions,
                        "drop",
                        name,
                        sql_on_target(&statement.text),
                        Some(statement.text.clone()),
                        statement.line,
                    );
                }
            }
            _ => {}
        }
    }
    Some(actions)
}

fn push_sql_action_unique(actions: &mut Vec<SqlAction>, action: SqlAction) {
    let exists = actions.iter().any(|existing| {
        existing.kind == action.kind
            && existing.name == action.name
            && existing.target == action.target
    });
    if !exists {
        actions.push(action);
    }
}

fn push_pg_alter_action(
    actions: &mut Vec<SqlAction>,
    command: &pg_query::protobuf::AlterTableCmd,
    table_name: &str,
    statement_text: &str,
    line: usize,
) {
    match pg_query::protobuf::AlterTableType::try_from(command.subtype).ok() {
        Some(pg_query::protobuf::AlterTableType::AtAddColumn) => {
            if let Some(def) = &command.def {
                if let Some(pg_query::protobuf::node::Node::ColumnDef(column)) = def.node.as_ref() {
                    push_sql_action(
                        actions,
                        "add_column",
                        column.colname.clone(),
                        Some(table_name.to_string()),
                        Some(statement_text.to_string()),
                        line,
                    );
                }
            }
        }
        Some(pg_query::protobuf::AlterTableType::AtDropColumn) => {
            push_sql_action(
                actions,
                "drop_column",
                command.name.clone(),
                Some(table_name.to_string()),
                Some(statement_text.to_string()),
                line,
            );
        }
        Some(pg_query::protobuf::AlterTableType::AtEnableRowSecurity) => {
            push_sql_action(
                actions,
                "rls",
                table_name.to_string(),
                None,
                Some("enabled".to_string()),
                line,
            );
        }
        _ => {}
    }
}

fn push_pg_rename_action(
    actions: &mut Vec<SqlAction>,
    rename: &pg_query::protobuf::RenameStmt,
    line: usize,
) {
    match pg_query::protobuf::ObjectType::try_from(rename.rename_type).ok() {
        Some(pg_query::protobuf::ObjectType::ObjectColumn) => {
            let table_name = pg_relation_name(rename.relation.as_ref());
            push_sql_action(
                actions,
                "rename_column",
                rename.subname.clone(),
                Some(table_name),
                Some(rename.newname.clone()),
                line,
            );
        }
        Some(pg_query::protobuf::ObjectType::ObjectTable) => {
            let old_name = pg_relation_name(rename.relation.as_ref());
            let schema = rename
                .relation
                .as_ref()
                .map(|relation| relation.schemaname.as_str())
                .unwrap_or_default();
            let new_name = if schema.is_empty() {
                rename.newname.clone()
            } else {
                format!("{schema}.{}", rename.newname)
            };
            push_sql_action(
                actions,
                "rename_table",
                old_name,
                Some(new_name),
                None,
                line,
            );
        }
        _ => {}
    }
}

fn pg_drop_names(drop: &pg_query::protobuf::DropStmt) -> Vec<String> {
    let object_type = pg_query::protobuf::ObjectType::try_from(drop.remove_type).ok();
    drop.objects
        .iter()
        .filter_map(|object| match object.node.as_ref() {
            Some(pg_query::protobuf::node::Node::List(list)) => match object_type {
                Some(pg_query::protobuf::ObjectType::ObjectPolicy)
                | Some(pg_query::protobuf::ObjectType::ObjectTrigger) => {
                    pg_node_names(&list.items).last().cloned()
                }
                _ => Some(pg_node_name(&list.items)),
            },
            Some(pg_query::protobuf::node::Node::ObjectWithArgs(object)) => {
                Some(pg_node_name(&object.objname))
            }
            _ => None,
        })
        .filter(|name| !name.is_empty())
        .collect()
}

fn push_sql_action(
    actions: &mut Vec<SqlAction>,
    kind: &str,
    name: String,
    target: Option<String>,
    detail: Option<String>,
    line: usize,
) {
    if !name.is_empty() {
        actions.push(SqlAction {
            kind: kind.to_string(),
            name,
            target,
            detail,
            line,
        });
    }
}

fn sql_name_after(line: &str, prefix: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let Some(index) = lower.find(prefix) else {
        return String::new();
    };
    let mut rest = line[index + prefix.len()..].trim_start();
    loop {
        let token = rest.split_whitespace().next().unwrap_or("");
        if matches!(
            token.to_ascii_lowercase().as_str(),
            "if" | "not" | "exists" | "or" | "replace"
        ) {
            rest = rest[token.len()..].trim_start();
        } else {
            break;
        }
    }
    sql_identifier(rest)
}

fn sql_identifier(rest: &str) -> String {
    let rest = rest.trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        return stripped
            .split_once('"')
            .map(|(name, _)| name.to_string())
            .unwrap_or_default();
    }
    normalize_sql_name(rest.split_whitespace().next().unwrap_or(""))
}

fn sql_policy_name(line: &str) -> String {
    sql_name_after(line, "create policy")
}

fn sql_index_name(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let prefix = if lower.starts_with("create unique index") {
        "create unique index"
    } else {
        "create index"
    };
    sql_name_after(line, prefix)
}

fn sql_drop_name(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    for prefix in [
        "drop table",
        "drop function",
        "drop policy",
        "drop trigger",
        "drop type",
        "drop view",
        "drop index",
    ] {
        if lower.starts_with(prefix) {
            return sql_name_after(line, prefix);
        }
    }
    String::new()
}

fn sql_drop_kind(line: &str) -> Option<&'static str> {
    let lower = line.to_ascii_lowercase();
    [
        ("drop table", "table"),
        ("drop function", "function"),
        ("drop policy", "policy"),
        ("drop trigger", "trigger"),
        ("drop type", "type"),
        ("drop view", "view"),
        ("drop index", "index"),
    ]
    .iter()
    .find_map(|(prefix, kind)| lower.starts_with(prefix).then_some(*kind))
}

fn sql_alter_add_column(line: &str) -> Option<SqlColumn> {
    let lower = line.to_ascii_lowercase();
    let index = lower.find(" add column ")?;
    let mut rest = line[index + " add column ".len()..].trim_start();
    loop {
        let token = rest.split_whitespace().next().unwrap_or("");
        if matches!(token.to_ascii_lowercase().as_str(), "if" | "not" | "exists") {
            rest = rest[token.len()..].trim_start();
        } else {
            break;
        }
    }
    let mut pieces = rest.split_whitespace();
    let name = normalize_sql_name(pieces.next().unwrap_or(""));
    if name.is_empty() {
        return None;
    }
    let rest = pieces.collect::<Vec<_>>().join(" ");
    let lower_rest = rest.to_ascii_lowercase();
    let mut flags = Vec::new();
    if lower_rest.contains("primary key") {
        flags.push("pk".to_string());
    }
    if lower_rest.contains("not null") {
        flags.push("required".to_string());
    }
    if lower_rest.contains("unique") {
        flags.push("unique".to_string());
    }
    if lower_rest.contains("default ") {
        flags.push("default".to_string());
    }
    Some(SqlColumn {
        name,
        data_type: sql_column_type(&rest),
        flags,
        references: sql_reference_target(&rest),
    })
}

fn sql_alter_drop_column(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let index = lower.find(" drop column ")?;
    let mut rest = line[index + " drop column ".len()..].trim_start();
    loop {
        let token = rest.split_whitespace().next().unwrap_or("");
        if matches!(token.to_ascii_lowercase().as_str(), "if" | "exists") {
            rest = rest[token.len()..].trim_start();
        } else {
            break;
        }
    }
    Some(sql_identifier(rest)).filter(|name| !name.is_empty())
}

fn sql_alter_rename_column(line: &str) -> Option<(String, String)> {
    let lower = line.to_ascii_lowercase();
    let index = lower.find(" rename column ")?;
    let rest = line[index + " rename column ".len()..].trim_start();
    let from = sql_identifier(rest);
    if from.is_empty() {
        return None;
    }
    let to_index = rest.to_ascii_lowercase().find(" to ")?;
    let to = sql_identifier(&rest[to_index + " to ".len()..]);
    if to.is_empty() {
        None
    } else {
        Some((from, to))
    }
}

fn sql_alter_rename_table(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let index = lower.find(" rename to ")?;
    let name = sql_identifier(&line[index + " rename to ".len()..]);
    Some(name).filter(|name| !name.is_empty())
}

fn sql_on_target(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let index = lower.find(" on ")?;
    let rest = &line[index + 4..];
    Some(normalize_sql_name(
        rest.split_whitespace()
            .filter(|part| !matches!(part.to_ascii_lowercase().as_str(), "table"))
            .next()
            .unwrap_or(""),
    ))
    .filter(|target| !target.is_empty())
}

fn sql_grant_target(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if let Some(index) = lower.find(" on table ") {
        return normalize_sql_name(
            line[index + " on table ".len()..]
                .split_whitespace()
                .next()
                .unwrap_or(""),
        );
    }
    if let Some(index) = lower.find(" on function ") {
        return normalize_sql_name(
            line[index + " on function ".len()..]
                .split_whitespace()
                .next()
                .unwrap_or(""),
        );
    }
    if let Some(index) = lower.find(" on schema ") {
        return normalize_sql_name(
            line[index + " on schema ".len()..]
                .split_whitespace()
                .next()
                .unwrap_or(""),
        );
    }
    "database".to_string()
}

fn sql_function_signature(line: &str) -> Option<String> {
    line.find('(').and_then(|open| {
        find_matching_paren(line, open).map(|close| line[open..=close].to_string())
    })
}

fn sql_enum_values(line: &str) -> Option<String> {
    let open = line.find('(')?;
    let close = find_matching_paren(line, open)?;
    Some(line[open + 1..close].trim().to_string()).filter(|value| !value.is_empty())
}

fn sql_column_type(rest: &str) -> String {
    let lower = rest.to_ascii_lowercase();
    let mut end = rest.len();
    for keyword in [
        " not ",
        " null",
        " default ",
        " primary ",
        " references ",
        " unique",
        " check",
        " constraint ",
        " generated ",
        " collate ",
    ] {
        if let Some(index) = lower.find(keyword) {
            end = end.min(index);
        }
    }
    rest[..end].trim().trim_end_matches(',').to_string()
}

fn sql_reference_target(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let index = lower.find(" references ")?;
    let rest = &line[index + " references ".len()..];
    let raw = rest.split_whitespace().next().unwrap_or("");
    let target = normalize_sql_name(raw.split_once('(').map(|(name, _)| name).unwrap_or(raw));
    if target.is_empty() {
        None
    } else {
        Some(target)
    }
}

fn normalize_sql_name(name: &str) -> String {
    let name = name.split_once('(').map(|(name, _)| name).unwrap_or(name);
    name.trim()
        .trim_matches(',')
        .trim_matches(';')
        .trim_matches('"')
        .trim_matches('(')
        .trim_matches(')')
        .trim_matches('"')
        .to_string()
}

fn find_matching_paren(source: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut previous = '\0';
    for (index, character) in source
        .char_indices()
        .skip_while(|(index, _)| *index < open_index)
    {
        if character == '\'' && !in_double && previous != '\\' {
            in_single = !in_single;
        } else if character == '"' && !in_single && previous != '\\' {
            in_double = !in_double;
        } else if !in_single && !in_double {
            if character == '(' {
                depth += 1;
            } else if character == ')' {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
        }
        previous = character;
    }
    None
}

fn split_sql_top_level(source: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut previous = '\0';
    let mut current = String::new();
    for character in source.chars() {
        if character == '\'' && !in_double && previous != '\\' {
            in_single = !in_single;
        } else if character == '"' && !in_single && previous != '\\' {
            in_double = !in_double;
        }

        if !in_single && !in_double {
            if character == '(' {
                depth += 1;
            } else if character == ')' {
                depth = depth.saturating_sub(1);
            } else if character == delimiter && depth == 0 {
                parts.push(current.trim().to_string());
                current.clear();
                previous = character;
                continue;
            }
        }

        current.push(character);
        previous = character;
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

fn extract_env_vars(contents: &str) -> Vec<String> {
    let mut vars = BTreeSet::new();
    for needle in ["process.env.", "Bun.env.", "import.meta.env."] {
        for segment in contents.split(needle).skip(1) {
            let name = read_symbol(segment);
            if is_env_var_name(&name) {
                vars.insert(name);
            }
        }
    }

    for segment in contents.split("env(").skip(1) {
        if let Some(name) = extract_quoted_specifier(segment) {
            if is_env_var_name(&name) {
                vars.insert(name);
            }
        }
    }

    vars.into_iter().collect()
}

fn build_local_edges(files: &[FileIntel], aliases: &[PathAlias]) -> Vec<ImportEdge> {
    let file_set: HashSet<String> = files
        .iter()
        .map(|file| file.normalized_path.clone())
        .collect();
    let mut edges = Vec::new();

    for file in files {
        for import in &file.imports {
            if !(import.starts_with("./")
                || import.starts_with("../")
                || aliases
                    .iter()
                    .any(|alias| import.starts_with(&alias.prefix)))
            {
                continue;
            }
            if let Some(target) =
                resolve_local_import(&file.normalized_path, import, &file_set, aliases)
            {
                edges.push(ImportEdge {
                    from: file.normalized_path.clone(),
                    to: target,
                });
            }
        }
    }

    edges.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then_with(|| left.to.cmp(&right.to))
    });
    edges.dedup_by(|left, right| left.from == right.from && left.to == right.to);
    edges
}

fn resolve_local_import(
    from: &str,
    import: &str,
    file_set: &HashSet<String>,
    aliases: &[PathAlias],
) -> Option<String> {
    let base = if import.starts_with("./") || import.starts_with("../") {
        let parent = from
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        normalize_virtual_path(&format!("{parent}/{import}"))
    } else if let Some(alias) = aliases
        .iter()
        .find(|alias| import.starts_with(&alias.prefix))
    {
        let rest = import.trim_start_matches(&alias.prefix);
        normalize_virtual_path(&format!("{}{}", alias.target_prefix, rest))
    } else {
        return None;
    };
    let candidates = [
        base.clone(),
        format!("{base}.ts"),
        format!("{base}.tsx"),
        format!("{base}.js"),
        format!("{base}.jsx"),
        format!("{base}.mjs"),
        format!("{base}.cjs"),
        format!("{base}.vue"),
        format!("{base}.svelte"),
        format!("{base}.astro"),
        format!("{base}/index.ts"),
        format!("{base}/index.tsx"),
        format!("{base}/index.js"),
        format!("{base}/index.jsx"),
    ];
    candidates
        .into_iter()
        .find(|candidate| file_set.contains(candidate))
}

fn extract_typescript_export(line: &str, exports: &mut Vec<String>) {
    let Some(rest) = line.strip_prefix("export ") else {
        return;
    };
    let rest = rest.strip_prefix("default ").unwrap_or(rest);
    let rest = rest.strip_prefix("async ").unwrap_or(rest);

    if let Some(named) = rest.strip_prefix('{') {
        if let Some((members, _)) = named.split_once('}') {
            for member in members.split(',').take(16 - exports.len()) {
                let name = member
                    .trim()
                    .split_once(" as ")
                    .map(|(_, alias)| alias)
                    .unwrap_or_else(|| member.trim());
                push_symbol(exports, name);
            }
        }
        return;
    }

    for prefix in [
        "function ",
        "const ",
        "let ",
        "var ",
        "class ",
        "interface ",
        "type ",
        "enum ",
    ] {
        if let Some(symbol) = rest.strip_prefix(prefix) {
            push_symbol(exports, symbol);
            return;
        }
    }
}

fn extract_rust_export(line: &str, exports: &mut Vec<String>) {
    let Some(rest) = line.strip_prefix("pub ") else {
        return;
    };
    for prefix in ["fn ", "struct ", "enum ", "trait ", "mod ", "const "] {
        if let Some(symbol) = rest.strip_prefix(prefix) {
            push_symbol(exports, symbol);
            return;
        }
    }
}

fn extract_props_by_type(contents: &str) -> HashMap<String, Vec<String>> {
    let mut props = HashMap::new();
    let lines: Vec<&str> = contents.lines().collect();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index].trim();
        let type_name = line
            .strip_prefix("type ")
            .or_else(|| line.strip_prefix("interface "))
            .map(read_symbol);
        let Some(type_name) = type_name else {
            index += 1;
            continue;
        };
        if !type_name.ends_with("Props") {
            index += 1;
            continue;
        }
        let mut fields = Vec::new();
        index += 1;
        while index < lines.len() {
            let field_line = lines[index].trim();
            if field_line.starts_with('}') || field_line.starts_with("};") {
                break;
            }
            if let Some((name, _)) = field_line.split_once(':') {
                let name = name.trim().trim_end_matches('?');
                if !name.is_empty()
                    && name
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '_')
                {
                    fields.push(name.to_string());
                }
            }
            index += 1;
        }
        props.insert(type_name, fields);
    }
    props
}

fn component_props(
    component_name: &str,
    declaration_tail: &str,
    props_by_type: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let preferred = format!("{component_name}Props");
    if let Some(props) = props_by_type.get(&preferred) {
        return props.clone();
    }
    for (name, props) in props_by_type {
        if declaration_tail.contains(name) {
            return props.clone();
        }
    }
    Vec::new()
}

fn push_symbol(exports: &mut Vec<String>, raw: &str) {
    let symbol = read_symbol(raw);
    if !symbol.is_empty() && !exports.contains(&symbol) {
        exports.push(symbol);
    }
}

fn read_symbol(raw: &str) -> String {
    raw.trim_start()
        .chars()
        .take_while(|character| {
            character.is_ascii_alphanumeric() || *character == '_' || *character == '$'
        })
        .collect()
}

fn extract_quoted_specifier(line: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let Some((_, rest)) = line.split_once(quote) else {
            continue;
        };
        let Some((specifier, _)) = rest.split_once(quote) else {
            continue;
        };
        return Some(specifier.to_string());
    }
    None
}

fn extract_route_literal(contents: &str) -> Option<String> {
    for needle in ["createFileRoute(", "createRoute("] {
        if let Some(segment) = contents.split(needle).nth(1) {
            if let Some(route) = extract_quoted_specifier(segment) {
                return Some(route);
            }
        }
    }
    None
}

fn is_component_name(name: &str) -> bool {
    name.chars()
        .next()
        .map(|character| character.is_ascii_uppercase())
        .unwrap_or(false)
        && name.chars().any(|character| character.is_ascii_lowercase())
}

fn is_env_var_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
}

fn is_test_file(path: &str) -> bool {
    path.contains("__tests__/")
        || path.contains(".test.")
        || path.contains(".spec.")
        || path.ends_with("_test.rs")
        || path.starts_with("test/")
        || path.starts_with("tests/")
}

fn is_generated_file(path: &str) -> bool {
    path.ends_with(".d.ts")
        || path.contains(".gen.")
        || path.contains("/generated")
        || path.ends_with("routeTree.gen.ts")
        || path.ends_with("worker-configuration.d.ts")
}

fn is_config_file(file: &FileIntel) -> bool {
    let path = file.normalized_path.as_str();
    path == "package.json"
        || path == "Cargo.toml"
        || path == "tsconfig.json"
        || path == "vite.config.ts"
        || path == "vitest.config.ts"
        || path == "next.config.ts"
        || path == "astro.config.mjs"
        || path == "svelte.config.js"
        || path == "nuxt.config.ts"
        || path == "wrangler.toml"
        || path == "components.json"
        || path.starts_with(".github/workflows/")
        || path.ends_with("config.toml")
        || path.ends_with("config.yml")
        || path.ends_with("config.yaml")
}

fn is_api_module(path: &str) -> bool {
    path.starts_with("src/api/")
        || path.starts_with("app/api/")
        || path.starts_with("pages/api/")
        || path.starts_with("server/api/")
        || path.starts_with("supabase/functions/")
        || path.contains("/server/")
        || path.ends_with("functions.ts")
        || path.ends_with("functions.tsx")
}

fn is_data_file(path: &str) -> bool {
    path.starts_with("supabase/")
        || path.starts_with("prisma/")
        || path.contains("schema")
        || path.contains("database")
        || path.contains("drizzle")
}

fn matches_route_extension(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("ts" | "tsx" | "js" | "jsx" | "vue" | "svelte" | "astro")
    )
}

fn route_from_file_path(path: &str) -> String {
    let path = path
        .trim_start_matches("src/routes/")
        .trim_start_matches("app/routes/")
        .trim_start_matches("routes/")
        .trim_start_matches("pages/")
        .trim_start_matches("src/pages/")
        .trim_start_matches("server/api/")
        .trim_start_matches("pages/api/")
        .trim_start_matches("app/api/");
    let path = strip_known_extension(path)
        .trim_end_matches("/index")
        .trim_end_matches(".index")
        .replace("_", "")
        .replace("$", ":");
    if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", path.replace('.', "/"))
    }
}

fn next_app_route(path: &str, suffix: &str) -> String {
    let route = path
        .trim_start_matches("app/")
        .trim_end_matches(suffix)
        .split('/')
        .filter(|segment| !segment.starts_with('('))
        .collect::<Vec<_>>()
        .join("/");
    if route.is_empty() {
        "/".to_string()
    } else {
        format!("/{route}")
    }
}

fn pages_route(path: &str, prefix: &str) -> String {
    let route = strip_known_extension(path.trim_start_matches(prefix))
        .trim_end_matches("/index")
        .replace("[...", ":")
        .replace("[[...", ":")
        .replace('[', ":")
        .replace([']', ')'], "");
    if route.is_empty() {
        "/".to_string()
    } else {
        format!("/{route}")
    }
}

fn sveltekit_route(path: &str) -> String {
    let route = path
        .trim_start_matches("src/routes/")
        .replace("/+page.svelte", "")
        .replace("/+server.ts", "");
    if route.is_empty() {
        "/".to_string()
    } else {
        format!("/{route}")
    }
}

fn strip_known_extension(path: &str) -> &str {
    for extension in [
        ".tsx", ".ts", ".jsx", ".js", ".mjs", ".cjs", ".vue", ".svelte", ".astro",
    ] {
        if let Some(stripped) = path.strip_suffix(extension) {
            return stripped;
        }
    }
    path
}

fn area_key(path: &Path) -> String {
    let parts: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    if parts.len() <= 1 {
        return ".".to_string();
    }
    match parts[0].as_str() {
        "src" | "docs" | "supabase" | ".github" | "scripts" | "crates" | "test" | "tests"
        | "app" | "pages" | "server" => format!("{}/{}", parts[0], parts[1]),
        _ => parts[0].clone(),
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn normalize_virtual_path(path: &str) -> String {
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

fn extract_object_keys(contents: &str, key: &str) -> Vec<String> {
    let Some(key_index) = contents.find(&format!("\"{key}\"")) else {
        return Vec::new();
    };
    let Some(open_offset) = contents[key_index..].find('{') else {
        return Vec::new();
    };
    let chars: Vec<char> = contents[key_index + open_offset..].chars().collect();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut current_string = String::new();
    let mut keys = Vec::new();

    for (index, character) in chars.iter().enumerate() {
        if in_string {
            if escaped {
                current_string.push(*character);
                escaped = false;
                continue;
            }
            if *character == '\\' {
                escaped = true;
                continue;
            }
            if *character == '"' {
                in_string = false;
                if depth == 1 && next_non_whitespace(&chars, index + 1) == Some(':') {
                    keys.push(current_string.clone());
                }
                current_string.clear();
                continue;
            }
            current_string.push(*character);
            continue;
        }

        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            '"' => in_string = true,
            _ => {}
        }
    }

    keys
}

fn next_non_whitespace(chars: &[char], start: usize) -> Option<char> {
    chars
        .iter()
        .skip(start)
        .copied()
        .find(|character| !character.is_whitespace())
}

fn env_var_index(analysis: &RepoAnalysis) -> BTreeMap<String, Vec<String>> {
    let mut env = BTreeMap::new();
    for file in &analysis.files {
        for var in &file.env_vars {
            env.entry(var.clone())
                .or_insert_with(Vec::new)
                .push(file.normalized_path.clone());
        }
    }
    env
}

fn database_actions(analysis: &RepoAnalysis) -> Vec<SqlAction> {
    let mut actions = Vec::new();
    for file in &analysis.files {
        for action in &file.sql_actions {
            let mut action = action.clone();
            action.target = action.target.or_else(|| Some(file.normalized_path.clone()));
            actions.push(action);
        }
    }
    actions
}

fn database_tables(analysis: &RepoAnalysis) -> BTreeMap<String, DatabaseTable> {
    let mut tables = BTreeMap::<String, DatabaseTable>::new();
    for file in &analysis.files {
        for table in &file.sql_tables {
            let entry = tables
                .entry(table.name.clone())
                .or_insert_with(|| DatabaseTable {
                    name: table.name.clone(),
                    ..DatabaseTable::default()
                });
            merge_table_columns(&mut entry.columns, &table.columns);
            push_unique(&mut entry.touched_by, file.normalized_path.clone());
        }
        for action in &file.sql_actions {
            if action.kind == "grant" {
                if let Some(entry) = tables.get_mut(&action.name) {
                    if let Some(detail) = &action.detail {
                        push_unique(&mut entry.grants, detail.clone());
                    }
                    push_unique(&mut entry.touched_by, file.normalized_path.clone());
                }
                continue;
            }
            if action.kind == "drop" {
                apply_database_drop(&mut tables, action, &file.normalized_path);
                continue;
            }
            if action.kind == "add_column" {
                if let (Some(table_name), Some(column)) = (
                    action.target.as_ref(),
                    action.detail.as_deref().and_then(sql_alter_add_column),
                ) {
                    let entry = tables
                        .entry(table_name.clone())
                        .or_insert_with(|| DatabaseTable {
                            name: table_name.clone(),
                            ..DatabaseTable::default()
                        });
                    merge_table_columns(&mut entry.columns, &[column]);
                    push_unique(&mut entry.touched_by, file.normalized_path.clone());
                }
                continue;
            }
            if action.kind == "drop_column" {
                if let Some(table_name) = &action.target {
                    if let Some(table) = tables.get_mut(table_name) {
                        table.columns.retain(|column| column.name != action.name);
                        push_unique(&mut table.touched_by, file.normalized_path.clone());
                    }
                }
                continue;
            }
            if action.kind == "rename_column" {
                if let (Some(table_name), Some(to)) = (&action.target, &action.detail) {
                    if let Some(table) = tables.get_mut(table_name) {
                        if let Some(column) = table
                            .columns
                            .iter_mut()
                            .find(|column| column.name == action.name)
                        {
                            column.name = to.clone();
                        }
                        push_unique(&mut table.touched_by, file.normalized_path.clone());
                    }
                }
                continue;
            }
            if action.kind == "rename_table" {
                if let Some(to) = &action.target {
                    if let Some(mut table) = tables.remove(&action.name) {
                        table.name = to.clone();
                        push_unique(&mut table.touched_by, file.normalized_path.clone());
                        tables.insert(to.clone(), table);
                    }
                }
                continue;
            }
            let table_name = match action.kind.as_str() {
                "policy" | "index" | "trigger" => action.target.as_ref(),
                "rls" | "alter" => Some(&action.name),
                _ => None,
            };
            let Some(table_name) = table_name else {
                continue;
            };
            let entry = tables
                .entry(table_name.clone())
                .or_insert_with(|| DatabaseTable {
                    name: table_name.clone(),
                    ..DatabaseTable::default()
                });
            push_unique(&mut entry.touched_by, file.normalized_path.clone());
            match action.kind.as_str() {
                "policy" => push_unique(&mut entry.policies, action.name.clone()),
                "index" => push_unique(&mut entry.indexes, action.name.clone()),
                "trigger" => push_unique(&mut entry.triggers, action.name.clone()),
                "rls" => entry.rls_enabled = true,
                _ => {}
            }
        }
    }
    tables
}

fn apply_database_drop(
    tables: &mut BTreeMap<String, DatabaseTable>,
    action: &SqlAction,
    path: &str,
) {
    let Some(detail) = &action.detail else {
        return;
    };
    match sql_drop_kind(detail).unwrap_or("unknown") {
        "table" => {
            tables.remove(&action.name);
            for table in tables.values_mut() {
                for column in &mut table.columns {
                    if column.references.as_deref() == Some(&action.name) {
                        column.references = None;
                    }
                }
            }
        }
        "policy" => {
            if let Some(target) = &action.target {
                if let Some(table) = tables.get_mut(target) {
                    table.policies.retain(|policy| policy != &action.name);
                    push_unique(&mut table.touched_by, path.to_string());
                }
            }
        }
        "index" => {
            for table in tables.values_mut() {
                let before = table.indexes.len();
                table.indexes.retain(|index| index != &action.name);
                if table.indexes.len() != before {
                    push_unique(&mut table.touched_by, path.to_string());
                }
            }
        }
        "trigger" => {
            if let Some(target) = &action.target {
                if let Some(table) = tables.get_mut(target) {
                    table.triggers.retain(|trigger| trigger != &action.name);
                    push_unique(&mut table.touched_by, path.to_string());
                }
            }
        }
        _ => {}
    }
}

fn merge_table_columns(existing: &mut Vec<SqlColumn>, incoming: &[SqlColumn]) {
    for column in incoming {
        if let Some(current) = existing.iter_mut().find(|item| item.name == column.name) {
            if current.data_type.is_empty() && !column.data_type.is_empty() {
                current.data_type = column.data_type.clone();
            }
            if current.references.is_none() && column.references.is_some() {
                current.references = column.references.clone();
            }
            for flag in &column.flags {
                push_unique(&mut current.flags, flag.clone());
            }
        } else {
            existing.push(column.clone());
        }
    }
}

fn database_relationships(tables: &BTreeMap<String, DatabaseTable>) -> Vec<(String, String)> {
    let mut relationships = BTreeSet::new();
    for table in tables.values() {
        for column in &table.columns {
            if let Some(target) = &column.references {
                relationships.insert((format!("{}.{}", table.name, column.name), target.clone()));
            }
        }
    }
    relationships.into_iter().collect()
}

fn display_column_flags(flags: &[String]) -> String {
    if flags.is_empty() {
        String::new()
    } else {
        format!(" ({})", flags.join(", "))
    }
}

fn push_named_list(output: &mut String, label: &str, items: &[String], limit: usize) {
    if items.is_empty() {
        return;
    }
    output.push_str(&format!("- {label}:\n"));
    for item in items.iter().take(limit) {
        output.push_str(&format!("  - `{item}`\n"));
    }
}

fn push_unique(items: &mut Vec<String>, item: String) {
    if !item.is_empty() && !items.contains(&item) {
        items.push(item);
    }
}

#[derive(Clone, Copy)]
enum BoundaryKind {
    Server,
    Client,
    Data,
    Generated,
    Env,
}

fn boundary_matches(file: &FileIntel, kind: BoundaryKind) -> bool {
    match kind {
        BoundaryKind::Server => {
            is_api_module(&file.normalized_path)
                || file.normalized_path.contains(".server.")
                || file.normalized_path.contains("/server/")
                || file.normalized_path.starts_with("supabase/functions/")
        }
        BoundaryKind::Client => {
            (matches!(
                file.extension.as_str(),
                "tsx" | "jsx" | "vue" | "svelte" | "astro"
            ) || !file.components.is_empty())
                && !is_api_module(&file.normalized_path)
        }
        BoundaryKind::Data => file.extension == "sql" || is_data_file(&file.normalized_path),
        BoundaryKind::Generated => file.is_generated,
        BoundaryKind::Env => !file.env_vars.is_empty(),
    }
}

fn reverse_edges(analysis: &RepoAnalysis) -> BTreeMap<String, Vec<String>> {
    let mut reverse: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for edge in &analysis.local_edges {
        reverse
            .entry(edge.to.clone())
            .or_default()
            .push(edge.from.clone());
    }
    for importers in reverse.values_mut() {
        importers.sort();
        importers.dedup();
    }
    reverse
}

fn file_map(analysis: &RepoAnalysis) -> BTreeMap<String, &FileIntel> {
    analysis
        .files
        .iter()
        .map(|file| (file.normalized_path.clone(), file))
        .collect()
}

fn direct_imports(analysis: &RepoAnalysis, path: &str) -> Vec<String> {
    analysis
        .local_edges
        .iter()
        .filter(|edge| edge.from == path)
        .map(|edge| edge.to.clone())
        .collect()
}

fn related_tests(analysis: &RepoAnalysis, path: &str) -> Vec<String> {
    let stem = strip_known_extension(path)
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(path);
    let area = path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let mut tests: Vec<String> = analysis
        .files
        .iter()
        .filter(|file| file.is_test)
        .filter(|file| {
            file.normalized_path.contains(stem)
                || (!area.is_empty() && file.normalized_path.starts_with(area))
        })
        .map(|file| file.normalized_path.clone())
        .collect();
    tests.sort();
    tests.dedup();
    tests
}

fn display_file_tags_or_none(file: &FileIntel) -> String {
    let tags = display_file_tags(file);
    if tags.is_empty() {
        " none".to_string()
    } else {
        tags.trim_start_matches(" — tags:").to_string()
    }
}

fn external_import_usage(analysis: &RepoAnalysis) -> Vec<(String, Vec<String>)> {
    let mut usage: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in &analysis.files {
        for import in &file.imports {
            if import.starts_with("./")
                || import.starts_with("../")
                || import.starts_with("@/")
                || import.starts_with("~/")
            {
                continue;
            }
            let package = package_name_from_import(import);
            if package.is_empty() {
                continue;
            }
            usage
                .entry(package)
                .or_default()
                .insert(file.normalized_path.clone());
        }
    }

    let mut usage: Vec<(String, Vec<String>)> = usage
        .into_iter()
        .map(|(package, files)| (package, files.into_iter().collect()))
        .collect();
    usage.sort_by(|left, right| {
        right
            .1
            .len()
            .cmp(&left.1.len())
            .then_with(|| left.0.cmp(&right.0))
    });
    usage
}

fn package_name_from_import(import: &str) -> String {
    let mut parts = import.split('/');
    let Some(first) = parts.next() else {
        return String::new();
    };
    if first.starts_with('@') {
        let Some(second) = parts.next() else {
            return first.to_string();
        };
        format!("{first}/{second}")
    } else {
        first.to_string()
    }
}

fn display_file_tags(file: &FileIntel) -> String {
    let mut tags = Vec::new();
    if !file.routes.is_empty() {
        tags.push("route");
    }
    if !file.api_endpoints.is_empty() || is_api_module(&file.normalized_path) {
        tags.push("api");
    }
    if !file.components.is_empty() {
        tags.push("component");
    }
    if file.extension == "sql" || is_data_file(&file.normalized_path) {
        tags.push("data");
    }
    if file.is_test {
        tags.push("test");
    }
    if file.is_generated {
        tags.push("generated");
    }
    if !file.env_vars.is_empty() {
        tags.push("env");
    }
    if file.used_ast {
        tags.push("ast");
    }
    if file.parse_error_count > 0 {
        tags.push("parse-error");
    }
    if tags.is_empty() {
        String::new()
    } else {
        format!(" — tags: {}", tags.join(", "))
    }
}

fn top_areas(analysis: &RepoAnalysis, limit: usize) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for file in &analysis.files {
        *counts.entry(area_key(&file.path)).or_default() += 1;
    }
    let mut areas: Vec<(String, usize)> = counts.into_iter().collect();
    areas.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    areas.truncate(limit);
    areas
}

fn top_reverse_imports(analysis: &RepoAnalysis, limit: usize) -> Vec<(String, usize)> {
    let mut imports: Vec<(String, usize)> = analysis
        .reverse_import_counts
        .iter()
        .map(|(path, count)| (path.clone(), *count))
        .collect();
    imports.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    imports.truncate(limit);
    imports
}

fn largest_files(analysis: &RepoAnalysis, limit: usize) -> Vec<&FileIntel> {
    let mut files: Vec<&FileIntel> = analysis
        .files
        .iter()
        .filter(|file| !file.is_generated && file.line_count > 0)
        .collect();
    files.sort_by(|left, right| {
        right
            .line_count
            .cmp(&left.line_count)
            .then_with(|| left.normalized_path.cmp(&right.normalized_path))
    });
    files.truncate(limit);
    files
}

fn count_routes(analysis: &RepoAnalysis) -> usize {
    analysis.files.iter().map(|file| file.routes.len()).sum()
}

fn count_api(analysis: &RepoAnalysis) -> usize {
    analysis
        .files
        .iter()
        .filter(|file| !file.api_endpoints.is_empty() || is_api_module(&file.normalized_path))
        .count()
}

fn count_components(analysis: &RepoAnalysis) -> usize {
    analysis
        .files
        .iter()
        .map(|file| file.components.len())
        .sum()
}

fn count_sql_objects(analysis: &RepoAnalysis) -> usize {
    analysis
        .files
        .iter()
        .map(|file| file.sql_objects.len())
        .sum()
}

fn count_tests(analysis: &RepoAnalysis) -> usize {
    analysis.files.iter().filter(|file| file.is_test).count()
}

fn display_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none detected".to_string()
    } else {
        items.join(", ")
    }
}

fn push_read_first(output: &mut String) {
    output.push_str("## How To Use\n\n");
    output.push_str("- Start with `summary.md`, `overview.md`, and `tasks.md`, then jump to the article that matches your task.\n");
    output.push_str("- For behavior changes, read the listed source files before editing.\n");
    output.push_str("- For broad refactors, inspect `graph.md` high-impact files first.\n");
    output.push_str("- For framework-specific work, prefer the matching route/API/component/data article over global search.\n");
    output.push_str("- Do not bulk-read deep context files such as `repo.json`, `database.md`, `imports.md`, or `calls.md` unless the task specifically requires them.\n");
}

fn push_article_read_policy(output: &mut String, path: &str) {
    let meta = intel_file_meta(path);
    output.push_str("## Read Policy\n\n");
    output.push_str(&format!(
        "- Tier: `{}`\n- {}\n\n",
        meta.tier.as_str(),
        meta.read_policy
    ));
}

fn json_string_array(items: &[String]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|item| format!("\"{}\"", json_escape(item)))
        .collect();
    format!("[{}]", parts.join(", "))
}

fn render_json_pairs(items: Vec<(String, usize)>, key_name: &str, value_name: &str) -> String {
    let objects: Vec<String> = items
        .into_iter()
        .map(|(key, value)| {
            format!(
                "{{\"{key_name}\":\"{}\",\"{value_name}\":{value}}}",
                json_escape(&key)
            )
        })
        .collect();
    format!("[{}]", objects.join(", "))
}

fn render_path_aliases_json(aliases: &[PathAlias]) -> String {
    let objects: Vec<String> = aliases
        .iter()
        .map(|alias| {
            format!(
                "{{\"prefix\":\"{}\",\"targetPrefix\":\"{}\"}}",
                json_escape(&alias.prefix),
                json_escape(&alias.target_prefix)
            )
        })
        .collect();
    format!("[{}]", objects.join(", "))
}

fn render_import_edges_json(analysis: &RepoAnalysis, limit: usize) -> String {
    let objects: Vec<String> = analysis
        .local_edges
        .iter()
        .take(limit)
        .map(|edge| {
            format!(
                "{{\"from\":\"{}\",\"to\":\"{}\"}}",
                json_escape(&edge.from),
                json_escape(&edge.to)
            )
        })
        .collect();
    format!("[{}]", objects.join(", "))
}

fn render_call_sites_json(analysis: &RepoAnalysis, limit: usize) -> String {
    let objects: Vec<String> = analysis
        .files
        .iter()
        .filter(|file| !file.call_sites.is_empty())
        .take(limit)
        .map(|file| {
            format!(
                "{{\"path\":\"{}\",\"calls\":{}}}",
                json_escape(&file.normalized_path),
                json_string_array(&file.call_sites)
            )
        })
        .collect();
    format!("[{}]", objects.join(", "))
}

fn render_impact_json(analysis: &RepoAnalysis, limit: usize) -> String {
    let reverse = reverse_edges(analysis);
    let objects: Vec<String> = top_reverse_imports(analysis, limit)
        .into_iter()
        .map(|(path, imported_by)| {
            let direct_imports = direct_imports(analysis, &path);
            let importers = reverse.get(&path).cloned().unwrap_or_default();
            let tests = related_tests(analysis, &path);
            format!(
                "{{\"path\":\"{}\",\"importedBy\":{},\"imports\":{},\"importers\":{},\"relatedTests\":{}}}",
                json_escape(&path),
                imported_by,
                json_string_array(&direct_imports.into_iter().take(30).collect::<Vec<_>>()),
                json_string_array(&importers.into_iter().take(30).collect::<Vec<_>>()),
                json_string_array(&tests.into_iter().take(20).collect::<Vec<_>>())
            )
        })
        .collect();
    format!("[{}]", objects.join(", "))
}

fn render_routes_json(analysis: &RepoAnalysis, limit: usize) -> String {
    let mut items = Vec::new();
    for file in &analysis.files {
        for route in &file.routes {
            items.push(format!(
                "{{\"file\":\"{}\",\"framework\":\"{}\",\"route\":\"{}\",\"kind\":\"{}\"}}",
                json_escape(&file.normalized_path),
                json_escape(&route.framework),
                json_escape(&route.route),
                json_escape(&route.kind)
            ));
            if items.len() >= limit {
                return format!("[{}]", items.join(", "));
            }
        }
    }
    format!("[{}]", items.join(", "))
}

fn render_api_json(analysis: &RepoAnalysis, limit: usize) -> String {
    let mut items = Vec::new();
    for file in &analysis.files {
        if file.api_endpoints.is_empty() && !is_api_module(&file.normalized_path) {
            continue;
        }
        let endpoints: Vec<String> = file
            .api_endpoints
            .iter()
            .map(|endpoint| {
                format!(
                    "{{\"method\":\"{}\",\"route\":\"{}\"}}",
                    json_escape(&endpoint.method),
                    json_escape(&endpoint.route)
                )
            })
            .collect();
        items.push(format!(
            "{{\"file\":\"{}\",\"endpoints\":[{}]}}",
            json_escape(&file.normalized_path),
            endpoints.join(", ")
        ));
        if items.len() >= limit {
            break;
        }
    }
    format!("[{}]", items.join(", "))
}

fn render_components_json(analysis: &RepoAnalysis, limit: usize) -> String {
    let mut items = Vec::new();
    for file in &analysis.files {
        for component in &file.components {
            items.push(format!(
                "{{\"name\":\"{}\",\"file\":\"{}\",\"props\":{}}}",
                json_escape(&component.name),
                json_escape(&file.normalized_path),
                json_string_array(&component.props)
            ));
            if items.len() >= limit {
                return format!("[{}]", items.join(", "));
            }
        }
    }
    format!("[{}]", items.join(", "))
}

fn render_sql_json(analysis: &RepoAnalysis, limit: usize) -> String {
    let mut items = Vec::new();
    for file in &analysis.files {
        for object in &file.sql_objects {
            items.push(format!(
                "{{\"kind\":\"{}\",\"name\":\"{}\",\"file\":\"{}\"}}",
                json_escape(&object.kind),
                json_escape(&object.name),
                json_escape(&file.normalized_path)
            ));
            if items.len() >= limit {
                return format!("[{}]", items.join(", "));
            }
        }
    }
    format!("[{}]", items.join(", "))
}

fn render_database_summary_json(analysis: &RepoAnalysis) -> String {
    let tables = database_tables(analysis);
    let actions = database_actions(analysis);
    format!(
        "{{\"tables\":{},\"relationships\":{},\"functions\":{},\"policies\":{},\"indexes\":{},\"triggers\":{},\"sqlFiles\":{},\"migrationFiles\":{}}}",
        tables.len(),
        database_relationships(&tables).len(),
        actions.iter().filter(|action| action.kind == "function").count(),
        actions.iter().filter(|action| action.kind == "policy").count(),
        actions.iter().filter(|action| action.kind == "index").count(),
        actions.iter().filter(|action| action.kind == "trigger").count(),
        analysis.files.iter().filter(|file| file.extension == "sql").count(),
        analysis
            .files
            .iter()
            .filter(|file| {
                file.extension == "sql"
                    && file.normalized_path.starts_with("supabase/migrations/")
            })
            .count()
    )
}

fn render_database_tables_json(analysis: &RepoAnalysis, limit: usize) -> String {
    let tables = database_tables(analysis);
    let objects: Vec<String> = tables
        .values()
        .take(limit)
        .map(|table| {
            let columns: Vec<String> = table
                .columns
                .iter()
                .take(80)
                .map(|column| {
                    format!(
                        "{{\"name\":\"{}\",\"type\":\"{}\",\"flags\":{},\"references\":{}}}",
                        json_escape(&column.name),
                        json_escape(&column.data_type),
                        json_string_array(&column.flags),
                        column
                            .references
                            .as_ref()
                            .map(|target| format!("\"{}\"", json_escape(target)))
                            .unwrap_or_else(|| "null".to_string())
                    )
                })
                .collect();
            format!(
                "{{\"name\":\"{}\",\"rls\":{},\"columns\":[{}],\"indexes\":{},\"triggers\":{},\"policies\":{},\"touchedBy\":{}}}",
                json_escape(&table.name),
                table.rls_enabled,
                columns.join(", "),
                json_string_array(&table.indexes),
                json_string_array(&table.triggers),
                json_string_array(&table.policies),
                json_string_array(&table.touched_by)
            )
        })
        .collect();
    format!("[{}]", objects.join(", "))
}

fn render_files_json(analysis: &RepoAnalysis, limit: usize) -> String {
    let mut items = Vec::new();
    for file in analysis.files.iter().take(limit) {
        let mut tags = Vec::new();
        if !file.routes.is_empty() {
            tags.push("route".to_string());
        }
        if !file.api_endpoints.is_empty() || is_api_module(&file.normalized_path) {
            tags.push("api".to_string());
        }
        if !file.components.is_empty() {
            tags.push("component".to_string());
        }
        if file.extension == "sql" || is_data_file(&file.normalized_path) {
            tags.push("data".to_string());
        }
        if file.is_test {
            tags.push("test".to_string());
        }
        if file.is_generated {
            tags.push("generated".to_string());
        }
        if !file.env_vars.is_empty() {
            tags.push("env".to_string());
        }
        if file.used_ast {
            tags.push("ast".to_string());
        }
        if file.parse_error_count > 0 {
            tags.push("parse-error".to_string());
        }
        items.push(format!(
            "{{\"path\":\"{}\",\"lines\":{},\"tags\":{},\"astParsed\":{},\"parseErrors\":{}}}",
            json_escape(&file.normalized_path),
            file.line_count,
            json_string_array(&tags),
            file.used_ast,
            file.parse_error_count
        ));
    }
    format!("[{}]", items.join(", "))
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "agent-toolkit-intel-{}-{}-{}",
            std::process::id(),
            TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn build_repo_intel_ignores_hidden_agent_tooling_dirs() {
        let root = temp_dir();
        fs::create_dir_all(root.join(".agents/intel")).unwrap();
        fs::create_dir_all(root.join(".codex")).unwrap();
        fs::write(root.join(".agents/intel/summary.md"), "# Generated\n").unwrap();
        fs::write(
            root.join(".codex/config.toml"),
            "approval_policy = 'on-request'\n",
        )
        .unwrap();

        let intel = build_repo_intel(&root).unwrap();

        assert_eq!(intel.file_count, 0);
        assert!(intel.summary_markdown.contains("overview.md"));
        assert!(intel.summary_markdown.contains("graph.md"));
        assert!(intel.summary_markdown.contains("Deep Context"));
        assert!(intel
            .summary_markdown
            .contains("database.md`, `imports.md`, `calls.md`, and `repo.json"));
    }

    #[test]
    fn build_repo_intel_counts_framework_surface_and_hotspots() {
        let root = temp_dir();
        fs::create_dir_all(root.join("src/components")).unwrap();
        fs::create_dir_all(root.join("src/routes")).unwrap();
        fs::create_dir_all(root.join("supabase/migrations")).unwrap();
        fs::write(root.join("AGENTS.md"), "# Rules\n").unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"check":"bun test"},"dependencies":{"react":"latest","@tanstack/react-start":"latest"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("src/components/button.tsx"),
            "type ButtonProps = {\n\tlabel: string\n\tsize?: string\n}\nimport { cn } from '../cn'\nexport function Button(props: ButtonProps) {\n\treturn null\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("src/routes/index.tsx"),
            "import { Button } from '../components/button'\nexport const Route = createFileRoute('/')({ component: Button })\n",
        )
        .unwrap();
        fs::write(
            root.join("supabase/migrations/001.sql"),
            "create table public.users (id uuid primary key);\n",
        )
        .unwrap();

        let intel = build_repo_intel(&root).unwrap();

        assert_eq!(intel.file_count, 5);
        assert!(intel.summary_markdown.contains("TanStack Start"));
        assert!(intel.summary_markdown.contains("Routes: 2"));
        assert!(intel.summary_markdown.contains("Components: 1"));
        assert!(intel.summary_markdown.contains("SQL objects: 1"));
    }

    #[test]
    fn write_repo_intel_creates_article_files() {
        let root = temp_dir();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/index.ts"), "export const value = 1\n").unwrap();

        write_repo_intel(&root).unwrap();

        for file in [
            "index.md",
            "overview.md",
            "tasks.md",
            "tooling.md",
            "routes.md",
            "api.md",
            "components.md",
            "data.md",
            "database.md",
            "graph.md",
            "impact.md",
            "boundaries.md",
            "imports.md",
            "calls.md",
            "dependencies.md",
            "symbols.md",
            "files.md",
            "env.md",
            "testing.md",
            "summary.md",
            "repo.json",
            "manifest.json",
        ] {
            assert!(root.join(".agents/intel").join(file).exists(), "{file}");
        }
        let summary = fs::read_to_string(root.join(".agents/intel/summary.md")).unwrap();
        assert!(summary.contains("Treat `database.md`, `imports.md`, `calls.md`, and `repo.json`"));
        let database = fs::read_to_string(root.join(".agents/intel/database.md")).unwrap();
        assert!(database.contains("Tier: `deep-context`"));
        let manifest = fs::read_to_string(root.join(".agents/intel/manifest.json")).unwrap();
        assert!(manifest.contains("\"entrypoint\": \"summary.md\""));
        assert!(manifest.contains("\"path\": \"repo.json\""));
        assert!(manifest.contains("\"tier\": \"machine-deep-context\""));
    }

    #[test]
    fn extracts_package_object_keys() {
        let scripts = extract_object_keys(
            r#"{"scripts":{"check":"bun test","dev":"vite","nested":"echo { ok }"},"dependencies":{"ignored":"no"}}"#,
            "scripts",
        );

        assert_eq!(scripts, vec!["check", "dev", "nested"]);
    }

    #[test]
    fn package_name_from_import_handles_scopes() {
        assert_eq!(
            package_name_from_import("@tanstack/react-router/file-route"),
            "@tanstack/react-router"
        );
        assert_eq!(package_name_from_import("react/jsx-runtime"), "react");
    }

    #[test]
    fn filters_external_repo_intel_tooling_from_package_names() {
        let filtered = filter_external_intel_tooling(vec![
            format!("{}{}", "code", "sight"),
            "check".to_string(),
        ]);

        assert_eq!(filtered, vec!["check"]);
    }

    #[test]
    fn resolves_local_imports() {
        let files = vec![
            FileIntel {
                path: PathBuf::from("src/index.ts"),
                normalized_path: "src/index.ts".to_string(),
                extension: "ts".to_string(),
                line_count: 1,
                imports: vec!["./lib".to_string()],
                exports: Vec::new(),
                call_sites: Vec::new(),
                components: Vec::new(),
                routes: Vec::new(),
                api_endpoints: Vec::new(),
                sql_objects: Vec::new(),
                sql_tables: Vec::new(),
                sql_actions: Vec::new(),
                env_vars: Vec::new(),
                is_test: false,
                is_generated: false,
                used_ast: false,
                parse_error_count: 0,
            },
            FileIntel {
                path: PathBuf::from("src/lib.ts"),
                normalized_path: "src/lib.ts".to_string(),
                extension: "ts".to_string(),
                line_count: 1,
                imports: Vec::new(),
                exports: Vec::new(),
                call_sites: Vec::new(),
                components: Vec::new(),
                routes: Vec::new(),
                api_endpoints: Vec::new(),
                sql_objects: Vec::new(),
                sql_tables: Vec::new(),
                sql_actions: Vec::new(),
                env_vars: Vec::new(),
                is_test: false,
                is_generated: false,
                used_ast: false,
                parse_error_count: 0,
            },
        ];

        let edges = build_local_edges(&files, &[]);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].to, "src/lib.ts");
    }

    #[test]
    fn resolves_src_alias_imports() {
        let files = vec![
            FileIntel {
                path: PathBuf::from("src/index.ts"),
                normalized_path: "src/index.ts".to_string(),
                extension: "ts".to_string(),
                line_count: 1,
                imports: vec!["@/lib".to_string()],
                exports: Vec::new(),
                call_sites: Vec::new(),
                components: Vec::new(),
                routes: Vec::new(),
                api_endpoints: Vec::new(),
                sql_objects: Vec::new(),
                sql_tables: Vec::new(),
                sql_actions: Vec::new(),
                env_vars: Vec::new(),
                is_test: false,
                is_generated: false,
                used_ast: false,
                parse_error_count: 0,
            },
            FileIntel {
                path: PathBuf::from("src/lib.ts"),
                normalized_path: "src/lib.ts".to_string(),
                extension: "ts".to_string(),
                line_count: 1,
                imports: Vec::new(),
                exports: Vec::new(),
                call_sites: Vec::new(),
                components: Vec::new(),
                routes: Vec::new(),
                api_endpoints: Vec::new(),
                sql_objects: Vec::new(),
                sql_tables: Vec::new(),
                sql_actions: Vec::new(),
                env_vars: Vec::new(),
                is_test: false,
                is_generated: false,
                used_ast: false,
                parse_error_count: 0,
            },
        ];

        let aliases = vec![PathAlias {
            prefix: "@/".to_string(),
            target_prefix: "src/".to_string(),
        }];
        let edges = build_local_edges(&files, &aliases);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].to, "src/lib.ts");
    }

    #[test]
    fn extracts_single_quoted_imports() {
        let imports = extract_imports("import { value } from './value'\n", "ts");

        assert_eq!(imports, vec!["./value"]);
    }

    #[test]
    fn extracts_multiline_imports() {
        let imports = extract_imports("import {\n\tvalue,\n\tother,\n} from './value'\n", "ts");

        assert_eq!(imports, vec!["./value"]);
    }

    #[test]
    fn extracts_tsconfig_path_aliases() {
        let aliases = extract_tsconfig_paths(
            r#"{"compilerOptions":{"paths":{"@app/*":["./app/*"],"@lib/*":["src/lib/*"]}}}"#,
        );

        assert_eq!(aliases.len(), 2);
        assert_eq!(normalize_alias_prefix(&aliases[0].0), "@app/");
        assert_eq!(normalize_alias_target(&aliases[0].1[0]), "app/");
    }

    #[test]
    fn extracts_database_design_from_sql_migration() {
        let sql = r#"
            create type public.plan_status as enum ('draft', 'active');
            create table public.parents (
                id uuid primary key default gen_random_uuid()
            );
            create table public.children (
                id uuid primary key,
                parent_id uuid not null references public.parents(id),
                name text not null unique,
                constraint children_parent_fk foreign key (parent_id) references public.parents(id)
            );
            create index idx_children_parent on public.children(parent_id);
            alter table public.children enable row level security;
            create policy "children_select_own" on public.children for select to authenticated using (true);
            create or replace function public.search_children(term text)
            returns setof public.children
            language sql
            as $$ select * from public.children; $$;
            grant select on table public.children to authenticated;
            grant execute on function public.search_children(text) to authenticated;
            drop policy if exists "All users can read children" on public.children;
        "#;

        let tables = extract_sql_tables(sql, "sql");
        let actions = extract_sql_actions(sql, "sql");

        let child = tables
            .iter()
            .find(|table| table.name == "public.children")
            .unwrap();
        assert!(child.columns.iter().any(|column| column.name == "parent_id"
            && column.references.as_deref() == Some("public.parents")));
        assert!(actions
            .iter()
            .any(|action| action.kind == "policy" && action.name == "children_select_own"));
        assert!(actions
            .iter()
            .any(|action| action.kind == "function" && action.name == "public.search_children"));
        assert!(actions
            .iter()
            .any(|action| action.kind == "rls" && action.name == "public.children"));
        assert!(actions
            .iter()
            .any(|action| action.kind == "drop" && action.name == "All users can read children"));
        assert!(actions
            .iter()
            .any(|action| action.kind == "grant" && action.name == "public.children"));
    }

    #[test]
    fn database_design_reduces_ordered_static_migrations() {
        let root = temp_dir();
        fs::create_dir_all(root.join("supabase/migrations")).unwrap();
        fs::write(
            root.join("supabase/migrations/001_create.sql"),
            r#"
                create table public.widgets (
                    id uuid primary key,
                    old_name text,
                    owner_id uuid references public.users(id)
                );
                create table public.retired_widgets (
                    id uuid primary key
                );
                create index idx_widgets_owner on public.widgets(owner_id);
                create policy "Old widget policy" on public.widgets for select using (true);
            "#,
        )
        .unwrap();
        fs::write(
            root.join("supabase/migrations/002_change.sql"),
            r#"
                alter table public.widgets add column if not exists display_name text not null default '';
                alter table public.widgets drop column if exists old_name;
                drop policy if exists "Old widget policy" on public.widgets;
                drop index if exists idx_widgets_owner;
                create policy "Current widget policy" on public.widgets for select using (true);
            "#,
        )
        .unwrap();
        fs::write(
            root.join("supabase/migrations/003_drop.sql"),
            "drop table if exists public.retired_widgets;\n",
        )
        .unwrap();

        let analysis = analyze_repo(&root).unwrap();
        let tables = database_tables(&analysis);
        let widgets = tables.get("public.widgets").unwrap();

        assert!(!tables.contains_key("public.retired_widgets"));
        assert!(widgets.columns.iter().any(|column| {
            column.name == "display_name" && column.flags.contains(&"required".to_string())
        }));
        assert!(!widgets
            .columns
            .iter()
            .any(|column| column.name == "old_name"));
        assert!(!widgets.policies.contains(&"Old widget policy".to_string()));
        assert!(widgets
            .policies
            .contains(&"Current widget policy".to_string()));
        assert!(!widgets.indexes.contains(&"idx_widgets_owner".to_string()));
    }

    #[test]
    fn database_design_does_not_create_tables_from_function_grants() {
        let root = temp_dir();
        fs::create_dir_all(root.join("supabase/migrations")).unwrap();
        fs::write(
            root.join("supabase/migrations/001.sql"),
            r#"
                create table public.real_table (id uuid primary key);
                create function public.helper() returns void language sql as $$ select null; $$;
                grant execute on function public.helper() to authenticated;
                grant usage on schema public to authenticated;
            "#,
        )
        .unwrap();

        let analysis = analyze_repo(&root).unwrap();
        let tables = database_tables(&analysis);

        assert!(tables.contains_key("public.real_table"));
        assert!(!tables.contains_key("public.helper"));
        assert!(!tables.contains_key("public"));
        assert!(!tables.contains_key("database"));
    }

    #[test]
    fn database_design_uses_postgres_parser_for_table_variants() {
        let sql = r#"
            create unlogged table if not exists public.parser_widgets (
                id uuid primary key,
                owner_id uuid references public.users(id)
            );
        "#;

        let tables = extract_sql_tables(sql, "sql");
        let actions = extract_sql_actions(sql, "sql");

        let table = tables
            .iter()
            .find(|table| table.name == "public.parser_widgets")
            .unwrap();
        assert!(table.columns.iter().any(|column| column.name == "owner_id"
            && column.references.as_deref() == Some("public.users")));
        assert!(actions
            .iter()
            .any(|action| { action.kind == "table" && action.name == "public.parser_widgets" }));
    }

    #[test]
    fn extracts_js_ts_intel_from_oxc_ast() {
        let ast = extract_ast_file_intel(
            Path::new("src/routes/index.tsx"),
            r#"
                import React from "react";
                import { helper } from "@/helper";
                export { helper } from "@/helper";
                export function GET() { return new Response("ok") }
                export const Counter = () => <div />;
                const route = createFileRoute("/dashboard")({ component: Counter });
                app.post("/api/items", () => null);
                console.log(process.env.SUPABASE_URL, import.meta.env.VITE_PUBLIC_URL);
                import("./lazy");
            "#,
            "tsx",
        )
        .unwrap();

        assert!(ast.imports.contains(&"react".to_string()));
        assert!(ast.imports.contains(&"@/helper".to_string()));
        assert!(ast.imports.contains(&"./lazy".to_string()));
        assert!(ast.exports.contains(&"GET".to_string()));
        assert!(ast.exports.contains(&"Counter".to_string()));
        assert!(ast.component_names.contains(&"Counter".to_string()));
        assert!(ast.env_vars.contains(&"SUPABASE_URL".to_string()));
        assert!(ast.env_vars.contains(&"VITE_PUBLIC_URL".to_string()));
        assert!(ast.call_sites.contains(&"createFileRoute".to_string()));
        assert!(ast.route_declarations.contains(&"/dashboard".to_string()));
        assert!(ast
            .http_routes
            .iter()
            .any(|endpoint| endpoint.method == "POST" && endpoint.route == "/api/items"));
    }

    #[test]
    fn component_names_exclude_all_caps_constants() {
        assert!(is_component_name("Button"));
        assert!(!is_component_name("ACTIVITY_LEVEL_ITEMS"));
    }
}
