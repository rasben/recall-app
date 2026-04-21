<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands, type SettingsJira, type JiraEvent } from "../../bindings";
  import PasswordInput from "../ui/PasswordInput.svelte";
  import * as Select from "$lib/components/ui/select/index.js";

  const defaultSiteUrl = "https://reload.atlassian.net";
  const tokenDescription = `Use an
      <a
              class="underline font-medium text-foreground"
              href="https://id.atlassian.com/manage-profile/security/api-tokens"
              target="_blank"
              rel="noreferrer">Atlassian API token</a
      >
      with your Atlassian account email (Jira Cloud).`;

  const eventTypeMap: Record<string, { type: JiraEvent; label: string }> = {
    commentWritten: { type: "CommentWritten", label: "Comments I posted" },
    issueCreated: { type: "IssueCreated", label: "Tickets I created" },
    issueCompleted: { type: "IssueCompleted", label: "Tickets moved to Done" },
    mentioned: { type: "Mentioned", label: "I was @mentioned" },
  };

  function eventTypeLabel(event: JiraEvent): string {
    return Object.values(eventTypeMap).find((e) => e.type === event)?.label ?? event;
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
      toast.error("Could not save Jira settings");
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
      toast.success("Jira site URL saved");
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
      toast.success("Email saved");
    }
  }

  async function saveApiToken() {
    const original = settings.api_token ?? "";
    const ok = await persist({ api_token: apiToken });
    if (!ok) {
      apiToken = original;
      toast.error("Could not save API token");
    } else {
      toast.success("API token saved");
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
      toast.error("Could not update event types");
    }
  }
</script>

<fieldset class="border-2 p-4 mt-6">
  <legend>Jira</legend>

  <div class="flex items-center gap-2 mb-4">
    <Checkbox
      id="jira-enabled"
      checked={enabled}
      onCheckedChange={(v) => toggleEnabled(v === true)}
    />
    <Label for="jira-enabled">Enable Jira source</Label>
  </div>

  {#if enabled}
    <Label for="jira-site-url" class="mb-2">Jira site URL</Label>
    <Input
      id="jira-site-url"
      type="url"
      class="mb-4"
      placeholder={defaultSiteUrl}
      bind:value={siteUrl}
      onblur={saveSiteUrl}
    />

    <Label for="jira-email" class="mb-2">Atlassian account email</Label>
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
      label="Atlassian API token"
      placeholder="Create a token…"
      inputId="jira-api-token"
      description={tokenDescription}
    />

    {#if apiToken}

    <label for="jira-enabled-events-trigger" class="mb-2 block"> Events to show </label>
    <Select.Root type="multiple" bind:value={enabledEvents} onValueChange={setEnabledEvents}>
      <Select.Trigger id="jira-enabled-events-trigger" class="w-full">
        {enabledEvents.length === 0
          ? "No events chosen"
          : [...enabledEvents]
              .sort((a, b) => eventTypeLabel(a).localeCompare(eventTypeLabel(b)))
              .map(eventTypeLabel)
              .join(", ")}
      </Select.Trigger>
      <Select.Content class="max-h-[300px]">
        {#each Object.entries(eventTypeMap) as [, { type, label }]}
          <Select.Item value={type} {label} />
        {/each}
      </Select.Content>
    </Select.Root>
    {/if}
  {/if}
</fieldset>
