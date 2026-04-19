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


## Ideas

- When opening app, load the last 7 days
- When opening app, if no datasources set, show a message with pointing to cog
- Save past days in the database, to make re-opening days easier
- Show last 50 commands run, for clarity