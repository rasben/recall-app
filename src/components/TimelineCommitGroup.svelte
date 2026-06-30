<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import GitCommit from "@lucide/svelte/icons/git-commit";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import { isDependabotCommit, type TimelineEvent } from "$lib/timeline";
  import TimelineEventRow from "./TimelineEvent.svelte";
  import { t } from "$lib/i18n.svelte";

  let {
    events,
    doneIds,
    onToggle,
  }: {
    events: TimelineEvent[];
    doneIds: Set<string>;
    onToggle: (id: string) => void;
  } = $props();

  let expanded = $state(false);

  let latest = $derived(events[events.length - 1]);
  let count = $derived(events.length);
  let doneCount = $derived(events.filter((e) => doneIds.has(e.id)).length);
  let allDone = $derived(doneCount === count);
  let countStr = $derived(count.toString());
  let isDependabot = $derived(events.every(isDependabotCommit));
  // git detail is "{repo} — {short}"; all commits in a burst share one repo.
  let repoName = $derived(events[0].detail?.split(" — ")[0] ?? null);
</script>

<div class="space-y-1">
  <button
    type="button"
    onclick={() => (expanded = !expanded)}
    aria-expanded={expanded}
    aria-label={expanded ? t("timeline.commit_burst_collapse") : t("timeline.commit_burst_expand")}
    class="timeline-event-btn relative flex w-full min-w-0 max-w-full cursor-pointer items-center gap-3 border-2 border-dashed bg-card py-2 pl-3 pr-2 text-left shadow-sm transition-all hover:border-foreground hover:shadow-none
      {allDone ? 'opacity-50' : ''}"
  >
    <span class="w-10 shrink-0 font-mono text-xs text-muted-foreground">{latest.time}</span>

    <span class="inline-flex size-6 shrink-0 items-center justify-center border bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300">
      <GitCommit class="size-3.5" />
    </span>

    <div class="timeline-event-body min-w-0 max-w-full flex-1">
      <p class="timeline-clamp-1 text-sm font-medium italic leading-tight text-muted-foreground {allDone ? 'line-through' : ''}">
        {isDependabot ? t("timeline.dependabot_burst", { count: countStr }) : t("timeline.commit_burst", { count: countStr })}
        {#if repoName}<span class="not-italic font-normal text-foreground">· {repoName}</span>{/if}
      </p>
      {#if doneCount > 0 && !allDone}
        <p class="mt-0.5 text-[10px] uppercase tracking-wider text-muted-foreground">
          {t("timeline.commit_burst_partial_done", { done: doneCount.toString(), count: countStr })}
        </p>
      {/if}
    </div>

    <span
      class="inline-flex shrink-0 items-center gap-1 border-2 border-border px-2 py-1 font-head text-[9px] uppercase tracking-widest text-muted-foreground"
      aria-hidden="true"
    >
      {expanded ? t("timeline.commit_burst_collapse") : t("timeline.commit_burst_expand")}
      <ChevronDown class="size-3 transition-transform {expanded ? 'rotate-180' : ''}" />
    </span>
  </button>

  {#if expanded}
    <div class="ml-6 space-y-1 border-l-2 border-border pl-3" transition:slide={{ duration: 180, easing: cubicOut }}>
      {#each events as event (event.id)}
        <TimelineEventRow {event} done={doneIds.has(event.id)} onToggle={() => onToggle(event.id)} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .timeline-event-btn :global(.timeline-clamp-1) {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    overflow-wrap: anywhere;
  }
  .timeline-event-btn:is(:hover, :focus-within) :global(.timeline-clamp-1) {
    white-space: normal;
    overflow: visible;
    text-overflow: clip;
  }
</style>
