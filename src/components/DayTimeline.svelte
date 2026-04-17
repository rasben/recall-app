<script lang="ts">
  import { MOCK_TIMELINE_EVENTS } from "$lib/mock-timeline-data";
  import { addDaysIso, groupEventsByHour, todayIso, type RecallEvent } from "$lib/timeline";
  import TimelineDateNav from "./TimelineDateNav.svelte";
  import TimelineEvent from "./TimelineEvent.svelte";

  let selectedDate = $state(todayIso());
  let events: RecallEvent[] = $state(MOCK_TIMELINE_EVENTS);
  let doneSet: Set<number> = $state(new Set());

  function shiftDate(days: number) {
    selectedDate = addDaysIso(selectedDate, days);
  }

  function toggleDone(index: number) {
    if (doneSet.has(index)) {
      doneSet.delete(index);
    } else {
      doneSet.add(index);
    }
    doneSet = new Set(doneSet);
  }

  let groupedByHour = $derived(groupEventsByHour(events));
  let doneCount = $derived(doneSet.size);
  let totalCount = $derived(events.length);
</script>

<div class="space-y-6">
  <TimelineDateNav {selectedDate} onShift={shiftDate} />

  {#if events.length === 0}
    <p class="py-12 text-center text-muted-foreground">No activity found for this day.</p>
  {:else}
    <p class="text-center text-sm text-muted-foreground">
      {doneCount} / {totalCount} logged to Harvest
    </p>

    <div class="space-y-6">
      {#each groupedByHour as group}
        <div class="flex gap-4">
          <div class="w-14 shrink-0 pt-1 text-right">
            <span class="font-head text-sm text-muted-foreground">{group.hour}</span>
          </div>
          <div class="flex-1 space-y-2">
            {#each group.items as { event, index }}
              <TimelineEvent {event} done={doneSet.has(index)} onToggle={() => toggleDone(index)} />
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
