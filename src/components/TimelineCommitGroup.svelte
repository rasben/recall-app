<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import GitCommit from "@lucide/svelte/icons/git-commit";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import type { TimelineEvent } from "$lib/timeline";
  import TimelineEventRow from "./TimelineEvent.svelte";

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
</script>

<div class="space-y-1">
  <button
    type="button"
    onclick={() => (expanded = !expanded)}
    aria-expanded={expanded}
    class="timeline-event-btn relative flex w-full min-w-0 max-w-full cursor-pointer items-start gap-3 border-2 bg-card py-2 pl-3 pr-2 text-left shadow-sm transition-all hover:shadow-none
      {allDone ? 'opacity-50' : ''}"
  >
    <span class="w-10 shrink-0 pt-0.5 font-mono text-xs text-muted-foreground">{latest.time}</span>

    <div class="mt-0.5 shrink-0">
      <span class="inline-flex size-6 items-center justify-center border bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300">
        {#if expanded}
          <ChevronDown class="size-3.5" />
        {:else}
          <GitCommit class="size-3.5" />
        {/if}
      </span>
    </div>

    <div class="timeline-event-body min-w-0 max-w-full flex-1 pr-10">
      <p class="timeline-clamp-1 text-sm font-medium leading-tight {allDone ? 'line-through' : ''}">
        {latest.title}
      </p>
      <p class="timeline-clamp-1 mt-0.5 min-w-0 max-w-full text-xs text-muted-foreground">
        <ChevronRight class="inline size-3 -mt-0.5 transition-transform {expanded ? 'rotate-90' : ''}" />
        {count} commits
        {#if doneCount > 0 && !allDone}
          <span class="ml-1 text-[10px] uppercase tracking-wider">· {doneCount}/{count} logged</span>
        {/if}
      </p>
    </div>

    <span
      class="absolute right-0 top-0 shrink-0 bg-foreground px-1 py-0.5 font-head text-[8px] uppercase tracking-widest text-background"
    >
      Git ×{count}
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
