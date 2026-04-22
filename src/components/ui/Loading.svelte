<script lang="ts">
  import GitCommit from "@lucide/svelte/icons/git-commit";
  import Github from "@lucide/svelte/icons/git-pull-request";
  import Calendar from "@lucide/svelte/icons/calendar";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import TicketCheck from "@lucide/svelte/icons/ticket-check";
  import Check from "@lucide/svelte/icons/check";
  import type { Component } from "svelte";
  import { t } from "$lib/i18n.svelte";

  let {
    currentSource = null,
    doneSources = new Set(),
    enabledSources = [],
  }: {
    currentSource?: string | null;
    doneSources?: Set<string>;
    enabledSources?: string[];
  } = $props();

  const sourceList: { name: string; Icon: Component }[] = [
    { name: "Git", Icon: GitCommit },
    { name: "GitHub", Icon: Github },
    { name: "Calendar", Icon: Calendar },
    { name: "Jira", Icon: TicketCheck },
    { name: "Zulip", Icon: MessageSquare },
  ];

  let filteredSources = $derived(
    enabledSources.length > 0
      ? sourceList.filter((s) => enabledSources.includes(s.name))
      : sourceList
  );
  let total = $derived(filteredSources.length || 1);
  let doneCount = $derived(doneSources.size);
  let progress = $derived((doneCount / total) * 100);
</script>

<div class="w-full space-y-3 pt-1">
  <!-- Progress bar -->
  <div class="flex items-center gap-3">
    <span class="font-head text-[10px] uppercase tracking-[0.3em] text-muted-foreground shrink-0">
      {t("timeline.loading")}
    </span>
    <div class="flex-1 h-2 bg-border overflow-hidden">
      <div
        class="h-full bg-primary transition-[width] duration-300 ease-out"
        style="width: {progress}%"
      ></div>
    </div>
  </div>

  <!-- Source cards -->
  {#if filteredSources.length > 0}
    <div class="grid" style="grid-template-columns: repeat({filteredSources.length}, 1fr)">
      {#each filteredSources as { name, Icon }, i}
        {@const isDone = doneSources.has(name)}
        {@const isActive = currentSource === name}
        <div
          class="border-2 px-2 py-4 flex flex-col items-center gap-2 transition-colors duration-300
            {i > 0 ? '-ml-[2px]' : ''}
            {isDone
              ? 'bg-primary border-foreground text-primary-foreground'
              : isActive
                ? 'bg-secondary text-secondary-foreground border-foreground animate-pulse'
                : 'border-border opacity-35'}"
        >
          {#if isDone}
            <Check class="size-5" />
          {:else}
            <Icon class="size-5" />
          {/if}
          <span class="font-head text-[9px] uppercase tracking-wider leading-none">{name}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
