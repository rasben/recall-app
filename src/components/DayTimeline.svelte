<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { cubicOut, quintOut } from "svelte/easing";
  import { commands } from "../bindings";
  import { addDaysIso, groupEventsByHour, todayIso, type TimelineEvent } from "$lib/timeline";
  import TimelineDateNav from "./TimelineDateNav.svelte";
  import TimelineEventRow from "./TimelineEvent.svelte";
  import Loading from "./ui/Loading.svelte";

  const LOAD_DEBOUNCE_MS = 500;
  /** Stagger starts after loading so the first row does not land the same instant as the spinner handoff. */
  const ROW_INTRO_BASE_DELAY_MS = 80;
  const ROW_FLY_MS = 520;
  const ROW_STAGGER_MS = 58;
  const ROW_STAGGER_CAP_MS = 1600;

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

  function goToday() {
    selectedDate = todayIso();
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

<div class="space-y-6 relative">
  <TimelineDateNav {selectedDate} onShift={shiftDate} onGoToday={goToday} />

  {#if loadError}
    <p
      class="rounded border-2 border-destructive bg-destructive/10 p-4 text-sm text-destructive"
      transition:fade={{ duration: 200, easing: cubicOut }}
    >
      {loadError}
    </p>
  {/if}

  {#if isLoading}
    <div class="absolute" in:fade={{ duration: 180, easing: cubicOut }} out:fade={{ duration: 240, easing: cubicOut }}>
      <Loading />
    </div>
  {:else if events.length === 0 && !loadError}
    <p class=" text-muted-foreground" in:fade|global={{ duration: 240, easing: cubicOut }}>
      No activity found for this day.
    </p>
  {:else}
    <div class="space-y-6">
      {#each groupedByHour as group (`${selectedDate}-${group.hour}`)}
        <div class="flex gap-4">
          <div class="w-14 shrink-0 pt-1 text-right">
            <span class="font-head text-sm text-muted-foreground">{group.hour}</span>
          </div>
          <div class="min-w-0 flex-1 space-y-2">
            {#each group.items as { event, index } (`${selectedDate}-${event.id}`)}
              <div
                class="will-change-transform"
                in:fly|global={{
                  y: 22,
                  duration: ROW_FLY_MS,
                  delay: ROW_INTRO_BASE_DELAY_MS + Math.min(index * ROW_STAGGER_MS, ROW_STAGGER_CAP_MS),
                  easing: quintOut,
                }}
              >
                <TimelineEventRow {event} done={doneIds.has(event.id)} onToggle={() => toggleDone(event.id)} />
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
