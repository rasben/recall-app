<script lang="ts">
  import GitCommit from "@lucide/svelte/icons/git-commit";
  import Github from "@lucide/svelte/icons/git-pull-request";
  import Calendar from "@lucide/svelte/icons/calendar";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import TicketCheck from "@lucide/svelte/icons/ticket-check";
  import Check from "@lucide/svelte/icons/check";
  import type { Component } from "svelte";
  import { SOURCE_LABELS, type TimelineEventSource } from "$lib/timeline";
  import { navState } from "$lib/nav-state.svelte";

  let { enabledSources }: { enabledSources: string[] } = $props();

  type Source = { key: TimelineEventSource; label: string; Icon: Component };
  const ALL_SOURCES: Source[] = [
    { key: "git", label: SOURCE_LABELS.git, Icon: GitCommit },
    { key: "github", label: SOURCE_LABELS.github, Icon: Github },
    { key: "calendar", label: SOURCE_LABELS.calendar, Icon: Calendar },
    { key: "jira", label: SOURCE_LABELS.jira, Icon: TicketCheck },
    { key: "zulip", label: SOURCE_LABELS.zulip, Icon: MessageSquare },
  ];

  let visibleSources = $derived(
    ALL_SOURCES.filter((s) => enabledSources.includes(s.label)),
  );

  function toggle(key: TimelineEventSource) {
    const next = new Set(navState.hiddenSources);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    navState.hiddenSources = next;
  }
</script>

{#if visibleSources.length > 1}
  <div class="mb-4 flex flex-wrap items-center gap-1.5">
    {#each visibleSources as { key, label, Icon } (key)}
      {@const hidden = navState.hiddenSources.has(key)}
      <button
        type="button"
        onclick={() => toggle(key)}
        aria-pressed={!hidden}
        class="inline-flex items-center gap-1.5 border-2 px-2 py-1 font-head text-[10px] uppercase tracking-widest transition-colors
          {hidden
            ? 'border-dashed border-border bg-transparent text-muted-foreground hover:text-foreground hover:border-foreground'
            : 'border-foreground bg-foreground text-background'}"
      >
        <span
          class="inline-flex size-3.5 items-center justify-center border
            {hidden ? 'border-border bg-transparent' : 'border-background bg-background text-foreground'}"
          aria-hidden="true"
        >
          {#if !hidden}
            <Check class="size-2.5 stroke-[3]" />
          {/if}
        </span>
        <Icon class="size-3" />
        <span>{label}</span>
      </button>
    {/each}
  </div>
{/if}
