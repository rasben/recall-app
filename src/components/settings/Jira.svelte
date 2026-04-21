<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands, type SettingsJira, type JiraEvent } from "../../bindings";
  import PasswordInput from "../ui/PasswordInput.svelte";
  import * as Select from "$lib/components/ui/select/index.js";
  import { t } from "$lib/i18n.svelte";

  const defaultSiteUrl = "https://reload.atlassian.net";

  const eventTypeMap: Record<string, { type: JiraEvent; labelKey: string }> = {
    commentWritten: { type: "CommentWritten", labelKey: "settings.jira.event.comment_written" },
    issueCreated: { type: "IssueCreated", labelKey: "settings.jira.event.issue_created" },
    issueCompleted: { type: "IssueCompleted", labelKey: "settings.jira.event.issue_completed" },
    mentioned: { type: "Mentioned", labelKey: "settings.jira.event.mentioned" },
  };

  function eventTypeLabel(event: JiraEvent): string {
    const entry = Object.values(eventTypeMap).find((e) => e.type === event);
    return entry ? t(entry.labelKey as Parameters<typeof t>[0]) : event;
  }

  let defaultSettings: SettingsJira = {
    enabled: false,
    site_url: defaultSiteUrl,
    email: "",
    api_token: "",
    enabled_events: [
      eventTypeMap.commentWritten.type,
      eventTypeMap.issueCreated.type,
      eventTypeMap.issueCompleted.type,
      eventTypeMap.mentioned.type,
    ],
  };

  let settings = $state<SettingsJira>(defaultSettings);

  let enabled = $state(false);
  let siteUrl = $state(defaultSiteUrl);
  let email = $state("");
  let apiToken = $state("");
  let enabledEvents = $state<JiraEvent[]>([]);

  onMount(() => {
    getSettings();
  });

  async function getSettings() {
    settings = (await commands.getSettingsJira()) ?? defaultSettings;
    enabled = settings.enabled;
    siteUrl = settings.site_url?.trim() || defaultSiteUrl;
    email = settings.email ?? "";
    apiToken = settings.api_token ?? "";
    enabledEvents = settings.enabled_events ?? [];
  }

  async function persist(partial: Partial<SettingsJira>) {
    const next: SettingsJira = { ...settings, ...partial };
    const result = await commands.setSettingsJira(next);
    if (result.status === "error") {
      toast.error(t("settings.jira.error_save"));
      return false;
    }
    settings = next;
    return true;
  }

  async function toggleEnabled(checked: boolean) {
    const original = settings.enabled;
    enabled = checked;
    const ok = await persist({ enabled: checked });
    if (!ok) {
      enabled = original;
    }
  }

  async function saveSiteUrl() {
    const original = settings.site_url ?? defaultSiteUrl;

    const trimmed = siteUrl.trim() || defaultSiteUrl;
    siteUrl = trimmed.replace(/\/+$/, "");
    const ok = await persist({ site_url: siteUrl });
    if (!ok) {
      siteUrl = original;
    } else {
      toast.success(t("settings.jira.saved_url"));
    }
  }

  async function saveEmail() {
    const original = settings.email ?? "";

    const trimmed = email.trim();
    email = trimmed;
    const ok = await persist({ email: trimmed });
    if (!ok) {
      email = original;
    } else {
      toast.success(t("settings.jira.saved_email"));
    }
  }

  async function saveApiToken() {
    const original = settings.api_token ?? "";
    const ok = await persist({ api_token: apiToken });
    if (!ok) {
      apiToken = original;
      toast.error(t("settings.jira.error_token"));
    } else {
      toast.success(t("settings.jira.saved_token"));
    }
  }

  async function setEnabledEvents(value: string[] | undefined) {
    const original = settings.enabled_events ?? [];
    const nextEvents = (value ?? []) as JiraEvent[];
    enabledEvents = nextEvents;
    const ok = await persist({ enabled_events: nextEvents });
    if (!ok) {
      enabledEvents = original;
      settings.enabled_events = original;
      toast.error(t("settings.jira.error_events"));
    }
  }
</script>

<fieldset class="border-2 p-4 mt-6">
  <legend>{t("settings.jira.legend")}</legend>

  <div class="flex items-center gap-2 mb-4">
    <Checkbox
      id="jira-enabled"
      checked={enabled}
      onCheckedChange={(v) => toggleEnabled(v === true)}
    />
    <Label for="jira-enabled">{t("settings.jira.enable")}</Label>
  </div>

  {#if enabled}
    <Label for="jira-site-url" class="mb-2">{t("settings.jira.site_url")}</Label>
    <Input
      id="jira-site-url"
      type="url"
      class="mb-4"
      placeholder={defaultSiteUrl}
      bind:value={siteUrl}
      onblur={saveSiteUrl}
    />

    <Label for="jira-email" class="mb-2">{t("settings.jira.email")}</Label>
    <Input
      id="jira-email"
      type="email"
      autocomplete="username"
      class="mb-4"
      placeholder="you@company.com"
      bind:value={email}
      onblur={saveEmail}
    />

    <PasswordInput
      bind:password={apiToken}
      saveAction={saveApiToken}
      label={t("settings.jira.token")}
      placeholder={t("settings.jira.token_placeholder")}
      inputId="jira-api-token"
      description={t("settings.jira.token_description")}
    />

    {#if apiToken}

    <Label for="jira-enabled-events-trigger" class="mb-2">{t("settings.jira.events_label")}</Label>
    <Select.Root type="multiple" bind:value={enabledEvents} onValueChange={setEnabledEvents}>
      <Select.Trigger id="jira-enabled-events-trigger" class="w-full">
        {enabledEvents.length === 0
          ? t("settings.jira.no_events")
          : [...enabledEvents]
              .sort((a, b) => eventTypeLabel(a).localeCompare(eventTypeLabel(b)))
              .map(eventTypeLabel)
              .join(", ")}
      </Select.Trigger>
      <Select.Content class="max-h-[300px]">
        {#each Object.entries(eventTypeMap) as [, { type, labelKey }]}
          <Select.Item value={type} label={t(labelKey as Parameters<typeof t>[0])} />
        {/each}
      </Select.Content>
    </Select.Root>
    {/if}
  {/if}
</fieldset>
