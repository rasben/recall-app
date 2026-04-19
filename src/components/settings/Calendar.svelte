<script lang="ts">
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { commands, type SettingsIcal, type IcalDebugInfo } from "../../bindings";
  import PasswordInput from "../ui/PasswordInput.svelte";

  const defaultSettings: SettingsIcal = {
    enabled: false,
    urls: [],
  };

  let settings = $state<SettingsIcal>(defaultSettings);
  let icalUrls = $state<string[]>([""]);

  onMount(async () => {
    settings = (await commands.getSettingsIcal()) ?? defaultSettings;
    const saved = settings.urls ?? [];
    icalUrls = saved.length > 0 ? [...saved] : [""];
  });

  async function persist(partial: Partial<SettingsIcal>) {
    const next: SettingsIcal = { ...settings, ...partial };
    const result = await commands.setSettingsIcal(next);
    if (result.status === "error") {
      toast.error("Could not save Calendar settings");
      return false;
    }
    settings = next;
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

  let debugResults = $state<IcalDebugInfo[] | null>(null);
  let debugging = $state(false);

  async function runDebug() {
    debugging = true;
    debugResults = null;
    try {
      debugResults = await commands.debugIcal();
    } finally {
      debugging = false;
    }
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

    <div class="flex gap-2 flex-wrap">
      <button type="button" class="border-2 px-3 py-1 text-sm" onclick={addUrl}>
        Add another iCal
      </button>
      <button
        type="button"
        class="border-2 px-3 py-1 text-sm disabled:opacity-50"
        disabled={debugging}
        onclick={runDebug}
      >
        {debugging ? "Testing…" : "Test"}
      </button>
    </div>

    {#if debugResults}
      <div class="mt-4 border-2 p-3 text-xs font-mono space-y-3">
        {#each debugResults as r}
          <div>
            <p class="font-bold break-all">{r.url}</p>
            {#if r.error}
              <p class="text-red-500">{r.error}</p>
            {:else}
              <p>
                Total: {r.total_events} events —
                All-day skipped: {r.all_day_skipped} —
                Recurring (RRULE): {r.recurring_rrule}
              </p>
              <p class="mt-1 font-semibold">Today's events ({r.today_events.length}):</p>
              {#if r.today_events.length === 0}
                <p class="text-muted-foreground">None found for today.</p>
              {:else}
                {#each r.today_events as line}
                  <p class="text-muted-foreground">{line}</p>
                {/each}
              {/if}
              {#if r.dtstart_samples.length > 0}
                <p class="mt-1 font-semibold">Raw DTSTART samples:</p>
                {#each r.dtstart_samples as s}
                  <p class="text-muted-foreground">{s}</p>
                {/each}
              {/if}
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</fieldset>
