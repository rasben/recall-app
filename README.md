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
- Zulip - expanded data
  - Read messages (nice-to-have)
- Performance
  - When opening app, load the last 7 days automatically
- UI
  - In the calendar view, use colour backgrounds to show how busy a day has been
    - Grey = Not loaded yet/no data
  - Add loading splash screen when app is being opened
    - When opening the app, a "hidden" loading is happening, where you cant click anything.
    - Add some kind of animation/transistion, like how Netflix has a transistion when opened
  - If no datasources set, show a message pointing user to settings page
- Privacy/Ease-of-mind
  - Add a screen, that shows the last 50 commands that has been run
    - E.g., the terminal commands that has been run by git data sources, or the APIs called by Jira datasources
- Fun (nice-to-have)
  - Add more transitions and animations
  - Add a TUI
    - Either a real TUI, or a fake one, making the app easily navigated with keyboard
