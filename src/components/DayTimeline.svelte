<script lang="ts">
  import { commands } from "../bindings";
  import { addDaysIso, groupEventsByHour, todayIso, type TimelineEvent } from "$lib/timeline";
  import TimelineDateNav from "./TimelineDateNav.svelte";
  import TimelineEventRow from "./TimelineEvent.svelte";
  import Loading from "./ui/Loading.svelte";

  const LOAD_DEBOUNCE_MS = 500;

  let selectedDate = $state(todayIso());
  let events = $state<TimelineEvent[]>([]);
  let loadError = $state<string | null>(null);
  let doneIds = $state<Set<string>>(new Set());
  let isLoading = $state(true);
  /** After first fetch, debounce so rapid day clicks only load the final day. */
  let pastInitialDay = $state(false);

  function shiftDate(days: number) {
    selectedDate = addDaysIso(selectedDate, days);
  }

  function toggleDone(id: string) {
    if (doneIds.has(id)) {
      doneIds.delete(id);
    } else {
      doneIds.add(id);
    }
    doneIds = new Set(doneIds);
  }

  $effect(() => {
    const day = selectedDate;
    const debounceMs = pastInitialDay ? LOAD_DEBOUNCE_MS : 0;
    pastInitialDay = true;

    doneIds = new Set();
    loadError = null;
    events = [];
    isLoading = true;

    let cancelled = false;
    const timeoutId = window.setTimeout(() => {
      commands.getTimelineForDay(day).then((result) => {
        if (cancelled) return;
        if (result.status === "ok") {
          events = result.data;
          loadError = null;
        } else {
          loadError = result.error;
          events = [];
        }
        isLoading = false;
      });
    }, debounceMs);

    return () => {
      cancelled = true;
      window.clearTimeout(timeoutId);
    };
  });

  let groupedByHour = $derived(groupEventsByHour(events));
</script>

<div class="space-y-6">
  <TimelineDateNav {selectedDate} onShift={shiftDate} />

  {#if loadError}
    <p class="rounded border-2 border-destructive bg-destructive/10 px-3 py-2 text-center text-sm text-destructive">
      {loadError}
    </p>
  {/if}

  {#if isLoading}
    <Loading />
  {:else if events.length === 0 && !loadError}
    <p class="py-12 text-center text-muted-foreground">No activity found for this day.</p>
  {:else}
    <div class="space-y-6">
      {#each groupedByHour as group}
        <div class="flex gap-4">
          <div class="w-14 shrink-0 pt-1 text-right">
            <span class="font-head text-sm text-muted-foreground">{group.hour}</span>
          </div>
          <div class="min-w-0 flex-1 space-y-2">
            {#each group.items as { event }}
              <TimelineEventRow {event} done={doneIds.has(event.id)} onToggle={() => toggleDone(event.id)} />
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
