<script lang="ts">
  import GitCommit from "@lucide/svelte/icons/git-commit";
  import Github from "@lucide/svelte/icons/github";
  import Calendar from "@lucide/svelte/icons/calendar";
  import Mail from "@lucide/svelte/icons/mail";
  import FileText from "@lucide/svelte/icons/file-text";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import TicketCheck from "@lucide/svelte/icons/ticket-check";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { TimelineEvent, TimelineEventSource } from "../bindings";
  import type { Component } from "svelte";
  import { t } from "$lib/i18n.svelte";

  let { event, done = false, onToggle }: { event: TimelineEvent; done?: boolean; onToggle?: () => void } = $props();

  const sourceConfig: Record<
    TimelineEventSource,
    { icon: Component; label: string; color: string }
  > = {
    git: { icon: GitCommit, label: "Git", color: "bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300" },
    github: { icon: Github, label: "GitHub", color: "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300" },
    calendar: { icon: Calendar, label: "Calendar", color: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300" },
    gmail: { icon: Mail, label: "Gmail", color: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300" },
    drive: { icon: FileText, label: "Drive", color: "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300" },
    jira: { icon: TicketCheck, label: "JIRA", color: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300" },
    zulip: { icon: MessageSquare, label: "Zulip", color: "bg-emerald-100 text-emerald-700 dark:bg-emerald-900 dark:text-emerald-300" },
  };

  let config = $derived(sourceConfig[event.source]);
</script>

<div
  role="button"
  tabindex="0"
  onclick={onToggle}
  onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onToggle?.(); } }}
  class="timeline-event-btn relative flex w-full min-w-0 max-w-full cursor-pointer items-start gap-3 border-2 bg-card py-2 pl-3 pr-2 text-left shadow-sm transition-all hover:shadow-none
    {done ? 'opacity-50' : ''}"
>
  <span class="w-10 shrink-0 pt-0.5 font-mono text-xs text-muted-foreground">{event.time}</span>
  {#if event.url}
    <button
            type="button"
            class="timeline-link-btn absolute bottom-1.5 left-3 w-10 text-center inline-flex items-center gap-1 text-[10px] text-muted-foreground opacity-0 transition-opacity hover:text-foreground"
            onclick={(e) => { e.stopPropagation(); openUrl(event.url!); }}
            aria-label={t("timeline.open_link")}
    >
      <span>{t("timeline.open_link")}</span>
      <ExternalLink class="size-3" />
    </button>
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
      <p class="timeline-clamp-1 mt-0.5 min-w-0 max-w-full text-xs text-muted-foreground">
        {event.detail}
      </p>
    {/if}
  </div>

  <div class="relative mt-0.5 shrink-0 self-end">
    <img
      src="/harvest.svg"
      alt={done ? t("timeline.logged_in_harvest") : t("timeline.not_logged_in_harvest")}
      class="block size-3 transition-all {done ? '' : 'opacity-25 grayscale'}"
    />
    {#if done}
      <span
        class="pointer-events-none absolute left-1 top-1 flex size-3 items-center justify-center text-[9px] leading-none"
        aria-hidden="true"
      >✔︎</span>
    {/if}
  </div>
  <span
    class="absolute right-0 top-0 shrink-0 bg-foreground px-1 py-0.5 font-head text-[8px] uppercase tracking-widest text-background"
  >
    {config.label}
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
  .timeline-event-btn:is(:hover, :focus-within) .timeline-event-body {
    position: relative;
    z-index: 1;
  }
  .timeline-event-btn:is(:hover, :focus-within) .timeline-link-btn {
    opacity: 1;
  }
</style>
