<script lang="ts">
  import SettingsGit from "./Git.svelte";
  import SettingsUI from "./UI.svelte";
  import SettingsGitHub from "./GitHub.svelte";
  import SettingsCalendar from "./Calendar.svelte";
  import SettingsJira from "./Jira.svelte";
  import SettingsZulip from "./Zulip.svelte";
  import SettingsCache from "./Cache.svelte";
  import { t } from "$lib/i18n.svelte";
</script>

<div class="flex items-center justify-between mb-4">
  <h2 class="font-head text-xl">{t("settings.title")}</h2>
</div>

<SettingsUI />
<SettingsGit />
<SettingsGitHub />
<SettingsCalendar />
<SettingsJira />
<SettingsZulip />
<SettingsCache />
