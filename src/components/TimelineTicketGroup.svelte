<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import GitCommit from "@lucide/svelte/icons/git-commit";
  import TicketCheck from "@lucide/svelte/icons/ticket-check";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import type { Component } from "svelte";
  import type { TaskGroupKeyType, TimelineEvent } from "$lib/timeline";
  import TimelineEventRow from "./TimelineEvent.svelte";
  import { t } from "$lib/i18n.svelte";

  let {
    label,
    keyType,
    events,
    doneIds,
    onToggle,
  }: {
    label: string;
    keyType: TaskGroupKeyType;
    events: TimelineEvent[];
    doneIds: Set<string>;
    onToggle: (id: string) => void;
  } = $props();

  let expanded = $state(false);

  let count = $derived(events.length);
  let doneCount = $derived(events.filter((e) => doneIds.has(e.id)).length);
  let allDone = $derived(doneCount === count);
  let first = $derived(events[0]);
  let last = $derived(events[events.length - 1]);
  // Wall-clock window the work item was touched in — a span, NOT time spent.
  let span = $derived(first.time === last.time ? first.time : `${first.time}–${last.time}`);

  // ticket = cross-source work item; repo/stream = secondary fallback keys.
  const config: Record<TaskGroupKeyType, { icon: Component; color: string }> = {
    ticket: { icon: TicketCheck, color: "bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300" },
    repo: { icon: GitCommit, color: "bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300" },
    stream: { icon: MessageSquare, color: "bg-emerald-100 text-emerald-700 dark:bg-emerald-900 dark:text-emerald-300" },
  };
  let Icon = $derived(config[keyType].icon);
</script>

<div class="space-y-1">
  <button
    type="button"
    onclick={() => (expanded = !expanded)}
    aria-expanded={expanded}
    aria-label={expanded ? t("timeline.group_collapse") : t("timeline.group_expand")}
    class="timeline-event-btn relative flex w-full min-w-0 max-w-full cursor-pointer items-start gap-3 border-2 bg-card py-2 pl-3 pr-2 text-left shadow-sm transition-all hover:shadow-none
      {allDone ? 'opacity-50' : ''}"
  >
    <span class="w-10 shrink-0 pt-0.5 font-mono text-xs text-muted-foreground">{span}</span>

    <div class="mt-0.5 shrink-0">
      <span class="inline-flex size-6 items-center justify-center border {config[keyType].color}">
        <Icon class="size-3.5" />
      </span>
    </div>

    <div class="timeline-event-body min-w-0 max-w-full flex-1 pr-10">
      <p class="timeline-clamp-1 text-sm font-medium leading-tight {allDone ? 'line-through' : ''}">
        {label}
      </p>
      {#if doneCount > 0 && !allDone}
        <p class="mt-0.5 text-[10px] uppercase tracking-wider text-muted-foreground">
          {t("timeline.commit_burst_partial_done", { done: doneCount.toString(), count: count.toString() })}
        </p>
      {/if}
    </div>

    <ChevronDown
      class="absolute right-7 top-2 size-3.5 text-muted-foreground transition-transform {expanded ? 'rotate-180' : ''}"
      aria-hidden="true"
    />

    <span
      class="absolute right-0 top-0 shrink-0 bg-foreground px-1 py-0.5 font-head text-[8px] uppercase tracking-widest text-background"
    >
      {keyType === "ticket" ? "" : keyType + " "}×{count}
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
