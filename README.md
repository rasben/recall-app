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

- Gmail data-source (nice-to-have)
  - Sent emails
  - Read emails
- Google Drive (nice-to-have)
  - Edited/Creates files
  - Read files
- Zulip - expanded data
  - Messages you've read (nice-to-have)
- Performance
  - When opening app, load the last 7 days automatically
  - Add "clear caches" in settings
- UI
  - Fix "flashbang" effect when opening app at night
    - The app flashes white when opening. 
    - Can we make the default color black? or transparent? or respect system already at that point?
  - Allow timeline items to have links
    - Right now, links conflict with the "cross out" effect. 
      - We need some solution that works, both accessibility- and UX wise.
    - Examples of where links could be useful:
      - Link to the related GH pull request
      - Link to related Zulip thread
      - Link to calendar invite
      - etc.
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
