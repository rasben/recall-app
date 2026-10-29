//! "Set up with Claude": teach the Claude desktop app (Code tab) about the
//! `recall` CLI and hand it a request to schedule a daily Harvest overview.
//!
//! Claude has no API for another app to create a scheduled task, so this does
//! the parts that are plain files (a skill describing the CLI, a permission
//! rule so unattended runs may call it) and opens Claude with the scheduling
//! request prefilled via a `claude://` link. The user sends it, and Claude
//! creates the task with its own tool.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};
use specta::Type;
use tauri::State;

use crate::state::AppState;
use crate::telemetry;

const SKILL_NAME: &str = "recall-harvest";

#[derive(Serialize, Type)]
pub struct ClaudeSetupStatus {
    /// False where the setup is not offered (Windows: Claude runs commands
    /// through Git Bash there, and the path/permission rule story is untested).
    pub supported: bool,
    /// Shell command that runs the bundled CLI; `None` when no `recall` binary
    /// sits next to the app (e.g. a `tauri dev` build that never built it).
    pub command: Option<String>,
    /// Permission rule added to Claude's settings, for `command`.
    pub allow_rule: Option<String>,
    pub skill_path: String,
    pub settings_path: String,
    /// The skill file exists with exactly the content this build would write.
    pub skill_installed: bool,
    /// Claude's settings already allow `allow_rule`.
    pub rule_installed: bool,
}

#[tauri::command]
#[specta::specta]
pub fn get_claude_setup_status() -> Result<ClaudeSetupStatus, String> {
    let claude = claude_dir()?;
    let command = cli_command();
    let skill_path = skill_path(&claude);
    let settings_path = claude.join("settings.json");

    let skill_installed = command
        .as_deref()
        .is_some_and(|cmd| fs::read_to_string(&skill_path).is_ok_and(|s| s == render_skill(cmd)));
    let allow_rule = command.as_deref().map(allow_rule);
    let rule_installed = allow_rule.as_deref().is_some_and(|rule| {
        read_settings(&settings_path).is_ok_and(|mut s| matches!(add_allow_rule(&mut s, rule), Ok(false)))
    });

    Ok(ClaudeSetupStatus {
        supported: cfg!(not(windows)),
        command,
        allow_rule,
        skill_path: skill_path.display().to_string(),
        settings_path: settings_path.display().to_string(),
        skill_installed,
        rule_installed,
    })
}

/// Write the skill, allow the CLI in Claude's settings, then open Claude with
/// a prefilled request to schedule the overview on weekdays at `time` (`HH:MM`).
#[tauri::command]
#[specta::specta]
pub fn setup_claude(state: State<'_, AppState>, time: String) -> Result<(), String> {
    let result = run_setup(&time);
    telemetry::record(&state, if result.is_ok() { "claude.setup.ok" } else { "claude.setup.error" });
    result
}

fn run_setup(time: &str) -> Result<(), String> {
    if cfg!(windows) {
        return Err("Setting up Claude is not supported on Windows yet".into());
    }
    let (hour, minute) = parse_time(time)?;
    let command = cli_command().ok_or("The recall command-line tool was not found next to the app")?;
    let claude = claude_dir()?;

    write_atomically(&skill_path(&claude), &render_skill(&command))?;

    let settings_path = claude.join("settings.json");
    let mut settings = read_settings(&settings_path)?;
    if add_allow_rule(&mut settings, &allow_rule(&command))? {
        let mut json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
        json.push('\n');
        write_atomically(&settings_path, &json)?;
    }

    tauri_plugin_opener::open_url(schedule_link(&command, hour, minute), None::<&str>)
        .map_err(|e| format!("Could not open Claude: {e}"))
}

/// Claude Code's config folder. `CLAUDE_CONFIG_DIR` is ignored on purpose: an
/// app started from the Dock does not see shell environment variables.
fn claude_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".claude"))
        .ok_or_else(|| "failed to resolve the home directory".to_string())
}

fn skill_path(claude: &Path) -> PathBuf {
    claude.join("skills").join(SKILL_NAME).join("SKILL.md")
}

/// The `recall` binary bundled next to the app's own executable, as a shell
/// command. Using the absolute path means it works without being on `PATH`,
/// and the permission rule matches exactly what Claude runs.
fn cli_command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let cli = exe.parent()?.join(format!("recall{}", std::env::consts::EXE_SUFFIX));
    cli.is_file().then(|| shell_quote(&cli.to_string_lossy()))
}

/// Single-quote `s` for a POSIX shell unless it is made only of safe characters.
fn shell_quote(s: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "/._-+".contains(c);
    if !s.is_empty() && s.chars().all(safe) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

fn allow_rule(command: &str) -> String {
    format!("Bash({command} timeline *)")
}

fn parse_time(time: &str) -> Result<(u32, u32), String> {
    let invalid = || format!("Invalid time `{time}` (expected HH:MM)");
    let (h, m) = time.split_once(':').ok_or_else(invalid)?;
    let hour: u32 = h.parse().map_err(|_| invalid())?;
    let minute: u32 = m.parse().map_err(|_| invalid())?;
    if hour > 23 || minute > 59 {
        return Err(invalid());
    }
    Ok((hour, minute))
}

fn render_skill(command: &str) -> String {
    format!(
        r#"---
name: {SKILL_NAME}
description: Summarise the user's work from their Recall activity timeline (git commits, pull requests, Jira, Zulip, calendar) so they can fill in Harvest. Use when they ask what they worked on, want a Harvest or time-tracking overview, or a scheduled Recall task asks for one.
allowed-tools: {rule}
---

<!-- Managed by the Recall app. "Set up with Claude" in Recall's settings rewrites this file. -->

# Recall: Harvest overview

Recall is a desktop app that collects the user's activity from their tools into one timeline per day. Its command-line tool prints that timeline:

```
{command} timeline --date last-workday --format markdown
```

Run that command, changing only the options below. Do not read Recall's database or settings files, and do not call GitHub, Jira, Zulip or calendar APIs yourself: the command is the only way in.

Options:

- `--date DAY` for one day, or `--from DAY --to DAY` for a range. DAY is `YYYY-MM-DD`, `today`, `yesterday` or `last-workday` (the most recent weekday before today, so Friday when run on a Monday).
- `--format markdown` (easiest to read) or `--format json`.

The output has one section per day and one bullet per event: local time, source, title, details and link. Events marked "logged in Harvest" (`"logged": true` in JSON) are already entered: leave them out.

Exit status:

- `0`: the timeline is complete.
- `2`: some sources failed and are listed at the top under "Incomplete". Still write the overview, but say clearly which sources are missing.
- `1`: an error. Report the message; do not try to work around it.

## Writing the overview

Make one table per day with the columns **Ticket/project**, **Task**, **Hours** and **Reasoning**, ending with a row for the day's total. It is used to fill in Harvest, so:

- Whenever activity can be tied to a Jira ticket ID (e.g. ABC-123), group and label it by that ID. Look for it in commit messages, branch names, PR titles and Jira links, but don't guess. Group the rest by project or repository.
- Within a ticket or project, split separate tasks into their own rows; routine related items (such as a batch of dependency-update PRs) may share one.
- Estimate hours in quarter hours (0.25), from meeting times and the span between related events: the events are only traces of work that also happens between them. Say in **Reasoning** what an estimate rests on, and flag rough guesses.
- Point out long stretches with no events instead of guessing what happened.
"#,
        rule = allow_rule(command),
    )
}

/// `claude://` link that opens a new Code session with the scheduling request
/// typed in (not sent).
fn schedule_link(command: &str, hour: u32, minute: u32) -> String {
    let prompt = format!(
        "Create a scheduled task titled \"Recall: Harvest overview\" that runs every weekday at \
         {hour:02}:{minute:02} (cron `{minute} {hour} * * 1-5`), with this prompt:\n\n\
         Use the {SKILL_NAME} skill: run `{command} timeline --date last-workday --format markdown` \
         and turn the output into the Harvest overview the skill describes (one table per day: \
         ticket/project, task, hours in quarter hours, reasoning). Leave out events already \
         marked as logged in Harvest. If some sources failed, say which."
    );
    format!("claude://code/new?q={}", urlencoding::encode(&prompt))
}

/// Claude's settings as JSON. A missing or blank file is an empty object; a
/// file that is not valid JSON is an error, so it is never overwritten.
fn read_settings(path: &Path) -> Result<Value, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Value::Object(Map::new())),
        Err(e) => return Err(format!("Could not read {}: {e}", path.display())),
    };
    if text.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(&text)
        .map_err(|e| format!("{} is not valid JSON, so it was left alone: {e}", path.display()))
}

/// Add `rule` to `permissions.allow`, creating either level when missing.
/// Returns whether anything changed. Refuses (rather than clobbering) when the
/// file has an unexpected shape.
fn add_allow_rule(settings: &mut Value, rule: &str) -> Result<bool, String> {
    let shape = || "Claude's settings.json has an unexpected shape, so it was left alone".to_string();
    let root = settings.as_object_mut().ok_or_else(shape)?;
    let permissions = root
        .entry("permissions")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(shape)?;
    let allow = permissions
        .entry("allow")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(shape)?;
    if allow.iter().any(|r| r.as_str() == Some(rule)) {
        return Ok(false);
    }
    allow.push(Value::String(rule.to_string()));
    Ok(true)
}

/// Write via a sibling temp file and a rename, so a crash never leaves a
/// half-written file behind for Claude to read.
fn write_atomically(path: &Path, contents: &str) -> Result<(), String> {
    let dir = path.parent().ok_or_else(|| format!("{} has no parent folder", path.display()))?;
    fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let tmp = path.with_extension("recall-tmp");
    fs::write(&tmp, contents).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CMD: &str = "/Applications/recall.app/Contents/MacOS/recall";

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("recall-claude-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn shell_quote_leaves_plain_paths_alone() {
        assert_eq!(shell_quote(CMD), CMD);
    }

    #[test]
    fn shell_quote_quotes_spaces_and_single_quotes() {
        assert_eq!(shell_quote("/Users/a b/recall"), "'/Users/a b/recall'");
        assert_eq!(shell_quote("/it's/recall"), r"'/it'\''s/recall'");
    }

    #[test]
    fn allow_rule_covers_every_timeline_invocation() {
        assert_eq!(allow_rule(CMD), format!("Bash({CMD} timeline *)"));
    }

    #[test]
    fn parse_time_accepts_hh_mm_only() {
        assert_eq!(parse_time("08:30").unwrap(), (8, 30));
        assert_eq!(parse_time("23:59").unwrap(), (23, 59));
        for bad in ["8.30", "24:00", "12:60", "", "ab:cd", "08:30:00"] {
            assert!(parse_time(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn skill_has_frontmatter_and_the_exact_command() {
        let skill = render_skill(CMD);
        assert!(skill.starts_with(&format!("---\nname: {SKILL_NAME}\ndescription: ")));
        assert!(skill.contains(&format!("allowed-tools: Bash({CMD} timeline *)\n---\n")));
        assert!(skill.contains(&format!("{CMD} timeline --date last-workday --format markdown")));
    }

    #[test]
    fn schedule_link_prefills_a_weekday_cron_and_the_command() {
        let link = schedule_link(CMD, 8, 5);
        let q = link.strip_prefix("claude://code/new?q=").expect("Code deep link");
        let prompt = urlencoding::decode(q).unwrap();
        assert!(prompt.contains("every weekday at 08:05 (cron `5 8 * * 1-5`)"), "{prompt}");
        assert!(prompt.contains(&format!("run `{CMD} timeline --date last-workday --format markdown`")));
        assert!(!q.contains(' ') && !q.contains('\n'), "prompt must be URL-encoded");
    }

    #[test]
    fn add_allow_rule_creates_missing_levels() {
        let mut s = json!({});
        assert!(add_allow_rule(&mut s, "Bash(x timeline *)").unwrap());
        assert_eq!(s, json!({ "permissions": { "allow": ["Bash(x timeline *)"] } }));
    }

    #[test]
    fn add_allow_rule_appends_once_and_keeps_everything_else() {
        let mut s = json!({
            "model": "opus",
            "permissions": { "allow": ["Bash(ls *)"], "deny": ["Read(.env)"] }
        });
        assert!(add_allow_rule(&mut s, "Bash(x timeline *)").unwrap());
        assert!(!add_allow_rule(&mut s, "Bash(x timeline *)").unwrap());
        assert_eq!(
            s,
            json!({
                "model": "opus",
                "permissions": { "allow": ["Bash(ls *)", "Bash(x timeline *)"], "deny": ["Read(.env)"] }
            })
        );
    }

    #[test]
    fn add_allow_rule_refuses_unexpected_shapes() {
        for mut s in [json!([]), json!({ "permissions": [] }), json!({ "permissions": { "allow": "x" } })] {
            assert!(add_allow_rule(&mut s, "r").is_err(), "{s}");
        }
    }

    #[test]
    fn read_settings_treats_missing_and_blank_files_as_empty() {
        let dir = temp_dir();
        assert_eq!(read_settings(&dir.join("settings.json")).unwrap(), json!({}));
        fs::write(dir.join("blank.json"), " \n").unwrap();
        assert_eq!(read_settings(&dir.join("blank.json")).unwrap(), json!({}));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_settings_rejects_invalid_json() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(read_settings(&path).unwrap_err().contains("not valid JSON"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rewritten_settings_keep_the_users_key_order() {
        let mut s: Value = serde_json::from_str(r#"{"zeta": 1, "alpha": 2}"#).unwrap();
        add_allow_rule(&mut s, "r").unwrap();
        let out = serde_json::to_string(&s).unwrap();
        assert!(out.starts_with(r#"{"zeta":1,"alpha":2,"permissions""#), "{out}");
    }

    #[test]
    fn write_atomically_creates_folders_and_leaves_no_temp_file() {
        let dir = temp_dir();
        let path = dir.join("skills").join(SKILL_NAME).join("SKILL.md");
        write_atomically(&path, "hello").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
        assert!(!path.with_extension("recall-tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
