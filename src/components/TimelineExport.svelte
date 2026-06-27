<script lang="ts">
  import { untrack } from "svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as Popover from "$lib/components/ui/popover/index.js";
  import ClipboardCopy from "@lucide/svelte/icons/clipboard-copy";
  import Loader from "@lucide/svelte/icons/loader-circle";
  import { toast } from "svelte-sonner";
  import { commands } from "../bindings";
  import { navState } from "$lib/nav-state.svelte";
  import { addDaysIso, todayIso } from "$lib/timeline";
  import { toJson, toMarkdown } from "$lib/export";
  import { resolveExportPrompt } from "$lib/export-prompt";
  import { t, langLocale } from "$lib/i18n.svelte";

  let { selectedDate }: { selectedDate: string } = $props();

  type Format = "markdown" | "json";

  let open = $state(false);
  // Seed with the current day; the open-effect below re-syncs on each open.
  let start = $state(untrack(() => selectedDate));
  let end = $state(untrack(() => selectedDate));
  let format = $state<Format>("markdown");
  let includePrompt = $state(true);
  let copying = $state(false);

  const maxDate = todayIso();
  // Empty inputs (a cleared native date field) are invalid too, so we never
  // send a blank date to the backend.
  let invalidRange = $derived(!start || !end || start > end);

  const PRESETS = [
    { days: 1, label: "export.preset_day" },
    { days: 7, label: "export.preset_week" },
    { days: 30, label: "export.preset_month" },
  ] as const;

  /** Start date a given preset would set, relative to the selected day. */
  function presetStart(days: number): string {
    return days <= 1 ? selectedDate : addDaysIso(selectedDate, -(days - 1));
  }

  // Which preset the current range matches, if any — drives the active button
  // highlight. A manually-edited range matches no preset (null).
  let activePreset = $derived(
    end === selectedDate
      ? (PRESETS.find((p) => start === presetStart(p.days))?.days ?? null)
      : null,
  );

  // Reset the range to the currently-viewed day each time the popover opens, so
  // it always reflects what the user is looking at rather than a stale choice.
  $effect(() => {
    if (open) {
      start = selectedDate;
      end = selectedDate;
    }
  });

  function preset(days: number) {
    end = selectedDate;
    start = presetStart(days);
  }

  async function copy() {
    if (copying || invalidRange) return;
    copying = true;
    try {
      const result = await commands.exportTimelineForRange(start, end);
      if (result.status !== "ok") {
        toast.error(result.error, { richColors: true });
        return;
      }
      const { days, errors } = result.data;
      const eventCount = days.reduce((n, d) => n + d.events.length, 0);
      if (eventCount === 0) {
        toast.error(t("export.empty"), { richColors: true });
        return;
      }
      // Resolve the prompt at copy time so a custom prompt saved in Settings is
      // picked up; an empty/default stored value follows the active language.
      let promptText: string | null = null;
      if (format === "markdown" && includePrompt) {
        const settings = await commands.getSettingsExport();
        promptText = resolveExportPrompt(settings?.prompt ?? "");
      }
      const text =
        format === "markdown"
          ? toMarkdown(days, start, end, langLocale(), promptText, {
              title: t("export.md_title"),
              intro: t("export.md_intro"),
              noActivity: t("export.md_no_activity"),
            })
          : toJson(days, start, end);
      await navigator.clipboard.writeText(text);
      // A source that errored still yields a copyable export from the others —
      // warn so the result isn't mistaken for a complete picture.
      if (errors.length > 0) {
        toast.error(
          t("export.partial", { count: String(eventCount), sources: errors.map((e) => e.source).join(", ") }),
          { richColors: true },
        );
      } else {
        toast.success(t("export.copied", { count: String(eventCount) }));
      }
      open = false;
    } catch (e) {
      toast.error(e instanceof Error ? e.message : t("export.error"), { richColors: true });
    } finally {
      copying = false;
    }
  }

  function editPrompt() {
    open = false;
    navState.openSettingsSection = "export";
  }
</script>

<Popover.Root bind:open>
  <Popover.Trigger
    class={`inline-flex h-7 items-center gap-1.5 border-2 px-2 text-sm transition outline-hidden hover:border-border hover:bg-accent hover:text-accent-foreground focus-visible:border-border ${open ? "border-border bg-accent text-accent-foreground" : "border-transparent text-muted-foreground"}`}
    title={t("export.title")}
  >
    <ClipboardCopy class="size-3.5" />
    <span>{t("export.button")}</span>
  </Popover.Trigger>
  <Popover.Content align="end" sideOffset={8} class="w-72 space-y-4">
    <p class="font-head text-sm">{t("export.title")}</p>

    <div class="flex flex-wrap gap-1.5">
      {#each PRESETS as p (p.days)}
        <Button
          variant={activePreset === p.days ? "default" : "outline"}
          size="sm"
          class="text-xs"
          onclick={() => preset(p.days)}
        >
          {t(p.label)}
        </Button>
      {/each}
    </div>

    <div class="grid grid-cols-2 gap-2">
      <div class="space-y-1">
        <Label for="export-start" class="text-xs text-muted-foreground">{t("export.start")}</Label>
        <Input id="export-start" type="date" bind:value={start} max={maxDate} class="text-xs" />
      </div>
      <div class="space-y-1">
        <Label for="export-end" class="text-xs text-muted-foreground">{t("export.end")}</Label>
        <Input id="export-end" type="date" bind:value={end} max={maxDate} class="text-xs" />
      </div>
    </div>

    {#if invalidRange}
      <p class="text-xs text-destructive">{t("export.invalid_range")}</p>
    {/if}

    <div class="space-y-1">
      <span class="text-xs text-muted-foreground">{t("export.format")}</span>
      <div class="flex gap-1.5">
        <Button
          variant={format === "markdown" ? "default" : "outline"}
          size="sm"
          class="flex-1 text-xs"
          onclick={() => (format = "markdown")}
        >
          {t("export.format_markdown")}
        </Button>
        <Button
          variant={format === "json" ? "default" : "outline"}
          size="sm"
          class="flex-1 text-xs"
          onclick={() => (format = "json")}
        >
          {t("export.format_json")}
        </Button>
      </div>
    </div>

    {#if format === "markdown"}
      <div class="space-y-1.5">
        <div class="flex items-center gap-2">
          <Checkbox
            id="export-include-prompt"
            size="sm"
            checked={includePrompt}
            onCheckedChange={(v) => (includePrompt = v === true)}
          />
          <Label for="export-include-prompt" class="text-xs">{t("export.include_prompt")}</Label>
        </div>
        <p class="text-xs text-muted-foreground">{t("export.prompt_hint")}</p>
        <button
          type="button"
          class="text-xs text-muted-foreground underline underline-offset-2 transition hover:text-foreground"
          onclick={editPrompt}
        >
          {t("export.edit_prompt")}
        </button>
      </div>
    {/if}

    <Button class="w-full text-xs" disabled={copying || invalidRange} onclick={copy}>
      {#if copying}
        <Loader class="size-3.5 animate-spin" />
        {t("export.copying")}
      {:else}
        <ClipboardCopy class="size-3.5" />
        {t("export.copy")}
      {/if}
    </Button>
  </Popover.Content>
</Popover.Root>
