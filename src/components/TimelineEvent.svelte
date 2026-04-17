<script lang="ts">
  import GitCommit from "@lucide/svelte/icons/git-commit";
  import Github from "@lucide/svelte/icons/github";
  import Calendar from "@lucide/svelte/icons/calendar";
  import Mail from "@lucide/svelte/icons/mail";
  import FileText from "@lucide/svelte/icons/file-text";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import TicketCheck from "@lucide/svelte/icons/ticket-check";
  import type { RecallEvent } from "$lib/timeline";
  import type { Component } from "svelte";

  let { event, done = false, onToggle }: { event: RecallEvent; done?: boolean; onToggle?: () => void } = $props();

  const sourceConfig: Record<string, { icon: Component; label: string; color: string }> = {
    git:      { icon: GitCommit,     label: "Git",      color: "bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300" },
    github:   { icon: Github,        label: "GitHub",   color: "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300" },
    calendar: { icon: Calendar,      label: "Calendar", color: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300" },
    gmail:    { icon: Mail,          label: "Gmail",    color: "bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300" },
    drive:    { icon: FileText,      label: "Drive",    color: "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300" },
    jira:     { icon: TicketCheck,   label: "JIRA",     color: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300" },
    zulip:    { icon: MessageSquare, label: "Zulip",    color: "bg-emerald-100 text-emerald-700 dark:bg-emerald-900 dark:text-emerald-300" },
  };

  let config = $derived(sourceConfig[event.source] ?? sourceConfig.git);
</script>

<button
  type="button"
  onclick={onToggle}
  class="w-full flex relative items-start gap-3 border-2 bg-card pl-3 pr-2 py-2 shadow-sm hover:shadow-none transition-all cursor-pointer text-left
    {done ? 'opacity-50' : ''}"
>

  <span class="text-xs font-mono text-muted-foreground pt-0.5 w-10 shrink-0">{event.time}</span>

  {#if config}
    {@const Icon = config.icon}
    <div class="shrink-0 mt-0.5">
      <span class="inline-flex items-center justify-center size-6 border {config.color}">
        <Icon class="size-3.5" />
      </span>
    </div>
  {/if}

  <div class="flex-1 min-w-0">
    <p class="font-medium text-sm leading-tight truncate {done ? 'line-through' : ''}">{event.title}</p>
    {#if event.detail}
      <p class="text-xs text-muted-foreground mt-0.5 truncate">{event.detail}</p>
    {/if}
  </div>

  <div class="relative shrink-0 self-end mt-0.5">
    <img
      src="/harvest.svg"
      alt={done ? "Logged in Harvest" : "Not logged in Harvest"}
      class="block size-3 transition-all {done ? '' : 'grayscale opacity-25'}"
    />
    {#if done}
      <span
        class="pointer-events-none absolute left-1 top-1 flex size-3 items-center justify-center text-[9px] leading-none"
        aria-hidden="true"
      >✔︎</span>
    {/if}
  </div>
  <span class="absolute bg-foreground text-background top-0 right-0 py-0.5 px-1 text-[10px] font-head uppercase tracking-wide shrink-0">
      {config.label}
  </span>


</button>
