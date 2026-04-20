<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { toast } from "svelte-sonner";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { commands, type SettingsIcal, type IcalSyncStatus } from "../../bindings";
  import PasswordInput from "../ui/PasswordInput.svelte";

  const defaultSettings: SettingsIcal = {
    enabled: false,
    urls: [],
  };

  let settings = $state<SettingsIcal>(defaultSettings);
  let icalUrls = $state<string[]>([""]);
  let syncStatus = $state<IcalSyncStatus | null>(null);
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  onMount(async () => {
    settings = (await commands.getSettingsIcal()) ?? defaultSettings;
    const saved = settings.urls ?? [];
    icalUrls = saved.length > 0 ? [...saved] : [""];
    if (settings.enabled) refreshSyncStatus();
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
  });

  async function refreshSyncStatus() {
    syncStatus = await commands.getIcalSyncStatus();
    if (syncStatus?.syncing) {
      if (!pollTimer) {
        pollTimer = setInterval(async () => {
          syncStatus = await commands.getIcalSyncStatus();
          if (!syncStatus?.syncing && pollTimer) {
            clearInterval(pollTimer);
            pollTimer = null;
          }
        }, 1500);
      }
    }
  }

  async function persist(partial: Partial<SettingsIcal>) {
    const next: SettingsIcal = { ...settings, ...partial };
    const result = await commands.setSettingsIcal(next);
    if (result.status === "error") {
      toast.error("Could not save Calendar settings");
      return false;
    }
    settings = next;
    if (next.enabled) refreshSyncStatus();
    return true;
  }

  async function toggleEnabled(checked: boolean) {
    const original = settings.enabled;
    const ok = await persist({ enabled: checked });
    if (!ok) settings = { ...settings, enabled: original };
  }

  async function saveUrls() {
    const cleaned = icalUrls.map((u) => u.trim()).filter((u) => u.length > 0);
    const ok = await persist({ urls: cleaned });
    if (ok) toast.success("Calendar URL saved");
  }

  function addUrl() {
    icalUrls = [...icalUrls, ""];
  }

  function removeUrl(index: number) {
    icalUrls = icalUrls.filter((_, i) => i !== index);
    saveUrls();
  }

  function formatSyncTime(ts: number | null | undefined): string {
    if (!ts) return "Never";
    return new Date(ts).toLocaleTimeString();
  }
</script>

<fieldset class="border-2 p-4 mt-4">
  <legend class="mb-2">Calendar</legend>

  <div class="flex items-center gap-2 mb-4">
    <Checkbox
      id="calendar-enabled"
      checked={settings.enabled}
      onCheckedChange={(v) => toggleEnabled(v === true)}
    />
    <Label for="calendar-enabled">Enable Calendar</Label>
  </div>

  {#if settings.enabled}
    {#each icalUrls as _, i}
      <PasswordInput
        bind:password={icalUrls[i]}
        saveAction={saveUrls}
        label="iCal URL"
        placeholder="https://calendar.google.com/calendar/ical/…"
        inputId="ical-url-{i}"
      />
      {#if icalUrls.length > 1}
        <button
          type="button"
          class="border-2 px-3 py-1 text-sm -mt-6 mb-8"
          onclick={() => removeUrl(i)}
        >
          Remove
        </button>
      {/if}
    {/each}

    <p class="text-muted-foreground text-sm mb-4">
      Find this in <a
        href="https://calendar.google.com/calendar/r/settings"
        target="_blank"
        rel="noopener noreferrer"
        class="underline">Google Calendar Settings</a
      >.<br />
      Click on a calendar → scroll to <strong>"Secret address in iCal format"</strong>.<br />
      Keep it private — anyone with this URL can read your calendar.
    </p>

    <div class="flex items-center gap-4 flex-wrap">
      <button type="button" class="border-2 px-3 py-1 text-sm" onclick={addUrl}>
        Add another iCal
      </button>

      {#if syncStatus}
        <span class="text-xs text-muted-foreground">
          {#if syncStatus.syncing}
            <span class="animate-pulse">Syncing…</span>
          {:else if syncStatus.last_error}
            <span class="text-red-500">Sync error: {syncStatus.last_error}</span>
          {:else}
            Last synced: {formatSyncTime(syncStatus.last_synced_at)}
          {/if}
        </span>
      {/if}
    </div>
  {/if}
</fieldset>
