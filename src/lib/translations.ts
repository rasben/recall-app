// The default AI-export prompts live in a JSON file so the Rust side (the
// `recall` CLI) can embed the same text.
import exportPrompts from "./export-prompts.json";

export type Lang = "en" | "da";

const en = {
  // Language switcher
  "lang.en": "English",
  "lang.da": "Danish",

  // Welcome screen
  "welcome.tagline": "What the hell did I do last week?",
  "welcome.description1":
    "Recall pulls activity from your tools into a single timeline for any given day — so there is one less thing that makes you wish to jump out the window.",
  "welcome.description2": "Pick a date. See what you actually did.",
  "welcome.description2.done": "Done.",
  "welcome.sources_label": "Data sources",
  "welcome.planned": "planned",
  "welcome.cta_setup": "Set up data sources →",
  "welcome.cta_skip": "Skip for now, explore first",

  // Page shell
  "page.open_settings": "Open settings",
  "page.close_settings": "Close settings",
  "page.new_version.title": "New version available: v{version}",
  "page.new_version.description": "You're running v{current}",
  "page.new_version.download": "Download",

  // Settings
  "settings.title": "Settings",
  "settings.tab.general": "General",
  "settings.tab.sources": "Data sources",
  "settings.tab.ai": "AI",
  "settings.tab.system": "System",
  "settings.language": "Language",
  "settings.language.error": "Could not set language.",
  "settings.go_to_data":
    "Click here to see what the hell you've spent your time on",

  // Theme
  "settings.theme.ui": "Interface",
  "settings.theme.legend": "Theme",
  "settings.theme.light": "Light",
  "settings.theme.dark": "Dark",
  "settings.theme.system": "System",
  "settings.theme.saved": "Theme updated!",
  "settings.theme.error": "Failed to save theme",

  // Test connection (shared)
  "settings.test_connection": "Test connection",
  "settings.test_connection.ok": "Connected",

  // Git
  "settings.git.legend": "Local Git commits",
  "settings.git.enable": "Enable local git source",
  "settings.git.path_label": "Directory to scan for git repos",
  "settings.git.path_hint": "Will scan {path} for git repositories.",
  "settings.git.browse": "Browse…",
  "settings.git.error_enable": "Could not enable Git source",
  "settings.git.error_path": "Could not save Git path",
  "settings.git.saved_path": "Git path saved!",

  // GitHub
  "settings.github.legend": "GitHub",
  "settings.github.enable": "Enable GitHub source",
  "settings.github.username": "GitHub username",
  "settings.github.username_placeholder": "your-github-username",
  "settings.github.token": "Personal Access Token (PAT)",
  "settings.github.token_placeholder": "ghp_...",
  "settings.github.token_description":
    'Create a <a class="underline font-medium text-foreground" href="https://github.com/settings/tokens/new" target="_blank" rel="noreferrer">fine-grained PAT</a>. For public repos, no extra permissions are needed. For private repos, grant read-only access to <strong>Contents</strong>.',
  "settings.github.events_label": "Events to show",
  "settings.github.no_events": "No events chosen",
  "settings.github.saved": "GitHub settings saved",
  "settings.github.error_enable": "Could not enable GitHub source",
  "settings.github.error_save": "Could not save GitHub settings",
  "settings.github.error_events": "Could not set chosen events",
  "settings.github.event.pull_request": "Pull Request (PR)",
  "settings.github.event.pr_review": "PR: Review",
  "settings.github.event.pr_review_comment": "PR: Review Comment",
  "settings.github.event.issue": "Issue",
  "settings.github.event.issue_comment": "Issue: Comment",
  "settings.github.event.push": "Push (commits)",
  "settings.github.event.push_description":
    "Shows commits you push to GitHub — on any branch, before they are merged. A good alternative to Local Git, especially on Windows where shell-based git scanning can be unreliable.",
  "settings.github.api_limit_notice":
    "GitHub's API only returns your ~300 most recent events (last 90 days). Older days may be empty even if you were active.",

  // Git
  "settings.git.github_notice":
    "This does not work well on Windows.<br/> Enable the <strong>Push (commits)</strong> event in the GitHub source instead.",

  // Jira
  "settings.jira.legend": "Jira",
  "settings.jira.enable": "Enable Jira source",
  "settings.jira.site_url": "Jira site URL",
  "settings.jira.email": "Atlassian account email",
  "settings.jira.token": "Atlassian API token",
  "settings.jira.token_placeholder": "Create a token…",
  "settings.jira.token_description":
    'Use an <a class="underline font-medium text-foreground" href="https://id.atlassian.com/manage-profile/security/api-tokens" target="_blank" rel="noreferrer">Atlassian API token</a> with your Atlassian account email (Jira Cloud).',
  "settings.jira.events_label": "Events to show",
  "settings.jira.no_events": "No events chosen",
  "settings.jira.error_save": "Could not save Jira settings",
  "settings.jira.saved_url": "Jira site URL saved",
  "settings.jira.saved_email": "Email saved",
  "settings.jira.error_token": "Could not save API token",
  "settings.jira.saved_token": "API token saved",
  "settings.jira.error_events": "Could not update event types",
  "settings.jira.event.comment_written": "Comments I posted",
  "settings.jira.event.issue_created": "Tickets I created",
  "settings.jira.event.issue_completed": "Tickets moved to Done",
  "settings.jira.event.mentioned": "I was @mentioned",

  // Zulip
  "settings.zulip.legend": "Zulip",
  "settings.zulip.enable": "Enable Zulip source",
  "settings.zulip.email": "Zulip account email",
  "settings.zulip.api_key": "API key",
  "settings.zulip.api_key_placeholder": "Your Zulip API key…",
  "settings.zulip.realm_url": "Realm URL",
  "settings.zulip.token_description":
    "Find your API key in Zulip under <strong>Settings → Account &amp; privacy → API key</strong>.",
  "settings.zulip.error_save": "Could not save Zulip settings",
  "settings.zulip.saved_url": "Zulip realm URL saved",
  "settings.zulip.saved_email": "Email saved",
  "settings.zulip.error_api_key": "Could not save API key",
  "settings.zulip.saved_api_key": "API key saved",

  // Calendar
  "settings.calendar.legend": "Calendar",
  "settings.calendar.enable": "Enable Calendar",
  "settings.calendar.ical_url": "iCal URL",
  "settings.calendar.email_label": "Your email addresses",
  "settings.calendar.email_hint":
    "Used to hide meetings you've declined. Add one per account if your iCal feeds span multiple accounts. Leave blank to show all meetings.",
  "settings.calendar.add_email": "Add another email",
  "settings.calendar.add_ical": "Add another iCal",
  "settings.calendar.remove": "Remove",
  "settings.calendar.syncing": "Syncing…",
  "settings.calendar.sync_error": "Sync error: {error}",
  "settings.calendar.last_synced": "Last synced: {time}",
  "settings.calendar.never": "Never",
  "settings.calendar.error_save": "Could not save Calendar settings",
  "settings.calendar.saved_url": "Calendar URL saved",
  "settings.calendar.saved_email": "Email saved",
  "settings.calendar.description":
    'Find this in <a href="https://calendar.google.com/calendar/r/settings" target="_blank" rel="noopener noreferrer" class="underline">Google Calendar Settings</a>.<br />Click on a calendar → scroll to <strong>"Secret address in iCal format"</strong>.<br />Keep it private — anyone with this URL can read your calendar.',

  // System
  "settings.system.legend": "System",
  "settings.cache.clearing": "Clearing…",
  "settings.cache.clear": "Clear all caches",
  "settings.cache.day": "cached day",
  "settings.cache.days": "cached days",
  "settings.cache.on_disk": "on disk",
  "settings.cache.error": "Failed to clear caches",
  "settings.cache.cleared": "Caches cleared",

  // Welcome
  "settings.welcome.show": "Show setup guide",
  "settings.welcome.description": "i get it. it's a really cool h1 effect.",

  // Timeline
  "timeline.no_sources":
    "No data sources enabled. Enable atleast one in the settings.",
  "timeline.no_activity": "No activity found for this day.",
  "timeline.loading": "Loading",
  "timeline.today": "Today",
  "timeline.refresh": "Refresh this day",
  "timeline.pick_date": "Pick a date",
  "timeline.keyboard_hint": "← → switch day, T = today",
  "timeline.load_month": "Load all of {month}",
  "timeline.loading_month": "Loading…",
  "timeline.less_more_activity": "less → more activity",
  "timeline.open_link": "Open",
  "timeline.commit_burst": "{count} commits — possible rebase",
  "timeline.dependabot_burst": "{count} Dependabot updates",
  "timeline.commit_burst_partial_done": "{done}/{count} logged",
  "timeline.commit_burst_expand": "Show commits",
  "timeline.commit_burst_collapse": "Hide commits",
  "timeline.open_link_for": "Open link: {title}",
  "timeline.mark_logged": "Mark as logged in Harvest",
  "timeline.mark_not_logged": "Mark as not logged in Harvest",
  "timeline.group_mode": "Group activity by",
  "timeline.view_by_time": "By time",
  "timeline.view_by_task": "By task",
  "timeline.group_expand": "Show details",
  "timeline.group_collapse": "Hide details",
  "timeline.group_items": "{count} items",
  "timeline.open_in_jira": "Open in Jira",
  "timeline.idle": "{duration} break",

  "export.button": "Export",
  "export.title": "Export activity",
  "export.preset_day": "Selected day",
  "export.preset_week": "Last 7 days",
  "export.preset_month": "Last 30 days",
  "export.start": "Start",
  "export.end": "End",
  "export.format": "Format",
  "export.format_markdown": "Markdown",
  "export.format_json": "JSON",
  "export.include_prompt": "Add a prompt intro",
  "export.prompt_hint":
    "Prepends an instruction so the copied text works as a ready-made AI prompt. Turn off to copy just the activity data.",
  "export.edit_prompt": "Edit prompt in settings",
  "export.partial": "Copied {count} events, but some sources failed: {sources}",
  "export.md_title": "Activity timeline",
  "export.md_intro":
    "Exported from Recall, a personal work-tracking app. Times are local. Each bullet is one tracked activity (commit, PR, calendar event, ticket update, message, …).",
  "export.md_no_activity": "No tracked activity.",
  "export.prompt": exportPrompts.en,
  "settings.export.title": "AI export prompt",
  "settings.export.description":
    "The intro prepended to a Markdown export when “Add a prompt intro” is on. Claude's scheduled Harvest overview follows it too. Leave it as the default to follow the app language, or write your own. Save an empty box to fall back to the default.",
  "settings.export.save": "Save prompt",
  "settings.export.revert": "Revert to default",
  "settings.export.saved": "Prompt saved",
  "settings.export.error": "Could not save the prompt.",

  "settings.cli.title": "Command line",
  "settings.cli.description":
    "The recall command prints your timeline in a terminal, for scripts and AI tools such as Claude. Run recall help to see how.",
  "settings.cli.install_hint": "Install it so a terminal can find it. This creates a link at",
  "settings.cli.install": "Install the recall command",
  "settings.cli.update": "Point it at this copy of Recall",
  "settings.cli.uninstall": "Uninstall the recall command",
  "settings.cli.linked": "The recall command is installed at",
  "settings.cli.stale":
    "The recall command points to another copy of Recall (perhaps an older one, or one you moved).",
  "settings.cli.foreign":
    "already exists and belongs to another program, so Recall leaves it alone. You can still run the command by its full path.",
  "settings.cli.on_path": "The recall command was installed with the app. Open a terminal and run recall help.",
  "settings.cli.manual": "To run it as recall, link this file into a folder on your PATH:",
  "settings.cli.installed_toast": "Installed. Open a new terminal window and run recall help.",
  "settings.cli.uninstalled_toast": "The recall command was uninstalled.",
  "settings.cli.error": "Could not change the recall command.",
  "cli.unavailable.not_in_applications":
    "Move Recall to your Applications folder and open it from there first. Recall is running from the disk image or a temporary copy, which goes away.",
  "cli.unavailable.appimage":
    "The AppImage runs from a temporary folder, so it can't provide a lasting recall command. Install the .deb or .rpm package to use it.",
  "cli.unavailable.windows": "The recall command isn't available on Windows yet.",
  "cli.unavailable.missing":
    "The recall command-line tool was not found next to the app. Install a release build of Recall.",
  "settings.advanced": "Advanced",
  "settings.export.collapse": "Hide",
  "settings.export.using_default": "Default prompt",
  "settings.export.using_custom": "Your prompt",
  "settings.claude.ready": "Claude writes your Harvest overview on weekdays at {time}.",
  "settings.claude.ready_hint":
    "Each morning it covers the days since your last working day (on Mondays: Friday to Sunday), follows your AI export prompt below, and names the session after them.",
  "settings.claude.change_time": "Change time",
  "settings.claude.again": "Run setup again",
  "settings.claude.outdated":
    "This version of Recall sets Claude up a little differently. Update so the morning overview uses the latest instructions.",
  "settings.claude.update": "Update",
  "settings.claude.save_time": "Save and update Claude",
  "settings.claude.cancel": "Cancel",
  "settings.claude.details": "What this changes on your computer",
  "settings.claude.install_cli": "Also install the recall command for the terminal (macOS may ask for your password)",

  "guide.step.sources": "Connect your tools",
  "guide.step.preview": "See it work",
  "guide.step.claude": "Daily overview",
  "guide.sources.title": "Connect your tools",
  "guide.sources.description":
    "Turn on the tools you use. Each needs a token or an address once, and you can change everything later in Settings.",
  "guide.preview.title": "See it work",
  "guide.preview.description": "This is what Recall found for {day}, your last working day.",
  "guide.preview.loading": "Fetching your activity…",
  "guide.preview.errors": "Some tools could not be reached:",
  "guide.preview.empty": "Nothing found for that day. You can connect more tools later in Settings.",
  "guide.claude.title": "A Harvest overview every morning",
  "guide.claude.description":
    "Optional: if you use the Claude desktop app, it can write an overview of your previous working day every weekday morning (on Mondays, the weekend too), ready to copy into Harvest. Skip this if you don't use Claude.",
  "guide.back": "Back",
  "guide.next": "Next",
  "guide.skip": "Skip",
  "guide.finish": "Go to my timeline",
  "settings.claude.title": "Daily overview with Claude",
  "settings.claude.description":
    "Have Claude (the desktop app's Code tab) write a Harvest overview every weekday morning of the days since your last working day (on Mondays: Friday to Sunday), following your AI export prompt. Claude only gets the timeline from Recall's command-line tool, never your GitHub, Jira or Zulip credentials.",
  "settings.claude.step_skill": "Add a skill that tells Claude how to use the recall command:",
  "settings.claude.step_rule": "Let Claude run that command without asking, by adding",
  "settings.claude.step_open":
    "Open Claude with a request to schedule the task. Check it and press Enter to send it.",
  "settings.claude.installed": "done",
  "settings.claude.time_label": "Weekdays at",
  "settings.claude.setup": "Set up with Claude",
  "settings.claude.done": "Claude is open: send the prefilled message to create the task.",
  "settings.claude.error": "Could not set up Claude.",
  "settings.claude.note":
    "Scheduled tasks only run while the Claude app is open and the computer is awake. A missed run happens the next time Claude starts.",

  "export.copy": "Copy to clipboard",
  "export.copying": "Preparing…",
  "export.copied": "Copied {count} events to clipboard",
  "export.empty": "No activity in this range — nothing to copy.",
  "export.invalid_range": "Start date must be on or before the end date.",
  "export.error": "Could not copy export.",
} as const;

export type TranslationKey = keyof typeof en;
export type Translations = Record<TranslationKey, string>;

const da: Translations = {
  "lang.en": "Engelsk",
  "lang.da": "Dansk",

  "welcome.tagline": "Hvad helvede lavede jeg i sidste uge?",
  "welcome.description1":
    "Recall trækker aktivitet fra dine værktøjer ind i én samlet tidslinje for en given dag — så der er én ting mindre der giver dig lyst til at hoppe ud af vinduet.",
  "welcome.description2": "Vælg en dato. Se hvad du faktisk lavede.",
  "welcome.description2.done": "Færdig.",
  "welcome.sources_label": "Datakilder",
  "welcome.planned": "planlagt",
  "welcome.cta_setup": "Opsæt datakilder →",
  "welcome.cta_skip": "Spring over, udforsk først",

  "page.open_settings": "Åbn indstillinger",
  "page.close_settings": "Luk indstillinger",
  "page.new_version.title": "Ny version tilgængelig: v{version}",
  "page.new_version.description": "Du kører v{current}",
  "page.new_version.download": "Hent",

  "settings.title": "Indstillinger",
  "settings.tab.general": "Generelt",
  "settings.tab.sources": "Datakilder",
  "settings.tab.ai": "AI",
  "settings.tab.system": "System",
  "settings.language": "Sprog",
  "settings.language.error": "Kunne ikke sætte sprog",
  "settings.go_to_data":
    "Tryk her, for at se hvad satan du har brugt din tid på",

  "settings.theme.ui": "Interface",
  "settings.theme.legend": "Tema",
  "settings.theme.light": "Lys",
  "settings.theme.dark": "Mørk",
  "settings.theme.system": "System",
  "settings.theme.saved": "Tema opdateret!",
  "settings.theme.error": "Kunne ikke gemme tema",

  "settings.test_connection": "Test forbindelse",
  "settings.test_connection.ok": "Forbundet",

  "settings.git.legend": "Lokale Git-commits",
  "settings.git.enable": "Aktiver lokal git-kilde",
  "settings.git.path_label": "Mappe at scanne for git-repos",
  "settings.git.path_hint": "Scanner {path} for git-repositories.",
  "settings.git.browse": "Gennemse…",
  "settings.git.error_enable": "Kunne ikke aktivere Git-kilde",
  "settings.git.error_path": "Kunne ikke gemme Git-sti",
  "settings.git.saved_path": "Git-sti gemt!",

  "settings.github.legend": "GitHub",
  "settings.github.enable": "Aktiver GitHub-kilde",
  "settings.github.username": "GitHub-brugernavn",
  "settings.github.username_placeholder": "dit-github-brugernavn",
  "settings.github.token": "Personal Access Token (PAT)",
  "settings.github.token_placeholder": "ghp_...",
  "settings.github.token_description":
    'Opret et <a class="underline font-medium text-foreground" href="https://github.com/settings/tokens/new" target="_blank" rel="noreferrer">fine-grained PAT</a>. For offentlige repos kræves ingen ekstra rettigheder. For private repos: giv læseadgang til <strong>Contents</strong>.',
  "settings.github.events_label": "Hændelser at vise",
  "settings.github.no_events": "Ingen hændelser valgt",
  "settings.github.saved": "GitHub-indstillinger gemt",
  "settings.github.error_enable": "Kunne ikke aktivere GitHub-kilde",
  "settings.github.error_save": "Kunne ikke gemme GitHub-indstillinger",
  "settings.github.error_events": "Kunne ikke gemme valgte hændelser",
  "settings.github.event.pull_request": "Pull Request (PR)",
  "settings.github.event.pr_review": "PR: Review",
  "settings.github.event.pr_review_comment": "PR: Reviewkommentar",
  "settings.github.event.issue": "Issue",
  "settings.github.event.issue_comment": "Issue: Kommentar",
  "settings.github.event.push": "Push (commits)",
  "settings.github.event.push_description":
    "Viser commits du pusher til GitHub — på alle branches, inden de merges. Et godt alternativ til Lokal Git, særligt på Windows, hvor shell-baseret git-scanning kan være upålidelig.",
  "settings.github.api_limit_notice":
    "GitHubs API returnerer kun dine ~300 nyeste hændelser (sidste 90 dage). Ældre dage kan være tomme, selvom du var aktiv.",

  "settings.git.github_notice":
    "Dette fungerer ikke godt på windows. <br/>Brug istedet <strong>Push (commits)</strong>-hændelsen under GitHub-kilden i stedet.",

  "settings.jira.legend": "Jira",
  "settings.jira.enable": "Aktiver Jira-kilde",
  "settings.jira.site_url": "Jira-side URL",
  "settings.jira.email": "Atlassian-konto email",
  "settings.jira.token": "Atlassian API-token",
  "settings.jira.token_placeholder": "Opret et token…",
  "settings.jira.token_description":
    'Brug et <a class="underline font-medium text-foreground" href="https://id.atlassian.com/manage-profile/security/api-tokens" target="_blank" rel="noreferrer">Atlassian API-token</a> med din Atlassian-konto email (Jira Cloud).',
  "settings.jira.events_label": "Hændelser at vise",
  "settings.jira.no_events": "Ingen hændelser valgt",
  "settings.jira.error_save": "Kunne ikke gemme Jira-indstillinger",
  "settings.jira.saved_url": "Jira-side URL gemt",
  "settings.jira.saved_email": "Email gemt",
  "settings.jira.error_token": "Kunne ikke gemme API-token",
  "settings.jira.saved_token": "API-token gemt",
  "settings.jira.error_events": "Kunne ikke opdatere hændelsestyper",
  "settings.jira.event.comment_written": "Kommentarer jeg postede",
  "settings.jira.event.issue_created": "Sager jeg oprettede",
  "settings.jira.event.issue_completed": "Sager flyttet til Færdig",
  "settings.jira.event.mentioned": "Jeg blev @nævnt",

  "settings.zulip.legend": "Zulip",
  "settings.zulip.enable": "Aktiver Zulip-kilde",
  "settings.zulip.email": "Zulip-konto email",
  "settings.zulip.api_key": "API-nøgle",
  "settings.zulip.api_key_placeholder": "Din Zulip API-nøgle…",
  "settings.zulip.realm_url": "Realm URL",
  "settings.zulip.token_description":
    "Find din API-nøgle i Zulip under <strong>Indstillinger → Konto &amp; privatliv → API-nøgle</strong>.",
  "settings.zulip.error_save": "Kunne ikke gemme Zulip-indstillinger",
  "settings.zulip.saved_url": "Zulip realm-URL gemt",
  "settings.zulip.saved_email": "Email gemt",
  "settings.zulip.error_api_key": "Kunne ikke gemme API-nøgle",
  "settings.zulip.saved_api_key": "API-nøgle gemt",

  "settings.calendar.legend": "Kalender",
  "settings.calendar.enable": "Aktiver Kalender",
  "settings.calendar.ical_url": "iCal URL",
  "settings.calendar.email_label": "Dine emailadresser",
  "settings.calendar.email_hint":
    "Bruges til at skjule møder du har afvist. Tilføj én pr. konto hvis dine iCal-feeds dækker flere konti. Lad stå tom for at vise alle møder.",
  "settings.calendar.add_email": "Tilføj endnu en email",
  "settings.calendar.add_ical": "Tilføj endnu en iCal",
  "settings.calendar.remove": "Fjern",
  "settings.calendar.syncing": "Synkroniserer…",
  "settings.calendar.sync_error": "Synkroniseringsfejl: {error}",
  "settings.calendar.last_synced": "Sidst synkroniseret: {time}",
  "settings.calendar.never": "Aldrig",
  "settings.calendar.error_save": "Kunne ikke gemme Kalender-indstillinger",
  "settings.calendar.saved_url": "Kalender-URL gemt",
  "settings.calendar.saved_email": "Email gemt",
  "settings.calendar.description":
    'Find det i <a href="https://calendar.google.com/calendar/r/settings" target="_blank" rel="noopener noreferrer" class="underline">Google Kalender-indstillinger</a>.<br />Klik på en kalender → scroll til <strong>"Hemmelig adresse i iCal-format"</strong>.<br />Hold det privat — alle med denne URL kan læse din kalender.',

  "settings.system.legend": "System",
  "settings.cache.clearing": "Rydder…",
  "settings.cache.clear": "Ryd alle cacher",
  "settings.cache.day": "cachet dag",
  "settings.cache.days": "cachede dage",
  "settings.cache.on_disk": "på disk",
  "settings.cache.error": "Kunne ikke rydde cacher",
  "settings.cache.cleared": "Cacher ryddet",

  "settings.welcome.show": "Vis opsætningsguiden",
  "settings.welcome.description": "forstår dig godt. det er en fed h1 effekt.",

  "timeline.no_sources":
    "Ingen datakilder aktiveret. Aktiver mindst én i indstillingerne.",
  "timeline.no_activity": "Ingen aktivitet fundet for denne dag.",
  "timeline.loading": "Indlæser",
  "timeline.today": "I dag",
  "timeline.refresh": "Genindlæs denne dag",
  "timeline.pick_date": "Vælg en dato",
  "timeline.keyboard_hint": "← → skift dag, T = i dag",
  "timeline.load_month": "Indlæs hele {month}",
  "timeline.loading_month": "Indlæser…",
  "timeline.less_more_activity": "mindre → mere aktivitet",
  "timeline.open_link": "Åbn",
  "timeline.commit_burst": "{count} commits — muligvis en rebase",
  "timeline.dependabot_burst": "{count} Dependabot-opdateringer",
  "timeline.commit_burst_partial_done": "{done}/{count} logget",
  "timeline.commit_burst_expand": "Vis commits",
  "timeline.commit_burst_collapse": "Skjul commits",
  "timeline.open_link_for": "Åbn link: {title}",
  "timeline.mark_logged": "Markér som logget i Harvest",
  "timeline.mark_not_logged": "Markér som ikke logget i Harvest",
  "timeline.group_mode": "Gruppér aktivitet efter",
  "timeline.view_by_time": "Efter tid",
  "timeline.view_by_task": "Efter opgave",
  "timeline.group_expand": "Vis detaljer",
  "timeline.group_collapse": "Skjul detaljer",
  "timeline.group_items": "{count} elementer",
  "timeline.open_in_jira": "Åbn i Jira",
  "timeline.idle": "{duration} pause",

  "export.button": "Eksportér",
  "export.title": "Eksportér aktivitet",
  "export.preset_day": "Valgte dag",
  "export.preset_week": "Sidste 7 dage",
  "export.preset_month": "Sidste 30 dage",
  "export.start": "Start",
  "export.end": "Slut",
  "export.format": "Format",
  "export.format_markdown": "Markdown",
  "export.format_json": "JSON",
  "export.include_prompt": "Tilføj en prompt-intro",
  "export.prompt_hint":
    "Tilføjer en instruktion, så den kopierede tekst fungerer som en færdig AI-prompt. Slå fra for kun at kopiere aktivitetsdata.",
  "export.edit_prompt": "Rediger prompt i indstillinger",
  "export.partial": "Kopierede {count} begivenheder, men nogle kilder fejlede: {sources}",
  "export.md_title": "Aktivitetstidslinje",
  "export.md_intro":
    "Eksporteret fra Recall, en personlig arbejdsregistrerings-app. Tidspunkter er lokale. Hvert punkt er én registreret aktivitet (commit, PR, kalenderbegivenhed, sagsopdatering, besked, …).",
  "export.md_no_activity": "Ingen registreret aktivitet.",
  "export.prompt": exportPrompts.da,
  "settings.export.title": "AI-eksport-prompt",
  "settings.export.description":
    "Introteksten der sættes foran en Markdown-eksport, når “Tilføj en prompt-intro” er slået til. Claudes planlagte Harvest-overblik følger den også. Lad den stå som standard for at følge appens sprog, eller skriv din egen. Gem et tomt felt for at falde tilbage til standarden.",
  "settings.export.save": "Gem prompt",
  "settings.export.revert": "Nulstil til standard",
  "settings.export.saved": "Prompt gemt",
  "settings.export.error": "Kunne ikke gemme prompten.",

  "settings.cli.title": "Kommandolinje",
  "settings.cli.description":
    "Kommandoen recall udskriver din tidslinje i en terminal, til scripts og AI-værktøjer som Claude. Kør recall help for at se hvordan.",
  "settings.cli.install_hint": "Installér den, så en terminal kan finde den. Det opretter et link i",
  "settings.cli.install": "Installér recall-kommandoen",
  "settings.cli.update": "Peg den på denne kopi af Recall",
  "settings.cli.uninstall": "Afinstallér recall-kommandoen",
  "settings.cli.linked": "Kommandoen recall er installeret i",
  "settings.cli.stale":
    "Kommandoen recall peger på en anden kopi af Recall (måske en ældre, eller en du har flyttet).",
  "settings.cli.foreign":
    "findes allerede og hører til et andet program, så Recall rører den ikke. Du kan stadig køre kommandoen via dens fulde sti.",
  "settings.cli.on_path": "Kommandoen recall blev installeret sammen med appen. Åbn en terminal, og kør recall help.",
  "settings.cli.manual": "Link denne fil ind i en mappe på din PATH for at køre den som recall:",
  "settings.cli.installed_toast": "Installeret. Åbn et nyt terminalvindue, og kør recall help.",
  "settings.cli.uninstalled_toast": "Kommandoen recall blev afinstalleret.",
  "settings.cli.error": "Kunne ikke ændre recall-kommandoen.",
  "cli.unavailable.not_in_applications":
    "Flyt Recall til din Programmer-mappe, og åbn den derfra først. Recall kører fra disk-imaget eller en midlertidig kopi, som forsvinder.",
  "cli.unavailable.appimage":
    "AppImage'et kører fra en midlertidig mappe, så det kan ikke give en varig recall-kommando. Installér .deb- eller .rpm-pakken for at bruge den.",
  "cli.unavailable.windows": "Kommandoen recall er endnu ikke tilgængelig på Windows.",
  "cli.unavailable.missing":
    "Recalls kommandolinjeværktøj blev ikke fundet ved siden af appen. Installér en release-build af Recall.",
  "settings.advanced": "Avanceret",
  "settings.export.collapse": "Skjul",
  "settings.export.using_default": "Standard-prompt",
  "settings.export.using_custom": "Din prompt",
  "settings.claude.ready": "Claude skriver dit Harvest-overblik på hverdage kl. {time}.",
  "settings.claude.ready_hint":
    "Hver morgen dækker det dagene siden din seneste arbejdsdag (om mandagen: fredag til søndag), følger din AI-eksport-prompt nedenfor og navngiver sessionen efter dem.",
  "settings.claude.change_time": "Skift tidspunkt",
  "settings.claude.again": "Kør opsætningen igen",
  "settings.claude.outdated":
    "Denne version af Recall sætter Claude lidt anderledes op. Opdatér, så morgenoverblikket bruger de nyeste instruktioner.",
  "settings.claude.update": "Opdatér",
  "settings.claude.save_time": "Gem og opdatér Claude",
  "settings.claude.cancel": "Annullér",
  "settings.claude.details": "Hvad det ændrer på din computer",
  "settings.claude.install_cli": "Installér også recall-kommandoen til terminalen (macOS beder måske om din adgangskode)",

  "guide.step.sources": "Forbind dine værktøjer",
  "guide.step.preview": "Se det virke",
  "guide.step.claude": "Dagligt overblik",
  "guide.sources.title": "Forbind dine værktøjer",
  "guide.sources.description":
    "Slå de værktøjer til, du bruger. Hvert af dem skal have en token eller en adresse én gang, og du kan ændre det hele senere under Indstillinger.",
  "guide.preview.title": "Se det virke",
  "guide.preview.description": "Det her fandt Recall for {day}, din seneste arbejdsdag.",
  "guide.preview.loading": "Henter din aktivitet…",
  "guide.preview.errors": "Nogle værktøjer kunne ikke nås:",
  "guide.preview.empty": "Intet fundet for den dag. Du kan forbinde flere værktøjer senere under Indstillinger.",
  "guide.claude.title": "Et Harvest-overblik hver morgen",
  "guide.claude.description":
    "Valgfrit: Hvis du bruger Claude-desktopappen, kan den skrive et overblik over din forrige arbejdsdag hver hverdagsmorgen (om mandagen også weekenden), klar til at kopiere ind i Harvest. Spring over, hvis du ikke bruger Claude.",
  "guide.back": "Tilbage",
  "guide.next": "Næste",
  "guide.skip": "Spring over",
  "guide.finish": "Gå til min tidslinje",
  "settings.claude.title": "Dagligt overblik med Claude",
  "settings.claude.description":
    "Lad Claude (desktop-appens Code-fane) skrive et Harvest-overblik hver hverdagsmorgen over dagene siden din seneste arbejdsdag (om mandagen: fredag til søndag) efter din AI-eksport-prompt. Claude får kun tidslinjen fra Recalls kommandolinjeværktøj, aldrig dine GitHub-, Jira- eller Zulip-legitimationsoplysninger.",
  "settings.claude.step_skill": "Tilføj en skill, der fortæller Claude, hvordan recall-kommandoen bruges:",
  "settings.claude.step_rule": "Lad Claude køre kommandoen uden at spørge ved at tilføje",
  "settings.claude.step_open":
    "Åbn Claude med en anmodning om at planlægge opgaven. Tjek den, og tryk Enter for at sende den.",
  "settings.claude.installed": "udført",
  "settings.claude.time_label": "Hverdage kl.",
  "settings.claude.setup": "Sæt op med Claude",
  "settings.claude.done": "Claude er åben: send den udfyldte besked for at oprette opgaven.",
  "settings.claude.error": "Kunne ikke sætte Claude op.",
  "settings.claude.note":
    "Planlagte opgaver kører kun, mens Claude-appen er åben og computeren er vågen. En mistet kørsel sker næste gang Claude starter.",

  "export.copy": "Kopiér til udklipsholder",
  "export.copying": "Forbereder…",
  "export.copied": "Kopierede {count} begivenheder til udklipsholderen",
  "export.empty": "Ingen aktivitet i denne periode — intet at kopiere.",
  "export.invalid_range": "Startdato skal være på eller før slutdatoen.",
  "export.error": "Kunne ikke kopiere eksporten.",
};

export const translations: Record<Lang, Translations> = { en, da };
