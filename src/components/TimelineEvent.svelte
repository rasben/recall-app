<script lang="ts">
  import GitCommit from "@lucide/svelte/icons/git-commit";
  import Github from "@lucide/svelte/icons/git-pull-request";
  import Calendar from "@lucide/svelte/icons/calendar";
  import Mail from "@lucide/svelte/icons/mail";
  import FileText from "@lucide/svelte/icons/file-text";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import TicketCheck from "@lucide/svelte/icons/ticket-check";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { TimelineEvent, TimelineEventSource } from "../bindings";
  import type { Component } from "svelte";
  import { SOURCE_LABELS } from "$lib/timeline";
  import { track } from "$lib/telemetry";
  import { t } from "$lib/i18n.svelte";

  let { event, done = false, onToggle }: { event: TimelineEvent; done?: boolean; onToggle?: () => void } = $props();

  // Label comes from the shared SOURCE_LABELS map; the badge CSS upper-cases it.
  const sourceConfig: Record<TimelineEventSource, { icon: Component; color: string }> = {
    git: { icon: GitCommit, color: "bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300" },
    github: { icon: Github, color: "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300" },
    calendar: { icon: Calendar, color: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300" },
    gmail: { icon: Mail, color: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300" },
    drive: { icon: FileText, color: "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300" },
    jira: { icon: TicketCheck, color: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300" },
    zulip: { icon: MessageSquare, color: "bg-emerald-100 text-emerald-700 dark:bg-emerald-900 dark:text-emerald-300" },
  };

  let config = $derived(sourceConfig[event.source]);
</script>

<!--
  The row itself is not interactive. Two real buttons live inside it:
  - an invisible "open link" button stretched over the whole row (only when the
    event has a URL), so the primary click is "read more", never "log";
  - the Harvest mark, layered above the stretched button, which is the only
    control that toggles the done state.
  This avoids nesting buttons inside a role="button" row.
-->
<div
  class="timeline-event-btn relative flex w-full min-w-0 max-w-full items-start gap-3 border-2 bg-card py-2 pl-3 pr-2 text-left shadow-sm transition-all hover:translate-x-0.5 hover:translate-y-0.5 hover:shadow-none
    {done ? 'opacity-50' : ''}"
>
  {#if event.url}
    <button
      type="button"
      class="timeline-open-btn absolute inset-0 z-0 cursor-pointer outline-hidden focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-foreground"
      onclick={() => { track(`link.open.${event.source}`); openUrl(event.url!); }}
      aria-label={t("timeline.open_link_for", { title: event.title })}
    ></button>
  {/if}

  <span class="w-10 shrink-0 pt-0.5 font-mono text-xs text-muted-foreground">{event.time}</span>
  {#if event.url}
    <span
      class="timeline-link-hint absolute bottom-1.5 left-3 inline-flex w-10 items-center justify-center gap-1 text-[10px] text-muted-foreground opacity-0 transition-opacity"
      aria-hidden="true"
    >
      <span>{t("timeline.open_link")}</span>
      <ExternalLink class="size-3" />
    </span>
  {/if}
  {#if config}
    {@const Icon = config.icon}
    <div class="mt-0.5 shrink-0">
      <span class="inline-flex size-6 items-center justify-center border {config.color}">
        <Icon class="size-3.5" />
      </span>
    </div>
  {/if}

  <div class="timeline-event-body min-w-0 max-w-full flex-1 pr-10">
    <p class="timeline-clamp-1 text-sm font-medium leading-tight {done ? 'line-through' : ''}">{event.title}</p>
    {#if event.detail}
      {#if event.source === "calendar"}
        <!-- Calendar detail is exclusively the event's duration (e.g. "1h 30m") —
             the only true time signal we have, so give it weight as a mono chip. -->
        <p class="mt-1">
          <span class="inline-flex items-center border bg-muted px-1.5 py-0.5 font-mono text-xs font-semibold tabular-nums text-foreground">
            {event.detail}
          </span>
        </p>
      {:else}
        <p class="timeline-clamp-1 mt-0.5 min-w-0 max-w-full text-xs text-muted-foreground">
          {event.detail}
        </p>
      {/if}
    {/if}
  </div>

  <button
    type="button"
    onclick={onToggle}
    aria-pressed={done}
    aria-label={done ? t("timeline.mark_not_logged") : t("timeline.mark_logged")}
    title={done ? t("timeline.mark_not_logged") : t("timeline.mark_logged")}
    class="timeline-harvest-btn relative z-10 -mb-1 -mr-0.5 flex size-7 shrink-0 cursor-pointer items-center justify-center self-end border-2 border-transparent transition-colors hover:border-foreground hover:bg-accent focus-visible:border-foreground focus-visible:outline-hidden"
  >
    <span class="relative block">
      <img
        src="/harvest.svg"
        alt=""
        class="block size-3 transition-all {done ? '' : 'opacity-25 grayscale'}"
      />
      {#if done}
        <span
          class="pointer-events-none absolute left-1 top-1 flex size-3 items-center justify-center text-[9px] leading-none"
          aria-hidden="true"
        >✔︎</span>
      {/if}
    </span>
  </button>
  <span
    class="absolute right-0 top-0 shrink-0 bg-foreground px-1 py-0.5 font-head text-[8px] uppercase tracking-widest text-background"
  >
    {SOURCE_LABELS[event.source]}
  </span>
</div>

<style>
  /* One-line ellipsis; unwrap on hover/focus-within. Title used to use Tailwind truncate (no hover reset). */
  .timeline-event-btn .timeline-clamp-1 {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    overflow-wrap: anywhere;
  }
  .timeline-event-btn:is(:hover, :focus-within) .timeline-clamp-1 {
    white-space: normal;
    overflow: visible;
    text-overflow: clip;
  }
  .timeline-event-btn:is(:hover, :focus-within) .timeline-link-hint {
    opacity: 1;
  }
  /* The Harvest mark is the one control that logs; make it obvious on hover. */
  .timeline-harvest-btn:is(:hover, :focus-visible) img {
    opacity: 1;
    filter: none;
  }
</style>
