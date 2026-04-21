<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Popover from "$lib/components/ui/popover/index.js";
  import { Calendar } from "$lib/components/ui/calendar/index.js";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { parseDate, today, getLocalTimeZone, type DateValue } from "@internationalized/date";
  import { formatDayHeadingParts, todayIso } from "$lib/timeline";
  import { navState } from "$lib/nav-state.svelte";
  import { commands } from "../bindings";

  let {
    selectedDate,
    onShift,
    onGoToday,
    onPick,
  }: {
    selectedDate: string;
    onShift: (days: number) => void;
    onGoToday: () => void;
    onPick: (iso: string) => void;
  } = $props();

  let headingParts = $derived(formatDayHeadingParts(selectedDate));
  let atToday = $derived(selectedDate === todayIso());

  let pickerOpen = $state(false);
  let pickerValue = $derived(parseDate(selectedDate));
  let calendarMonth = $state<DateValue | undefined>(undefined);
  let loadingMonth = $state(false);

  function handlePick(next: DateValue | undefined) {
    if (!next) return;
    const iso = next.toString();
    if (iso !== selectedDate) onPick(iso);
    pickerOpen = false;
  }

  $effect(() => {
    if (pickerOpen) calendarMonth = pickerValue;
  });

  async function loadMonth() {
    if (loadingMonth) return;
    const month = calendarMonth ?? pickerValue;
    loadingMonth = true;

    const todayStr = todayIso();
    const year = month.year;
    const mo = month.month;
    const daysInMonth = new Date(year, mo, 0).getDate();

    const daysToLoad: string[] = [];
    for (let d = 1; d <= daysInMonth; d++) {
      const iso = `${year}-${String(mo).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
      if (iso >= todayStr) continue;
      if (navState.dayCounts[iso] !== undefined) continue;
      daysToLoad.push(iso);
    }

    const promises = daysToLoad.map(
      (day, i) =>
        new Promise<void>((resolve) => {
          window.setTimeout(async () => {
            const result = await commands.getTimelineForDay(day);
            if (result.status === "ok") {
              navState.dayCounts[day] = result.data.length;
            }
            resolve();
          }, i * 100);
        }),
    );

    await Promise.all(promises);
    loadingMonth = false;
  }

  let visibleMonthLabel = $derived.by(() => {
    const m = calendarMonth ?? pickerValue;
    return new Date(m.year, m.month - 1, 1).toLocaleString("default", {
      month: "long",
      year: "numeric",
    });
  });
</script>

<div class="flex items-center gap-5 mb-8">
  <Button variant="outline" size="icon" onclick={() => onShift(-1)}>
    <ChevronLeft />
  </Button>

  <Popover.Root bind:open={pickerOpen}>
    <Popover.Trigger
      class="font-head min-w-0 text-xl cursor-pointer border-2 border-transparent px-2 py-0.5 transition hover:border-border hover:bg-accent hover:text-accent-foreground outline-hidden focus-visible:border-border"
      aria-label="Pick a date"
    >
      <span class="block text-muted-foreground xs:inline">{headingParts.weekday}</span>
      <span class="block xs:inline">{headingParts.monthDay}</span>
    </Popover.Trigger>
    <Popover.Content align="start" sideOffset={8}>
      <Calendar
        type="single"
        value={pickerValue}
        onValueChange={handlePick}
        weekStartsOn={1}
        dayCounts={navState.dayCounts}
        bind:placeholder={calendarMonth}
      />
      <div class="px-1 pb-1">
        <Button
          variant="outline"
          size="sm"
          class="w-full text-xs"
          disabled={loadingMonth}
          onclick={loadMonth}
        >
          {loadingMonth ? "Loading…" : `Load ${visibleMonthLabel}`}
        </Button>
      </div>
    </Popover.Content>
  </Popover.Root>

  {#if !atToday}
    <Button variant="outline" size="icon" onclick={() => onShift(1)}>
      <ChevronRight />
    </Button>

    <Button variant="outline" class="text-sm" onclick={onGoToday}>
      Today
    </Button>
  {/if}
</div>
