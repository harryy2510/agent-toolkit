use std::fs;
use std::path::Path;
use std::process::Command;

const DB_LINT_SCRIPT_CANDIDATES: [&str; 4] =
    ["db:lint", "lint:db", "supabase:lint", "supabase:db:lint"];

pub fn staged_db_lint_needed(root: &Path) -> bool {
    if !has_supabase_project_files(root) {
        return false;
    }

    let Ok(output) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .output()
    else {
        return true;
    };
    if !output.status.success() {
        return true;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(is_supabase_db_lint_path)
}

pub fn db_lint_script(root: &Path) -> Option<String> {
    let contents = fs::read_to_string(root.join("package.json")).ok()?;
    let package = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    let scripts = package.get("scripts")?.as_object()?;

    DB_LINT_SCRIPT_CANDIDATES
        .iter()
        .find(|candidate| {
            matches!(
                scripts.get(**candidate).and_then(|script| script.as_str()),
                Some(script) if !script.trim().is_empty()
            )
        })
        .map(|candidate| (*candidate).to_string())
}

fn has_supabase_project_files(root: &Path) -> bool {
    root.join("supabase/config.toml").exists() || root.join("supabase/migrations").is_dir()
}

fn is_supabase_db_lint_path(path: &str) -> bool {
    path == "supabase/config.toml"
        || path == "supabase/seed.sql"
        || (path.starts_with("supabase/") && path.ends_with(".sql"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "agent-toolkit-supabase-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn db_lint_script_prefers_project_package_scripts() {
        let root = temp_dir();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"test":"bun test","lint:db":"supabase db lint --local --fail-on warning"}}"#,
        )
        .unwrap();

        assert_eq!(db_lint_script(&root), Some("lint:db".to_string()));
    }

    #[test]
    fn staged_db_lint_needed_detects_staged_supabase_sql() {
        let root = temp_dir();
        let _ = Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        fs::create_dir_all(root.join("supabase/migrations")).unwrap();
        fs::write(
            root.join("supabase/migrations/20260507000000_create_table.sql"),
            "create table public.items (id bigint primary key);\n",
        )
        .unwrap();
        let _ = Command::new("git")
            .args(["add", "supabase/migrations/20260507000000_create_table.sql"])
            .current_dir(&root)
            .output()
            .unwrap();

        assert!(staged_db_lint_needed(&root));
    }

    #[test]
    fn staged_db_lint_needed_ignores_non_supabase_changes() {
        let root = temp_dir();
        let _ = Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        fs::create_dir_all(root.join("supabase/migrations")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/index.ts"), "export const value = 1;\n").unwrap();
        let _ = Command::new("git")
            .args(["add", "src/index.ts"])
            .current_dir(&root)
            .output()
            .unwrap();

        assert!(!staged_db_lint_needed(&root));
    }
}
