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

  // Settings
  "settings.title": "Settings",
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
  "settings.calendar.email_label": "Your email address",
  "settings.calendar.email_hint":
    "Used to hide meetings you've declined. Leave blank to show all meetings.",
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

  // Cache
  "settings.cache.legend": "Cache",
  "settings.cache.clearing": "Clearing…",
  "settings.cache.clear": "Clear all caches",
  "settings.cache.day": "cached day",
  "settings.cache.days": "cached days",
  "settings.cache.on_disk": "on disk",
  "settings.cache.error": "Failed to clear caches",
  "settings.cache.cleared": "Caches cleared",

  // Timeline
  "timeline.no_sources":
    "No data sources enabled. Enable atleast one in the settings.",
  "timeline.no_activity": "No activity found for this day.",
  "timeline.loading": "Loading",
  "timeline.today": "Today",
  "timeline.pick_date": "Pick a date",
  "timeline.load_month": "Load all of {month}",
  "timeline.loading_month": "Loading…",
  "timeline.less_more_activity": "less → more activity",
  "timeline.open_link": "Open",
  "timeline.logged_in_harvest": "Logged in Harvest",
  "timeline.not_logged_in_harvest": "Not logged in Harvest",
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

  "settings.title": "Indstillinger",
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
  "settings.calendar.email_label": "Din emailadresse",
  "settings.calendar.email_hint":
    "Bruges til at skjule møder du har afvist. Lad stå tom for at vise alle møder.",
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

  "settings.cache.legend": "Cache",
  "settings.cache.clearing": "Rydder…",
  "settings.cache.clear": "Ryd alle caches",
  "settings.cache.day": "cachet dag",
  "settings.cache.days": "cachede dage",
  "settings.cache.on_disk": "på disk",
  "settings.cache.error": "Kunne ikke rydde caches",
  "settings.cache.cleared": "Caches ryddet",

  "timeline.no_sources":
    "Ingen datakilder aktiveret. Aktiver mindst én i indstillingerne.",
  "timeline.no_activity": "Ingen aktivitet fundet for denne dag.",
  "timeline.loading": "Indlæser",
  "timeline.today": "I dag",
  "timeline.pick_date": "Vælg en dato",
  "timeline.load_month": "Indlæs hele {month}",
  "timeline.loading_month": "Indlæser…",
  "timeline.less_more_activity": "mindre → mere aktivitet",
  "timeline.open_link": "Åbn",
  "timeline.logged_in_harvest": "Logget i Harvest",
  "timeline.not_logged_in_harvest": "Ikke logget i Harvest",
};

export const translations: Record<Lang, Translations> = { en, da };
