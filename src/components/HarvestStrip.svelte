<script lang="ts">
  import { fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import type { HarvestTimeEntry } from "../bindings";
  import { formatHours, totalHours } from "$lib/harvest";
  import { t } from "$lib/i18n.svelte";

  let {
    entries = [],
    error = null,
    loading = false,
  }: {
    entries?: HarvestTimeEntry[];
    error?: string | null;
    loading?: boolean;
  } = $props();

  let total = $derived(totalHours(entries));
  let dayUrl = $derived(entries.find((e) => e.url)?.url ?? null);
</script>

<section
  class="border-2 bg-card shadow-sm transition-opacity {loading
    ? 'opacity-60'
    : ''}"
  aria-label={t("timeline.harvest.title")}
  aria-busy={loading}
>
  <header
    class="flex items-center justify-between gap-3 border-b-2 px-3 py-1.5"
  >
    <div class="flex items-center gap-2 min-w-0">
      <span
        class="font-head text-[10px] uppercase tracking-widest text-muted-foreground"
      >
        {t("timeline.harvest.title")}
      </span>
      {#if dayUrl}
        <button
          type="button"
          class="inline-flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground"
          title={t("timeline.harvest.open")}
          aria-label={t("timeline.harvest.open")}
          onclick={() => openUrl(dayUrl!)}
        >
          <ExternalLink class="size-3" />
        </button>
      {/if}
    </div>
    <span
      class="font-head text-sm tabular-nums {total > 0
        ? ''
        : 'text-muted-foreground'}"
    >
      {formatHours(total)}
    </span>
  </header>

  {#if error}
    <p
      class="px-3 py-2 text-xs text-destructive"
      transition:fade={{ duration: 200, easing: cubicOut }}
    >
      {t("timeline.harvest.error", { error })}
    </p>
  {:else if entries.length === 0}
    <p class="px-3 py-2 text-xs text-muted-foreground">
      {loading ? t("timeline.loading") + "…" : t("timeline.harvest.empty")}
    </p>
  {:else}
    <ul class="divide-y">
      {#each entries as entry (entry.id)}
        <li class="flex items-baseline gap-3 px-3 py-1.5 text-xs">
          <span class="w-10 shrink-0 text-right font-mono tabular-nums">
            {formatHours(entry.hours)}
          </span>
          <span class="min-w-0 flex-1 flex flex-wrap items-baseline gap-x-2">
            <span class="font-medium">{entry.project}</span>
            {#if entry.task}
              <span class="text-muted-foreground">{entry.task}</span>
            {/if}
            {#if entry.notes}
              <span
                class="min-w-0 flex-1 truncate text-muted-foreground"
                title={entry.notes}
              >
                {entry.notes}
              </span>
            {/if}
          </span>
          {#if entry.is_running}
            <span
              class="shrink-0 bg-foreground px-1 py-0.5 font-head text-[8px] uppercase tracking-widest text-background"
            >
              {t("timeline.harvest.running")}
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>
