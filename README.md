# Recall

A desktop app that builds a timeline of your workday so you can fill in Harvest without guessing.

Pick a date, and Recall pulls your activity from multiple sources into one view:

- **Calendar (iCal)** — meetings and events (Google Calendar, or any iCal feed)
- **GitHub** — PRs, commits, reviews, issue comments
- **Local git repos** — commits by your author name
- **JIRA** — tickets you interacted with
- **Zulip** — messages you sent
- **Gmail** — emails you sent or replied to *(planned)*
- **Google Drive** — docs you edited *(planned)*

## Setup

```
npm install
npm run tauri dev
```

## Stack

Svelte 5 + SvelteKit 2 frontend, Tauri 2 + Rust backend, SQLite for settings/credentials.


## To-Do's

- Gmail data-source (requires Google OAuth — see AGENTS.md for why this is deferred)
  - Sent emails
  - Read emails
- Google Drive (requires Google OAuth)
  - Edited/Created files
  - Read files
- Zulip - expanded data
  - Messages you've read
- UI
  - In the calendar view, use colour backgrounds to show how busy a day has been
    - Grey = Not loaded yet/no data
    - Make some kind of button where the user can choose to load a whole months of data, to see this.
  - Show how much disk-space the cache-saving is currently using
- Privacy/Ease-of-mind
  - Add a screen, that shows the last 50 commands that has been run
    - E.g., the terminal commands that has been run by git data sources, or the APIs called by Jira datasources
- Fun
  - Add more transitions and animations
  - Add a TUI
    - Either a real TUI, or a fake one, making the app easily navigated with keyboard
