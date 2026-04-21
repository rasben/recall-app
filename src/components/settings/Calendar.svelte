<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { toast } from "svelte-sonner";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { commands, type SettingsIcal, type IcalSyncStatus } from "../../bindings";
  import PasswordInput from "../ui/PasswordInput.svelte";
  import { t } from "$lib/i18n.svelte";

  const defaultSettings: SettingsIcal = {
    enabled: false,
    urls: [],
    email: null,
  };

  let settings = $state<SettingsIcal>(defaultSettings);
  let icalUrls = $state<string[]>([""]);
  let emailInput = $state<string>("");
  let syncStatus = $state<IcalSyncStatus | null>(null);
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  onMount(async () => {
    settings = (await commands.getSettingsIcal()) ?? defaultSettings;
    const saved = settings.urls ?? [];
    icalUrls = saved.length > 0 ? [...saved] : [""];
    emailInput = settings.email ?? "";
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
      toast.error(t("settings.calendar.error_save"));
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
    if (ok) toast.success(t("settings.calendar.saved_url"));
  }

  async function saveEmail() {
    const trimmed = emailInput.trim();
    const ok = await persist({ email: trimmed.length > 0 ? trimmed : null });
    if (ok) toast.success(t("settings.calendar.saved_email"));
  }

  function addUrl() {
    icalUrls = [...icalUrls, ""];
  }

  function removeUrl(index: number) {
    icalUrls = icalUrls.filter((_, i) => i !== index);
    saveUrls();
  }

  function formatSyncTime(ts: number | null | undefined): string {
    if (!ts) return t("settings.calendar.never");
    return new Date(ts).toLocaleTimeString();
  }
</script>

<fieldset class="border-2 p-4 mt-6">
  <legend>{t("settings.calendar.legend")}</legend>

  <div class="flex items-center gap-2 mb-4">
    <Checkbox
      id="calendar-enabled"
      checked={settings.enabled}
      onCheckedChange={(v) => toggleEnabled(v === true)}
    />
    <Label for="calendar-enabled">{t("settings.calendar.enable")}</Label>
  </div>

  {#if settings.enabled}
    {#each icalUrls as _, i}
      <PasswordInput
        bind:password={icalUrls[i]}
        saveAction={saveUrls}
        label={t("settings.calendar.ical_url")}
        placeholder="https://calendar.google.com/calendar/ical/…"
        inputId="ical-url-{i}"
        description={icalUrls.length > 1 ? '' : t("settings.calendar.description")}
      />
      {#if icalUrls.length > 1}
        <Button variant="outline" class="-mt-6 mb-2" onclick={() => removeUrl(i)}>
          {t("settings.calendar.remove")}
        </Button>
      {/if}
    {/each}

    {#if icalUrls.length > 0}
    <div class="mt-4">
      <Label for="calendar-email" class="text-sm">{t("settings.calendar.email_label")}</Label>
      <p class="text-xs text-muted-foreground mb-1">{t("settings.calendar.email_hint")}</p>
      <div class="flex gap-2">
        <Input
          id="calendar-email"
          type="email"
          placeholder="you@example.com"
          bind:value={emailInput}
          onblur={saveEmail}
          onkeydown={(e) => { if (e.key === 'Enter') saveEmail(); }}
        />
      </div>
    </div>
    <div class="flex items-center gap-4 flex-wrap mt-4">
      <Button variant="outline" onclick={addUrl}>
        {t("settings.calendar.add_ical")}
      </Button>

      {#if syncStatus}
        <span class="text-xs text-muted-foreground">
          {#if syncStatus.syncing}
            <span class="animate-pulse">{t("settings.calendar.syncing")}</span>
          {:else if syncStatus.last_error}
            <span class="text-red-500">{t("settings.calendar.sync_error", { error: syncStatus.last_error })}</span>
          {:else}
            {t("settings.calendar.last_synced", { time: formatSyncTime(syncStatus.last_synced_at) })}
          {/if}
        </span>
      {/if}
    </div>
    {/if}
  {/if}
</fieldset>
