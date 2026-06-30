<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import GitCommit from "@lucide/svelte/icons/git-commit";
  import TicketCheck from "@lucide/svelte/icons/ticket-check";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import { openUrl } from "@tauri-apps/plugin-opener";
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
    jiraBaseUrl = null,
  }: {
    label: string;
    keyType: TaskGroupKeyType;
    events: TimelineEvent[];
    doneIds: Set<string>;
    onToggle: (id: string) => void;
    jiraBaseUrl?: string | null;
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

  // Link to the ticket: prefer a JIRA event's own browse URL in the group, else
  // build it from the configured site so even git-only ticket groups link out.
  let ticketUrl = $derived.by(() => {
    if (keyType !== "ticket") return null;
    const fromEvent = events.find((e) => e.source === "jira" && e.url)?.url;
    if (fromEvent) return fromEvent;
    return jiraBaseUrl ? `${jiraBaseUrl}/browse/${label}` : null;
  });

  function toggle() {
    expanded = !expanded;
  }
</script>

<div class="space-y-1">
  <div
    role="button"
    tabindex="0"
    onclick={toggle}
    onkeydown={(e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        toggle();
      }
    }}
    aria-expanded={expanded}
    aria-label={expanded ? t("timeline.group_collapse") : t("timeline.group_expand")}
    class="timeline-event-btn relative flex w-full min-w-0 max-w-full cursor-pointer items-center gap-3 border-2 border-dashed bg-card py-2 pl-3 pr-2 text-left shadow-sm transition-all hover:border-foreground hover:shadow-none
      {allDone ? 'opacity-50' : ''}"
  >
    <span class="w-10 shrink-0 font-mono text-xs text-muted-foreground">{span}</span>

    <span class="inline-flex size-6 shrink-0 items-center justify-center border {config[keyType].color}">
      <Icon class="size-3.5" />
    </span>

    <div class="min-w-0 flex-1">
      <p class="timeline-clamp-1 text-sm font-medium leading-tight {allDone ? 'line-through' : ''}">{label}</p>
      <p class="mt-0.5 text-[10px] uppercase tracking-wider text-muted-foreground">
        {#if doneCount > 0 && !allDone}
          {t("timeline.commit_burst_partial_done", { done: doneCount.toString(), count: count.toString() })}
        {:else}
          {t("timeline.group_items", { count: count.toString() })}
        {/if}
      </p>
    </div>

    {#if ticketUrl}
      <button
        type="button"
        class="inline-flex shrink-0 items-center gap-1 border-2 border-border px-2 py-1 font-head text-[9px] uppercase tracking-widest text-muted-foreground transition-colors hover:border-foreground hover:text-foreground"
        onclick={(e) => {
          e.stopPropagation();
          if (ticketUrl) openUrl(ticketUrl);
        }}
        aria-label={t("timeline.open_in_jira")}
      >
        Jira
        <ExternalLink class="size-3" />
      </button>
    {/if}

    <span
      class="inline-flex shrink-0 items-center gap-1 border-2 border-border px-2 py-1 font-head text-[9px] uppercase tracking-widest text-muted-foreground"
      aria-hidden="true"
    >
      {expanded ? t("timeline.group_collapse") : t("timeline.group_expand")}
      <ChevronDown class="size-3 transition-transform {expanded ? 'rotate-180' : ''}" />
    </span>
  </div>

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
