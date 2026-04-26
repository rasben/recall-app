<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands, type SettingsJira, type JiraEvent } from "../../bindings";
  import PasswordInput from "../ui/PasswordInput.svelte";
  import { t } from "$lib/i18n.svelte";

  const defaultSiteUrl = "https://reload.atlassian.net";

  const eventTypeMap: Record<string, { type: JiraEvent; labelKey: string }> = {
    commentWritten: { type: "CommentWritten", labelKey: "settings.jira.event.comment_written" },
    issueCreated: { type: "IssueCreated", labelKey: "settings.jira.event.issue_created" },
    issueCompleted: { type: "IssueCompleted", labelKey: "settings.jira.event.issue_completed" },
    mentioned: { type: "Mentioned", labelKey: "settings.jira.event.mentioned" },
  };

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

  async function toggleEvent(type: JiraEvent, checked: boolean) {
    const original = settings.enabled_events ?? [];
    const nextEvents = checked
      ? (original.includes(type) ? original : [...original, type])
      : original.filter((e) => e !== type);
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

    <Label class="mb-2">{t("settings.jira.events_label")}</Label>
    <div class="flex flex-col gap-2">
      {#each Object.entries(eventTypeMap) as [, { type, labelKey }]}
        <div class="flex items-center gap-2">
          <Checkbox
            id="jira-event-{type}"
            checked={enabledEvents.includes(type)}
            onCheckedChange={(v) => toggleEvent(type, v === true)}
          />
          <Label for="jira-event-{type}">{t(labelKey as Parameters<typeof t>[0])}</Label>
        </div>
      {/each}
    </div>
    {/if}
  {/if}
</fieldset>
