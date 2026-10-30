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

use crate::commands::cli_install::{locate_cli, shell_quote, CliAvailability};
use crate::commands::settings::{get_val, save_val};
use crate::commands::settings_export::prompt_languages;
use crate::state::AppState;
use crate::telemetry;

const SKILL_NAME: &str = "recall-harvest";

/// Where Recall remembers the time it last asked Claude to schedule the task
/// (`"08:30"`). Not `settings_`-prefixed: it records what setup did rather
/// than being a setting, so `save_val` must not count it as a settings save.
const SCHEDULE_KEY: &str = "claude_setup_time";

#[derive(Serialize, Type)]
pub struct ClaudeSetupStatus {
    /// Where the bundled CLI is. Setup is only offered when it is stable (see
    /// `CliAvailability::stable`); Windows is not offered because Claude runs
    /// commands through Git Bash there and the path/rule story is untested.
    pub availability: CliAvailability,
    /// Shell command that runs the bundled CLI, by absolute path; `None` when
    /// the location is not stable.
    pub command: Option<String>,
    /// Permission rule added to Claude's settings, for `command`.
    pub allow_rule: Option<String>,
    pub skill_path: String,
    pub settings_path: String,
    /// The skill file exists with exactly the content this build would write.
    pub skill_installed: bool,
    /// Claude's settings already allow `allow_rule`.
    pub rule_installed: bool,
    /// Weekday time (`HH:MM`) of the last successful setup, if any. Claude
    /// owns the task, so this is what Recall asked for, not necessarily what
    /// the task says now.
    pub scheduled_time: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn get_claude_setup_status(state: State<'_, AppState>, lang: String) -> Result<ClaudeSetupStatus, String> {
    let claude = claude_dir()?;
    let (availability, command) = cli_command();
    let skill_path = skill_path(&claude);
    let settings_path = claude.join("settings.json");

    let skill_installed = command
        .as_deref()
        .is_some_and(|cmd| fs::read_to_string(&skill_path).is_ok_and(|s| s == render_skill(cmd, &lang)));
    let allow_rule = command.as_deref().map(allow_rule);
    let rule_installed = allow_rule.as_deref().is_some_and(|rule| {
        read_settings(&settings_path).is_ok_and(|mut s| matches!(add_allow_rule(&mut s, rule), Ok(false)))
    });

    Ok(ClaudeSetupStatus {
        availability,
        command,
        allow_rule,
        skill_path: skill_path.display().to_string(),
        settings_path: settings_path.display().to_string(),
        skill_installed,
        rule_installed,
        scheduled_time: get_val(&state, SCHEDULE_KEY),
    })
}

/// Write the skill, allow the CLI in Claude's settings, then open Claude with
/// a prefilled request to schedule the overview on weekdays at `time` (`HH:MM`).
/// `lang` is the UI language: the run uses that language's built-in export
/// prompt unless the user has written their own.
#[tauri::command]
#[specta::specta]
pub fn setup_claude(state: State<'_, AppState>, time: String, lang: String) -> Result<(), String> {
    let result = run_setup(&time, &lang).and_then(|()| save_val(&state, SCHEDULE_KEY, &time));
    telemetry::record(&state, if result.is_ok() { "claude.setup.ok" } else { "claude.setup.error" });
    result
}

fn run_setup(time: &str, lang: &str) -> Result<(), String> {
    let (hour, minute) = parse_time(time)?;
    if !prompt_languages().any(|l| l == lang) {
        return Err(format!("Unknown language `{lang}`"));
    }
    let command = match cli_command() {
        (_, Some(command)) => command,
        (CliAvailability::Windows, None) => return Err("Setting up Claude is not supported on Windows yet".into()),
        (CliAvailability::NotInApplications, None) => return Err("Move Recall to your Applications folder first".into()),
        (CliAvailability::AppImage, None) => {
            return Err("The AppImage cannot provide a stable recall command; install the .deb or .rpm package".into())
        }
        (_, None) => return Err("The recall command-line tool was not found next to the app".into()),
    };
    let claude = claude_dir()?;

    write_atomically(&skill_path(&claude), &render_skill(&command, lang))?;

    let settings_path = claude.join("settings.json");
    let mut settings = read_settings(&settings_path)?;
    if add_allow_rule(&mut settings, &allow_rule(&command))? {
        let mut json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
        json.push('\n');
        write_atomically(&settings_path, &json)?;
    }

    tauri_plugin_opener::open_url(schedule_link(&command, lang, hour, minute), None::<&str>)
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

/// The bundled `recall` binary as a shell command, by absolute path: it works
/// whether or not the CLI is on `PATH`, and the permission rule matches
/// exactly what Claude runs. `None` unless that path is stable.
fn cli_command() -> (CliAvailability, Option<String>) {
    let (availability, cli) = locate_cli();
    let command = cli
        .filter(|_| availability.stable())
        .map(|cli| shell_quote(&cli.to_string_lossy()));
    (availability, command)
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

/// What the scheduled run executes: every day since the last working day,
/// headed by the user's export prompt (theirs, or `lang`'s built-in default).
/// On Tuesday to Friday that is just yesterday; on Monday it is Friday to
/// Sunday, so weekend work is not lost (the task itself only runs on weekdays).
fn timeline_invocation(command: &str, lang: &str) -> String {
    format!("{command} timeline --from last-workday --to yesterday --format markdown --with-prompt --lang {lang}")
}

fn render_skill(command: &str, lang: &str) -> String {
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
{invocation}
```

It covers every day since the last working day: just yesterday on Tuesday to Friday, Friday to Sunday on a Monday. Run that command, changing only the options below. Do not read Recall's database or settings files, and do not call GitHub, Jira, Zulip or calendar APIs yourself: the command is the only way into that data.

Options:

- `--date DAY` for one day, or `--from DAY --to DAY` for a range. DAY is `YYYY-MM-DD`, `today`, `yesterday` or `last-workday` (the most recent weekday before today, so Friday when run on a Monday).
- `--with-prompt` puts the user's instructions for the overview at the top (see below); `--lang` only picks the language of the built-in instructions.

Exit status:

- `0`: the timeline is complete.
- `2`: some sources failed and are listed under "Incomplete". Still write the overview, but say clearly which sources are missing.
- `1`: an error. Report the message; do not try to work around it.

## Writing the overview

Everything above the first `---` line of the output is the user's own instructions for this overview: their "AI export prompt" from Recall's settings, which they can edit there. Follow them as if they had written them here. Below the line is the timeline: one section per day, one bullet per event (local time, source, title, details, link). Events marked "logged in Harvest" are already entered: leave them out. Leave out days with no tracked activity too (typically a weekend the user did not work); when there was weekend work, give those days their own tables like any other day.

## Naming the session

When the overview is written, rename this session so it can be found later:

```
Harvest · <days> · <gist>
```

`<days>` is what the overview covers (not today), written in the language of the overview: the weekday and date of a single day, e.g. `Harvest · Tue 28 Oct 2026 · BUPL security day`, or the first and last day that had activity when it covers several, e.g. `Harvest · Fri 24 – Sat 25 Oct 2026 · BUPL release`. The gist is two to five words naming the main work. Use your tool for setting the session title if you have one; otherwise start your answer with that line as a heading.
"#,
        rule = allow_rule(command),
        invocation = timeline_invocation(command, lang),
    )
}

/// `claude://` link that opens a new Code session with a request (typed in,
/// not sent) to create the scheduled task, or update it when setup is re-run.
fn schedule_link(command: &str, lang: &str, hour: u32, minute: u32) -> String {
    let prompt = format!(
        "Create a scheduled task titled \"Recall: Harvest overview\" that runs every weekday at \
         {hour:02}:{minute:02} (cron `{minute} {hour} * * 1-5`). If a task with that title already \
         exists, update its schedule and prompt instead of creating a second one. Its prompt:\n\n\
         Use the {SKILL_NAME} skill to write my Harvest overview of every day since my last working \
         day (on a Monday that includes the weekend): run \
         `{invocation}`, follow the instructions at the top of its output, and name the session \
         as the skill describes.",
        invocation = timeline_invocation(command, lang),
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
    fn allow_rule_covers_every_timeline_invocation() {
        assert_eq!(allow_rule(CMD), format!("Bash({CMD} timeline *)"));
        // The rule's prefix matches what the skill and the task run.
        assert!(timeline_invocation(CMD, "da").starts_with(&format!("{CMD} timeline ")));
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
        let skill = render_skill(CMD, "da");
        assert!(skill.starts_with(&format!("---\nname: {SKILL_NAME}\ndescription: ")));
        assert!(skill.contains(&format!("allowed-tools: Bash({CMD} timeline *)\n---\n")));
        assert!(skill.contains(&format!(
            "```\n{CMD} timeline --from last-workday --to yesterday --format markdown --with-prompt --lang da\n```"
        )));
        assert!(skill.contains("Harvest · <days> · <gist>"));
    }

    #[test]
    fn schedule_link_prefills_a_weekday_cron_and_the_command() {
        let link = schedule_link(CMD, "en", 8, 5);
        let q = link.strip_prefix("claude://code/new?q=").expect("Code deep link");
        let prompt = urlencoding::decode(q).unwrap();
        assert!(prompt.contains("every weekday at 08:05 (cron `5 8 * * 1-5`)"), "{prompt}");
        assert!(prompt.contains(&format!(
            "run `{CMD} timeline --from last-workday --to yesterday --format markdown --with-prompt --lang en`"
        )));
        assert!(prompt.contains("update its schedule and prompt instead of creating a second one"));
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
