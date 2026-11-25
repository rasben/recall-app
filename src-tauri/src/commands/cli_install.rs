//! Where the bundled `recall` CLI is, and putting it on the user's `PATH`.
//!
//! The CLI ships next to the app's own executable. Linux `.deb`/`.rpm`
//! packages install both into `/usr/bin`, so it is on `PATH` already; on macOS
//! it sits inside `recall.app`, so the app offers to link it into
//! `/usr/local/bin` (on the default macOS `PATH`), asking for an administrator
//! password when that folder is not writable.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::state::AppState;
use crate::telemetry;

/// Where the macOS link goes. `/usr/local/bin` is in the default `/etc/paths`.
const MAC_LINK: &str = "/usr/local/bin/recall";

/// How the bundled CLI can be reached from a shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum CliAvailability {
    /// macOS app in a stable place: the CLI can be linked onto `PATH`.
    Linkable,
    /// The package installed it into a folder on `PATH` (Linux `/usr/bin`).
    OnPath,
    /// Stable location, but not on `PATH` and nothing to link it with (e.g. a
    /// Linux build run from its build folder): the user links it themselves.
    Manual,
    /// macOS app running from the disk image or a translocated copy: its path
    /// changes once it is moved to Applications, so nothing may point at it.
    NotInApplications,
    /// Running as an AppImage: the CLI is inside a mount that changes every launch.
    AppImage,
    /// Not offered on Windows yet.
    Windows,
    /// No `recall` binary next to the app (e.g. a `tauri dev` build).
    Missing,
}

impl CliAvailability {
    /// Whether the CLI has a stable absolute path that other tools (Claude's
    /// skill and permission rule) may refer to.
    pub(crate) fn stable(self) -> bool {
        matches!(self, Self::Linkable | Self::OnPath | Self::Manual)
    }
}

/// The macOS `PATH` link's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum CliLink {
    NotInstalled,
    /// Points at this app's CLI.
    Installed,
    /// Points at a Recall CLI elsewhere (an older or moved copy of the app).
    Stale,
    /// Something that is not Recall's: left alone.
    Foreign,
}

#[derive(Serialize, Type)]
pub struct CliInstallStatus {
    pub availability: CliAvailability,
    /// The bundled CLI, when found.
    pub cli_path: Option<String>,
    /// Where the link goes (macOS only).
    pub link_path: Option<String>,
    /// State of `link_path`; `None` when there is no link to manage.
    pub link: Option<CliLink>,
}

/// Locate the bundled CLI and classify how it can be reached.
pub(crate) fn locate_cli() -> (CliAvailability, Option<PathBuf>) {
    let exe = std::env::current_exe().ok();
    let cli = exe
        .as_deref()
        .and_then(Path::parent)
        .map(|dir| dir.join(format!("recall{}", std::env::consts::EXE_SUFFIX)));
    let availability = classify(
        std::env::consts::OS,
        cli.as_deref(),
        cli.as_deref().is_some_and(Path::is_file),
        std::env::var_os("APPIMAGE").is_some(),
    );
    (availability, cli)
}

fn classify(os: &str, cli: Option<&Path>, cli_exists: bool, appimage: bool) -> CliAvailability {
    if os == "windows" {
        return CliAvailability::Windows;
    }
    if appimage {
        return CliAvailability::AppImage;
    }
    let Some(cli) = cli.filter(|_| cli_exists) else {
        return CliAvailability::Missing;
    };
    let path = cli.to_string_lossy();
    if os == "macos" {
        if path.starts_with("/Volumes/") || path.contains("/AppTranslocation/") {
            return CliAvailability::NotInApplications;
        }
        return CliAvailability::Linkable;
    }
    let on_path = ["/usr/bin", "/usr/local/bin", "/bin"]
        .iter()
        .any(|dir| cli.parent() == Some(Path::new(dir)));
    if on_path {
        CliAvailability::OnPath
    } else {
        CliAvailability::Manual
    }
}

/// Single-quote `s` for a POSIX shell unless it is made only of safe characters.
pub(crate) fn shell_quote(s: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "/._-+".contains(c);
    if !s.is_empty() && s.chars().all(safe) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_cli_install_status() -> CliInstallStatus {
    let (availability, cli) = locate_cli();
    let linkable = availability == CliAvailability::Linkable;
    CliInstallStatus {
        availability,
        cli_path: cli
            .as_ref()
            .filter(|_| availability != CliAvailability::Missing)
            .map(|p| p.display().to_string()),
        link_path: linkable.then(|| MAC_LINK.to_string()),
        link: cli.filter(|_| linkable).map(|cli| link_state(Path::new(MAC_LINK), &cli)),
    }
}

/// Link the bundled CLI to `/usr/local/bin/recall` (macOS).
#[tauri::command]
#[specta::specta]
pub fn install_cli(state: State<'_, AppState>) -> Result<(), String> {
    let result = linkable_cli().and_then(|cli| install_link(Path::new(MAC_LINK), &cli));
    telemetry::record(&state, if result.is_ok() { "cli.install.ok" } else { "cli.install.error" });
    result
}

/// Remove the `/usr/local/bin/recall` link, if it is Recall's.
#[tauri::command]
#[specta::specta]
pub fn uninstall_cli(state: State<'_, AppState>) -> Result<(), String> {
    let result = linkable_cli().and_then(|cli| uninstall_link(Path::new(MAC_LINK), &cli));
    telemetry::record(&state, if result.is_ok() { "cli.uninstall.ok" } else { "cli.uninstall.error" });
    result
}

fn linkable_cli() -> Result<PathBuf, String> {
    match locate_cli() {
        (CliAvailability::Linkable, Some(cli)) => Ok(cli),
        (CliAvailability::NotInApplications, _) => {
            Err("Move Recall to your Applications folder first".into())
        }
        _ => Err("Installing the recall command is only needed on macOS".into()),
    }
}

/// A link that some copy of Recall made: it points at a `recall` binary inside
/// an app bundle.
fn is_recall_cli(target: &Path) -> bool {
    target.file_name().is_some_and(|n| n == "recall")
        && target.to_string_lossy().contains(".app/Contents/MacOS/")
}

fn link_state(link: &Path, cli: &Path) -> CliLink {
    match fs::symlink_metadata(link) {
        Err(_) => CliLink::NotInstalled,
        Ok(meta) if meta.file_type().is_symlink() => match fs::read_link(link) {
            Ok(target) if target == cli => CliLink::Installed,
            Ok(target) if is_recall_cli(&target) => CliLink::Stale,
            _ => CliLink::Foreign,
        },
        Ok(_) => CliLink::Foreign,
    }
}

fn install_link(link: &Path, cli: &Path) -> Result<(), String> {
    match link_state(link, cli) {
        CliLink::Installed => return Ok(()),
        CliLink::Foreign => {
            return Err(format!("{} already exists and is not Recall's, so it was left alone", link.display()))
        }
        CliLink::NotInstalled | CliLink::Stale => {}
    }
    match replace_link(link, cli) {
        Ok(()) => Ok(()),
        Err(e) if matches!(e.kind(), ErrorKind::PermissionDenied | ErrorKind::NotFound) => {
            let dir = link.parent().unwrap_or(Path::new("/"));
            run_as_admin(&format!(
                "mkdir -p {} && ln -sfn {} {}",
                shell_quote(&dir.to_string_lossy()),
                shell_quote(&cli.to_string_lossy()),
                shell_quote(&link.to_string_lossy()),
            ))
        }
        Err(e) => Err(format!("Could not create {}: {e}", link.display())),
    }
}

/// Swap in a fresh link without the privileged path, when the folder allows it.
fn replace_link(link: &Path, cli: &Path) -> std::io::Result<()> {
    if fs::symlink_metadata(link).is_ok() {
        fs::remove_file(link)?;
    }
    symlink(cli, link)
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink(_target: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(ErrorKind::Unsupported, "symlinks are not supported here"))
}

fn uninstall_link(link: &Path, cli: &Path) -> Result<(), String> {
    match link_state(link, cli) {
        CliLink::NotInstalled => return Ok(()),
        CliLink::Foreign => {
            return Err(format!("{} is not Recall's, so it was left alone", link.display()))
        }
        CliLink::Installed | CliLink::Stale => {}
    }
    match fs::remove_file(link) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            run_as_admin(&format!("rm -f {}", shell_quote(&link.to_string_lossy())))
        }
        Err(e) => Err(format!("Could not remove {}: {e}", link.display())),
    }
}

/// Run `script` with `sh` as root via the standard macOS password prompt.
fn run_as_admin(script: &str) -> Result<(), String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(applescript_admin(script))
        .output()
        .map_err(|e| format!("Could not ask for administrator rights: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("-128") {
        Err("Cancelled".into())
    } else {
        Err(stderr.trim().to_string())
    }
}

/// `do shell script "…" with administrator privileges`, with `script` escaped
/// for an AppleScript string literal.
fn applescript_admin(script: &str) -> String {
    let escaped = script.replace('\\', r"\\").replace('"', "\\\"");
    format!("do shell script \"{escaped}\" with administrator privileges")
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP_CLI: &str = "/Applications/recall.app/Contents/MacOS/recall";

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("recall-link-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn classify_covers_every_platform_case() {
        let p = |s| Some(Path::new(s));
        use CliAvailability::*;
        assert_eq!(classify("windows", p(r"C:\x\recall.exe"), true, false), Windows);
        assert_eq!(classify("linux", p("/tmp/.mount_x/usr/bin/recall"), true, true), AppImage);
        assert_eq!(classify("macos", p(APP_CLI), false, false), Missing);
        assert_eq!(classify("macos", None, false, false), Missing);
        assert_eq!(classify("macos", p(APP_CLI), true, false), Linkable);
        assert_eq!(classify("macos", p("/Volumes/recall/recall.app/Contents/MacOS/recall"), true, false), NotInApplications);
        assert_eq!(
            classify("macos", p("/private/var/folders/x/AppTranslocation/y/d/recall.app/Contents/MacOS/recall"), true, false),
            NotInApplications
        );
        assert_eq!(classify("linux", p("/usr/bin/recall"), true, false), OnPath);
        assert_eq!(classify("linux", p("/home/me/recall/target/release/recall"), true, false), Manual);
    }

    #[test]
    fn only_stable_locations_may_be_referred_to() {
        use CliAvailability::*;
        let stable: Vec<_> = [Linkable, OnPath, Manual, NotInApplications, AppImage, Windows, Missing]
            .into_iter()
            .filter(|a| a.stable())
            .collect();
        assert_eq!(stable, vec![Linkable, OnPath, Manual]);
    }

    #[test]
    fn shell_quote_leaves_plain_paths_alone() {
        assert_eq!(shell_quote(APP_CLI), APP_CLI);
    }

    #[test]
    fn shell_quote_quotes_spaces_and_single_quotes() {
        assert_eq!(shell_quote("/Users/a b/recall"), "'/Users/a b/recall'");
        assert_eq!(shell_quote("/it's/recall"), r"'/it'\''s/recall'");
    }

    #[test]
    fn applescript_escapes_quotes_and_backslashes() {
        assert_eq!(
            applescript_admin(r#"ln -s 'a"b\c' /x"#),
            r#"do shell script "ln -s 'a\"b\\c' /x" with administrator privileges"#
        );
    }

    #[test]
    fn recall_links_are_recognised_by_their_bundle_path() {
        assert!(is_recall_cli(Path::new(APP_CLI)));
        assert!(is_recall_cli(Path::new("/Users/me/Downloads/recall.app/Contents/MacOS/recall")));
        assert!(!is_recall_cli(Path::new("/opt/homebrew/bin/recall")));
        assert!(!is_recall_cli(Path::new("/Applications/recall.app/Contents/MacOS/recall-app")));
    }

    #[test]
    fn install_links_a_missing_command_and_is_idempotent() {
        let dir = temp_dir();
        let link = dir.join("recall");
        let cli = dir.join("recall.app/Contents/MacOS/recall");
        assert_eq!(link_state(&link, &cli), CliLink::NotInstalled);
        install_link(&link, &cli).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), cli);
        assert_eq!(link_state(&link, &cli), CliLink::Installed);
        install_link(&link, &cli).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_replaces_a_link_to_another_copy_of_recall() {
        let dir = temp_dir();
        let link = dir.join("recall");
        symlink(Path::new("/Users/me/Downloads/recall.app/Contents/MacOS/recall"), &link).unwrap();
        assert_eq!(link_state(&link, Path::new(APP_CLI)), CliLink::Stale);
        install_link(&link, Path::new(APP_CLI)).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), Path::new(APP_CLI));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn foreign_commands_are_never_replaced_or_removed() {
        let dir = temp_dir();
        let file = dir.join("recall");
        fs::write(&file, "someone else's").unwrap();
        let link = dir.join("recall-link");
        symlink(Path::new("/opt/homebrew/bin/recall"), &link).unwrap();
        for path in [&file, &link] {
            assert_eq!(link_state(path, Path::new(APP_CLI)), CliLink::Foreign);
            assert!(install_link(path, Path::new(APP_CLI)).is_err());
            assert!(uninstall_link(path, Path::new(APP_CLI)).is_err());
        }
        assert_eq!(fs::read_to_string(&file).unwrap(), "someone else's");
        assert_eq!(fs::read_link(&link).unwrap(), Path::new("/opt/homebrew/bin/recall"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn uninstall_removes_only_recall_links() {
        let dir = temp_dir();
        let link = dir.join("recall");
        uninstall_link(&link, Path::new(APP_CLI)).unwrap(); // nothing there: fine
        symlink(Path::new(APP_CLI), &link).unwrap();
        uninstall_link(&link, Path::new(APP_CLI)).unwrap();
        assert!(fs::symlink_metadata(&link).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
