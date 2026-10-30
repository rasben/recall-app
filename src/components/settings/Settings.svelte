<script lang="ts">
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import SettingsGit from "./Git.svelte";
  import SettingsUI from "./UI.svelte";
  import SettingsGitHub from "./GitHub.svelte";
  import SettingsCalendar from "./Calendar.svelte";
  import SettingsJira from "./Jira.svelte";
  import SettingsZulip from "./Zulip.svelte";
  import SettingsExport from "./Export.svelte";
  import SettingsCommandLine from "./CommandLine.svelte";
  import SettingsClaude from "./Claude.svelte";
  import SettingsSystem from "./System.svelte";
  import { navState } from "$lib/nav-state.svelte";
  import { t } from "$lib/i18n.svelte";

  let { onShowWelcome }: { onShowWelcome?: () => void } = $props();

  type TabId = "general" | "sources" | "ai" | "system";
  let activeTab = $state<TabId>("sources");
  let promptOpen = $state(false);

  /** Which tab a deep-link section (navState.openSettingsSection) lives in. */
  const SECTION_TAB: Record<string, TabId> = { export: "ai" };

  // Another view (e.g. the export popover) can request a specific panel; jump
  // to its tab and clear the request. "export" also unfolds the prompt editor.
  $effect(() => {
    const section = navState.openSettingsSection;
    if (!section) return;
    const tab = SECTION_TAB[section];
    if (tab) activeTab = tab;
    if (section === "export") promptOpen = true;
    navState.openSettingsSection = null;
  });

  const contentClass = "border-0 p-0 [&>*:first-child]:mt-0";
</script>

<div class="flex items-center justify-between mb-4">
  <h2 class="font-head text-xl">{t("settings.title")}</h2>
</div>

<Tabs.Root bind:value={activeTab} class="gap-0">
  <Tabs.List class="h-auto w-full flex-wrap justify-start gap-1">
    <Tabs.Trigger value="general">{t("settings.tab.general")}</Tabs.Trigger>
    <Tabs.Trigger value="sources">{t("settings.tab.sources")}</Tabs.Trigger>
    <Tabs.Trigger value="ai">{t("settings.tab.ai")}</Tabs.Trigger>
    <Tabs.Trigger value="system">{t("settings.tab.system")}</Tabs.Trigger>
  </Tabs.List>

  <Tabs.Content value="general" class={contentClass}>
    <SettingsUI />
  </Tabs.Content>

  <Tabs.Content value="sources" class={contentClass}>
    <SettingsGit />
    <SettingsGitHub />
    <SettingsJira />
    <SettingsZulip />
    <SettingsCalendar />
  </Tabs.Content>

  <Tabs.Content value="ai" class={contentClass}>
    <SettingsClaude />
    <SettingsExport bind:open={promptOpen} />
  </Tabs.Content>

  <Tabs.Content value="system" class={contentClass}>
    <SettingsSystem {onShowWelcome} />
    <h3 class="mt-8 font-head text-sm uppercase tracking-wide text-muted-foreground">{t("settings.advanced")}</h3>
    <SettingsCommandLine />
  </Tabs.Content>
</Tabs.Root>
