<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { cubicOut, quintOut } from "svelte/easing";
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { commands } from "../bindings";
  import { addDaysIso, applyOptimisticToggle, formatGapLabel, GAP_IDLE_MINUTES, GAP_MIN_MINUTES, groupByTask, groupCloseCommits, groupEventsByHour, rollbackOptimisticToggle, todayIso, type TimelineEvent } from "$lib/timeline";
  import { navState } from "$lib/nav-state.svelte";
  import TimelineDateNav from "./TimelineDateNav.svelte";
  import TimelineSourceFilter from "./TimelineSourceFilter.svelte";
  import TimelineEventRow from "./TimelineEvent.svelte";
  import TimelineCommitGroup from "./TimelineCommitGroup.svelte";
  import TimelineTicketGroup from "./TimelineTicketGroup.svelte";
  import Loading from "./ui/Loading.svelte";
  import MissingSettings from "./ui/MissingSettings.svelte";
  import { t } from "$lib/i18n.svelte";
  import Travolta from "./ui/Travolta.svelte";
  import NyanCat from "./ui/NyanCat.svelte";
  import Waiting from "./ui/Waiting.svelte";

  const LOAD_DEBOUNCE_MS = 500;
  const PREFETCH_DAYS = 6;
  const PREFETCH_STAGGER_MS = 300;
  /** Stagger starts after loading so the first row does not land the same instant as the spinner handoff. */
  const ROW_INTRO_BASE_DELAY_MS = 80;
  const ROW_FLY_MS = 520;
  const ROW_STAGGER_MS = 58;
  const ROW_STAGGER_CAP_MS = 1600;

  let selectedDate = $derived(navState.selectedDate);
  let events = $state<TimelineEvent[]>([]);
  let loadError = $state<string | null>(null);
  let sourceErrors = $state(new Map<string, string>());
  let doneIds = $state<Set<string>>(new Set());
  let isLoading = $state(true);
  let loadingSource = $state<string | null>(null);
  let doneSources = $state(new Set<string>());
  let enabledSources = $state<string[]>([]);
  let settingsLoaded = $state(false);
  /** After first fetch, debounce so rapid day clicks only load the final day. */
  let pastInitialDay = $state(false);

  let unlistenSource: (() => void) | null = null;
  listen<{ source: string; done: boolean; error?: string }>("timeline:source", ({ payload }) => {
    if (payload.done) {
      doneSources = new Set([...doneSources, payload.source]);
      loadingSource = null;
      if (payload.error) {
        sourceErrors = new Map([...sourceErrors, [payload.source, payload.error]]);
      }
    } else {
      loadingSource = payload.source;
    }
  }).then((unlisten) => {
    unlistenSource = unlisten;
  });

  onDestroy(() => unlistenSource?.());

  function shiftDate(days: number) {
    navState.selectedDate = addDaysIso(navState.selectedDate, days);
  }

  function goToday() {
    navState.selectedDate = todayIso();
  }

  function pickDate(iso: string) {
    navState.selectedDate = iso;
  }

  async function refreshDay() {
    if (isLoading) return;
    const day = selectedDate;
    doneIds = new Set();
    doneSources = new Set();
    sourceErrors = new Map();
    loadingSource = null;
    loadError = null;
    events = [];
    isLoading = true;

    const result = await commands.refreshTimelineForDay(day);
    if (selectedDate !== day) return;
    if (result.status === "ok") {
      events = result.data;
      loadError = null;
      navState.dayCounts[day] = result.data.length;
      const ids = result.data.map((e) => e.id);
      const harvest = await commands.getTimelineHarvestDoneForEventIds(ids);
      if (selectedDate !== day) return;
      doneIds = harvest.status === "ok" ? new Set(harvest.data) : new Set();
    } else {
      loadError = result.error;
      events = [];
      doneIds = new Set();
    }
    isLoading = false;
  }

  async function toggleDone(id: string) {
    const [optimistic, wasAdded] = applyOptimisticToggle(doneIds, id);
    doneIds = optimistic;

    const result = await commands.setTimelineHarvestDone(id, wasAdded);
    if (result.status === "error") {
      doneIds = rollbackOptimisticToggle(doneIds, id, wasAdded);
    }
  }

  $effect(() => {
    const day = selectedDate;
    const debounceMs = pastInitialDay ? LOAD_DEBOUNCE_MS : 0;
    pastInitialDay = true;

    doneIds = new Set();
    doneSources = new Set();
    sourceErrors = new Map();
    loadingSource = null;
    loadError = null;
    events = [];
    isLoading = true;

    let cancelled = false;
    const timeoutId = window.setTimeout(() => {
      commands.getTimelineForDay(day).then(async (result) => {
        if (cancelled) return;
        if (result.status === "ok") {
          events = result.data;
          loadError = null;
          navState.dayCounts[day] = result.data.length;
          const ids = result.data.map((e) => e.id);
          const harvest = await commands.getTimelineHarvestDoneForEventIds(ids);
          if (cancelled) return;
          if (harvest.status === "ok") {
            doneIds = new Set(harvest.data);
          } else {
            doneIds = new Set();
          }
        } else {
          loadError = result.error;
          events = [];
          doneIds = new Set();
        }
        isLoading = false;
      });
    }, debounceMs);

    return () => {
      cancelled = true;
      window.clearTimeout(timeoutId);
    };
  });

  let visibleEvents = $derived(
    events.filter((e) => !navState.hiddenSources.has(e.source)),
  );
  let visibleRows = $derived(groupCloseCommits(visibleEvents));
  let groupedByHour = $derived(groupEventsByHour(visibleRows));
  let taskRows = $derived(groupByTask(visibleEvents));

  function modeButtonClass(active: boolean): string {
    return active
      ? "border-foreground bg-foreground text-background"
      : "border-dashed border-border bg-transparent text-muted-foreground hover:text-foreground hover:border-foreground";
  }

  onMount(async () => {
    const today = todayIso();
    for (let i = 1; i <= PREFETCH_DAYS; i++) {
      const day = addDaysIso(today, -i);
      window.setTimeout(() => commands.getTimelineForDay(day), i * PREFETCH_STAGGER_MS);
    }

    const countsResult = await commands.getCachedDayEventCounts();
    if (countsResult.status === "ok") {
      Object.assign(navState.dayCounts, countsResult.data);
    }

    const [git, github, ical, jira, zulip] = await Promise.all([
      commands.getSettingsGit(),
      commands.getSettingsGithub(),
      commands.getSettingsIcal(),
      commands.getSettingsJira(),
      commands.getSettingsZulip(),
    ]);
    const enabled: string[] = [];
    if (git?.enabled) enabled.push("Git");
    if (github?.enabled) enabled.push("GitHub");
    if (ical?.enabled) enabled.push("Calendar");
    if (jira?.enabled) enabled.push("Jira");
    if (zulip?.enabled) enabled.push("Zulip");
    enabledSources = enabled;
    settingsLoaded = true;
  });
</script>

<div class="relative space-y-6">
  <TimelineDateNav
    {selectedDate}
    onShift={shiftDate}
    onGoToday={goToday}
    onPick={pickDate}
    onRefresh={refreshDay}
    refreshing={isLoading}
  />

  {#if loadError}
    <p
      class="rounded border-2 border-destructive bg-destructive/10 p-4 text-sm text-destructive"
      transition:fade={{ duration: 200, easing: cubicOut }}
    >
      {loadError}
    </p>
  {/if}

  {#if !isLoading && sourceErrors.size > 0}
    <div
      class="rounded border-2 border-destructive/50 bg-destructive/5 p-3 space-y-1"
      transition:fade={{ duration: 200, easing: cubicOut }}
    >
      {#each [...sourceErrors.entries()] as [source, error]}
        <p class="text-sm text-destructive"><strong>{source}:</strong> {error}</p>
      {/each}
    </div>
  {/if}

  {#if !isLoading && settingsLoaded && enabledSources.length > 0 && events.length > 0}
    <div class="flex flex-wrap items-center gap-x-4 gap-y-2">
      <div class="flex items-center gap-1.5" role="group" aria-label={t("timeline.group_mode")}>
        <button
          type="button"
          onclick={() => (navState.groupMode = "time")}
          aria-pressed={navState.groupMode === "time"}
          class="border-2 px-2 py-1 font-head text-[10px] uppercase tracking-widest transition-colors {modeButtonClass(navState.groupMode === 'time')}"
        >
          {t("timeline.view_by_time")}
        </button>
        <button
          type="button"
          onclick={() => (navState.groupMode = "task")}
          aria-pressed={navState.groupMode === "task"}
          class="border-2 px-2 py-1 font-head text-[10px] uppercase tracking-widest transition-colors {modeButtonClass(navState.groupMode === 'task')}"
        >
          {t("timeline.view_by_task")}
        </button>
      </div>
      <TimelineSourceFilter {enabledSources} />
    </div>
  {/if}

  <div class="relative">
  {#if settingsLoaded && enabledSources.length === 0}
    <MissingSettings message={t("timeline.no_sources")} />

    <Waiting />
  {:else if isLoading}
    <div class="absolute inset-x-0" in:fade={{ duration: 180, easing: cubicOut }} out:fade={{ duration: 240, easing: cubicOut }}>
      <Loading currentSource={loadingSource} {doneSources} {enabledSources} {sourceErrors} />
    </div>
    <NyanCat />
  {:else if visibleEvents.length === 0 && !loadError}
    <p class="relative text-muted-foreground z-2" in:fade|global={{ duration: 240, easing: cubicOut }}>
      {t("timeline.no_activity")}
    </p>

    <Travolta />
  {:else if navState.groupMode === 'task'}
    <div class="space-y-2">
      {#each taskRows as row, index (`${selectedDate}-${row.kind === 'group' ? row.key : row.event.id}`)}
        <div
          class="relative z-0 will-change-transform has-[.timeline-event-btn:is(:hover,:focus-within)]:z-10 has-[.timeline-event-btn:is(:hover,:focus-within)]:overflow-visible"
          in:fly|global={{
            y: 22,
            duration: ROW_FLY_MS,
            delay: ROW_INTRO_BASE_DELAY_MS + Math.min(index * ROW_STAGGER_MS, ROW_STAGGER_CAP_MS),
            easing: quintOut,
          }}
        >
          {#if row.kind === 'group'}
            <TimelineTicketGroup label={row.label} keyType={row.keyType} events={row.events} {doneIds} onToggle={toggleDone} />
          {:else}
            <TimelineEventRow event={row.event} done={doneIds.has(row.event.id)} onToggle={() => toggleDone(row.event.id)} />
          {/if}
        </div>
      {/each}
    </div>
  {:else}
    <div class="space-y-6">
      {#each groupedByHour as group (`${selectedDate}-${group.hour}`)}
        <div class="flex gap-4">
          <div class="w-14 shrink-0 pt-1 text-right">
            <span class="font-head text-sm text-muted-foreground">{group.hour}</span>
          </div>
          <div class="min-w-0 flex-1 space-y-2">
            {#each group.items as { row, index, gapMinutes } (`${selectedDate}-${row.kind === 'event' ? row.event.id : row.key}`)}
              {#if gapMinutes !== null && gapMinutes >= GAP_IDLE_MINUTES}
                <div class="flex items-center gap-2 py-1 text-[10px] uppercase tracking-widest text-muted-foreground">
                  <span class="h-px flex-1 bg-border"></span>
                  <span>{t("timeline.idle", { duration: formatGapLabel(gapMinutes) })}</span>
                  <span class="h-px flex-1 bg-border"></span>
                </div>
              {:else if gapMinutes !== null && gapMinutes >= GAP_MIN_MINUTES}
                <div class="pl-1 text-[10px] text-muted-foreground/70" aria-hidden="true">+{formatGapLabel(gapMinutes)}</div>
              {/if}
              <div
                class="relative z-0 will-change-transform has-[.timeline-event-btn:is(:hover,:focus-within)]:z-10 has-[.timeline-event-btn:is(:hover,:focus-within)]:overflow-visible"
                in:fly|global={{
                  y: 22,
                  duration: ROW_FLY_MS,
                  delay: ROW_INTRO_BASE_DELAY_MS + Math.min(index * ROW_STAGGER_MS, ROW_STAGGER_CAP_MS),
                  easing: quintOut,
                }}
              >
                {#if row.kind === 'event'}
                  <TimelineEventRow event={row.event} done={doneIds.has(row.event.id)} onToggle={() => toggleDone(row.event.id)} />
                {:else}
                  <TimelineCommitGroup events={row.events} {doneIds} onToggle={toggleDone} />
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
  </div>
</div>
