<script lang="ts">
  import SettingsGit from "./Git.svelte";
  import SettingsUI from "./UI.svelte";
  import SettingsGitHub from "./GitHub.svelte";
  import SettingsCalendar from "./Calendar.svelte";
  import SettingsJira from "./Jira.svelte";
  import SettingsZulip from "./Zulip.svelte";
  import SettingsCache from "./Cache.svelte";
</script>

<h2 class="font-head mb-4 text-xl">
  Settings
</h2>

<SettingsUI />
<SettingsGit />
<SettingsGitHub />
<SettingsCalendar />
<SettingsJira />
<SettingsZulip />
<SettingsCache />
