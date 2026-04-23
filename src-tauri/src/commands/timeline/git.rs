/// To-Do - this file is completely AI-coded, and not well-reviewed.
/// We need to review it, and optimize it.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{Local, NaiveDate, TimeZone};
use tauri::State;

use crate::commands::settings_git::get_settings_git;
use crate::state::AppState;
use crate::timeline::{TimelineEvent, TimelineEventSource};

const MAX_SCAN_DEPTH: u32 = 14;

pub(super) fn events_for_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Result<Vec<(i64, TimelineEvent)>, String> {
    let Some(settings) = get_settings_git(state.clone()) else {
        return Ok(Vec::new());
    };
    if !settings.enabled {
        return Ok(Vec::new());
    }
    let root = settings.path;
    if root.is_empty() {
        return Ok(Vec::new());
    }

    let day_naive = NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map_err(|_| format!("Invalid date (expected YYYY-MM-DD): {day}"))?;

    let next_day = day_naive
        .succ_opt()
        .ok_or_else(|| format!("no day after {day}"))?;
    let since = day_naive.and_hms_opt(0, 0, 0).ok_or("Invalid day start")?;
    let until = next_day.and_hms_opt(0, 0, 0).ok_or("Invalid day end")?;

    let since_local = Local
        .from_local_datetime(&since)
        .single()
        .ok_or("Ambiguous local start of day")?;
    let until_local = Local
        .from_local_datetime(&until)
        .single()
        .ok_or("Ambiguous local end of day")?;

    // git log --until is exclusive of the given time, so passing next-day 00:00
    // captures everything through the end of `day`.
    let since_str = since_local.format("%Y-%m-%d %H:%M:%S").to_string();
    let until_str = until_local.format("%Y-%m-%d %H:%M:%S").to_string();

    let root_path = PathBuf::from(&root);
    if !root_path.is_dir() {
        return Err(format!("Git scan path is not a directory: {root}"));
    }

    let mut repos = Vec::new();
    collect_git_repo_roots(&root_path, &mut repos, 0)
        .map_err(|e| format!("Failed to scan {root}: {e}"))?;

    if repos.is_empty() {
        return Ok(Vec::new());
    }

    let author = resolve_git_user_name(&repos, &root_path).unwrap_or_default();
    if author.is_empty() {
        return Ok(Vec::new());
    }

    let mut rows: Vec<(i64, TimelineEvent)> = Vec::new();

    for repo in &repos {
        let repo_name = repo
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| repo.to_string_lossy().into_owned());

        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .arg("log")
            .arg("--all")
            .arg("--since")
            .arg(&since_str)
            .arg("--until")
            .arg(&until_str)
            .arg("--author")
            .arg(&author)
            .arg("-z")
            .arg("--pretty=format:%H%x09%ct%x09%s")
            .output()
            .map_err(|e| format!("Failed to run git in {}: {e}", repo.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "git log failed in {}: {}",
                repo.display(),
                stderr.trim()
            ));
        }

        let stdout = output.stdout;
        for record in stdout.split(|&b| b == 0).filter(|chunk| !chunk.is_empty()) {
            let line = String::from_utf8_lossy(record);
            let Some((hash, ts, subject)) = parse_git_log_record(&line) else {
                continue;
            };
            let time = Local
                .timestamp_opt(ts, 0)
                .single()
                .map(|dt| dt.format("%H:%M").to_string())
                .unwrap_or_else(|| "00:00".to_string());

            let short = &hash[..7.min(hash.len())];
            let id = format!("git:{}:{}", repo.display(), hash);

            rows.push((
                ts,
                TimelineEvent {
                    id,
                    time,
                    source: TimelineEventSource::Git,
                    title: subject,
                    detail: Some(format!("{repo_name} — {short}")),
                    url: None,
                },
            ));
        }
    }

    rows.sort_by_key(|(ts, _)| *ts);
    Ok(rows)
}

fn collect_git_repo_roots(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) -> std::io::Result<()> {
    if depth > MAX_SCAN_DEPTH {
        return Ok(());
    }

    if dir.join(".git").exists() {
        out.push(dir.to_path_buf());
        return Ok(());
    }

    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };

    for entry in read.flatten() {
        // Don't follow symlinks — avoids infinite loops (e.g. ~/Downloads
        // with a link back to $HOME) and keeps the scan rooted under the
        // user-configured directory.
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        if matches!(
            name.as_str(),
            "node_modules" | "target" | ".git" | ".cargo" | "dist" | "build"
        ) {
            continue;
        }

        collect_git_repo_roots(&path, out, depth + 1)?;
    }

    Ok(())
}

fn resolve_git_user_name(repos: &[PathBuf], scan_root: &Path) -> Option<String> {
    if let Some(name) = try_git_config_user_name(scan_root) {
        if !name.is_empty() {
            return Some(name);
        }
    }
    for repo in repos {
        if let Some(name) = try_git_config_user_name(repo) {
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

fn try_git_config_user_name(repo: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "user.name"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Parse one null-delimited record from `git log -z --pretty=format:%H%x09%ct%x09%s`.
/// Returns `(full_hash, unix_timestamp, subject)` or `None` if the record is malformed.
fn parse_git_log_record(record: &str) -> Option<(String, i64, String)> {
    let mut parts = record.splitn(3, '\t');
    let hash = parts.next()?.trim().to_string();
    let ts_str = parts.next()?.trim();
    let subject = parts.next().unwrap_or("").trim().to_string();
    if hash.len() < 7 || ts_str.is_empty() {
        return None;
    }
    let ts: i64 = ts_str.parse().ok()?;
    Some((hash, ts, subject))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_tmp(label: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("recall-git-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn git(repo: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap()
            .status;
        assert!(status.success(), "git {args:?} failed in {}", repo.display());
    }

    fn init_repo(dir: &Path, author: &str) {
        git(dir, &["init"]);
        git(dir, &["config", "user.name", author]);
        git(dir, &["config", "user.email", "test@example.com"]);
    }

    // --- parse_git_log_record ---

    #[test]
    fn parse_record_valid() {
        let line = "a1b2c3d4e5f6a1b2\t1705312800\tFix the bug";
        let (hash, ts, subject) = parse_git_log_record(line).unwrap();
        assert_eq!(hash, "a1b2c3d4e5f6a1b2");
        assert_eq!(ts, 1705312800);
        assert_eq!(subject, "Fix the bug");
    }

    #[test]
    fn parse_record_empty_subject() {
        let line = "a1b2c3d4e5f6a1b2\t1705312800\t";
        let (_, _, subject) = parse_git_log_record(line).unwrap();
        assert_eq!(subject, "");
    }

    #[test]
    fn parse_record_short_hash_returns_none() {
        assert!(parse_git_log_record("abc\t1705312800\tMsg").is_none());
    }

    #[test]
    fn parse_record_missing_timestamp_returns_none() {
        assert!(parse_git_log_record("a1b2c3d4e5f6\t\tMsg").is_none());
    }

    #[test]
    fn parse_record_non_numeric_timestamp_returns_none() {
        assert!(parse_git_log_record("a1b2c3d4e5f6\tnot-a-ts\tMsg").is_none());
    }

    // --- collect_git_repo_roots ---

    #[test]
    fn finds_direct_git_repo() {
        let tmp = make_tmp("direct");
        fs::create_dir_all(tmp.join(".git")).unwrap();
        let mut repos = Vec::new();
        collect_git_repo_roots(&tmp, &mut repos, 0).unwrap();
        assert_eq!(repos, vec![tmp.clone()]);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn finds_multiple_sibling_repos() {
        let tmp = make_tmp("siblings");
        fs::create_dir_all(tmp.join("alpha/.git")).unwrap();
        fs::create_dir_all(tmp.join("beta/.git")).unwrap();
        let mut repos = Vec::new();
        collect_git_repo_roots(&tmp, &mut repos, 0).unwrap();
        repos.sort();
        assert_eq!(repos.len(), 2);
        assert!(repos[0].ends_with("alpha"));
        assert!(repos[1].ends_with("beta"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn skips_node_modules() {
        let tmp = make_tmp("node-mod");
        fs::create_dir_all(tmp.join("node_modules/hidden/.git")).unwrap();
        let mut repos = Vec::new();
        collect_git_repo_roots(&tmp, &mut repos, 0).unwrap();
        assert!(repos.is_empty(), "node_modules must be skipped");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn skips_target_and_build() {
        let tmp = make_tmp("skip-dirs");
        for skipped in &["target", "build", "dist", ".cargo"] {
            fs::create_dir_all(tmp.join(skipped).join("nested/.git")).unwrap();
        }
        let mut repos = Vec::new();
        collect_git_repo_roots(&tmp, &mut repos, 0).unwrap();
        assert!(repos.is_empty(), "build/target/dist/.cargo must be skipped");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn stops_recursing_into_sub_repos() {
        let tmp = make_tmp("stop-at-repo");
        // tmp itself is a repo — the nested repo must not be discovered
        fs::create_dir_all(tmp.join(".git")).unwrap();
        fs::create_dir_all(tmp.join("inner/.git")).unwrap();
        let mut repos = Vec::new();
        collect_git_repo_roots(&tmp, &mut repos, 0).unwrap();
        assert_eq!(repos, vec![tmp.clone()]);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn non_repo_dir_returns_empty() {
        let tmp = make_tmp("no-repo");
        fs::create_dir_all(tmp.join("src")).unwrap();
        let mut repos = Vec::new();
        collect_git_repo_roots(&tmp, &mut repos, 0).unwrap();
        assert!(repos.is_empty());
        let _ = fs::remove_dir_all(&tmp);
    }

    // --- try_git_config_user_name ---

    #[test]
    fn reads_user_name_from_local_git_config() {
        let tmp = make_tmp("user-name");
        init_repo(&tmp, "Recall Tester");
        let name = try_git_config_user_name(&tmp);
        assert_eq!(name.as_deref(), Some("Recall Tester"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn returns_none_for_nonexistent_path() {
        // git -C <missing> config user.name exits non-zero → None
        let missing = Path::new("/nonexistent-recall-test-path-xyz");
        assert!(try_git_config_user_name(missing).is_none());
    }

    // --- integration: real commit round-trip ---

    #[test]
    fn git_log_finds_commit_with_correct_metadata() {
        let tmp = make_tmp("commit-roundtrip");
        init_repo(&tmp, "Recall Tester");
        fs::write(tmp.join("hello.txt"), "hi").unwrap();
        git(&tmp, &["add", "hello.txt"]);
        git(&tmp, &["commit", "-m", "Add hello"]);

        let output = Command::new("git")
            .arg("-C").arg(&tmp)
            .args(["log", "--all",
                "--since", "2020-01-01 00:00:00",
                "--until", "2099-12-31 23:59:59",
                "--author", "Recall Tester",
                "-z", "--pretty=format:%H%x09%ct%x09%s"])
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();

        assert!(output.status.success());

        let parsed: Vec<_> = output.stdout
            .split(|&b| b == 0)
            .filter(|c| !c.is_empty())
            .filter_map(|c| parse_git_log_record(&String::from_utf8_lossy(c)))
            .collect();

        assert_eq!(parsed.len(), 1);
        let (hash, ts, subject) = &parsed[0];
        assert!(hash.len() >= 7);
        assert!(*ts > 0);
        assert_eq!(subject, "Add hello");

        let _ = fs::remove_dir_all(&tmp);
    }
}
