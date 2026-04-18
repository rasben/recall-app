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

    let since = day_naive.and_hms_opt(0, 0, 0).ok_or("Invalid day start")?;
    let until = day_naive.and_hms_opt(23, 59, 59).ok_or("Invalid day end")?;

    let since_local = Local
        .from_local_datetime(&since)
        .single()
        .ok_or("Ambiguous local start of day")?;
    let until_local = Local
        .from_local_datetime(&until)
        .single()
        .ok_or("Ambiguous local end of day")?;

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
            let mut parts = line.splitn(3, '\t');
            let hash = parts.next().unwrap_or("").trim();
            let ts_str = parts.next().unwrap_or("").trim();
            let subject = parts.next().unwrap_or("").trim();

            if hash.len() < 7 || ts_str.is_empty() {
                continue;
            }

            let ts: i64 = ts_str.parse().unwrap_or(0);
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
                    title: subject.to_string(),
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
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
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
