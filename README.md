# Recall

A desktop app that builds a timeline of your workday so you can fill in Harvest without guessing.

Pick a date, and Recall pulls your activity from multiple sources into one view:

- **GitHub** — PRs, commits, reviews, issue comments
- **Google Calendar** — meetings and events
- **Gmail** — emails you sent or replied to
- **Google Drive** — docs you edited
- **Local git repos** — commits by your author name
- **JIRA** — tickets you interacted with
- **Zulip** — messages you sent

## Setup

```
npm install
npm run tauri dev
```

## Stack

Svelte 5 + SvelteKit 2 frontend, Tauri 2 + Rust backend, SQLite for settings/credentials.


## To-Do's

- Gmail data-source
  - Sent emails (must-have)
  - Read emails (nice-to-have)
- Google Calendar
- Google Drive
  - Edited/Creates files (must-have)
  - Read files (nice-to-have)
- Zulip data source
  - Sent messages (must-have)
  - Read messages (nice-to-have)
  - We need to group them together, to not spam the feed
- Performance
  - When opening app, load the last 7 days automatically
- UI
  - If no datasources set, show a message pointing user to settings page
- Privacy/Ease-of-mind
  - Add a screen, that shows the last 50 commands that has been run
    - E.g., the terminal commands that has been run by git data sources, or the APIs called by Jira datasources
- Fun (nice-to-have)
  - Add more transitions and animations
  - Add a TUI
    - Either a real TUI, or a fake one, making the app easily navigated with keyboard
