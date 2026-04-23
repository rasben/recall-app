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
  import { t, langLocale } from "$lib/i18n.svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onDestroy } from "svelte";

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

  let headingParts = $derived(formatDayHeadingParts(selectedDate, langLocale()));
  let atToday = $derived(selectedDate === todayIso());

  let pickerOpen = $state(false);
  let pickerValue = $derived(parseDate(selectedDate));
  let calendarMonth = $state<DateValue | undefined>(undefined);
  let loadingMonth = $state(false);
  const MONTH_SOURCES_TOTAL = 5;
  let monthSourcesDone = $state(0);
  let monthProgress = $derived(
    loadingMonth ? Math.min(monthSourcesDone / MONTH_SOURCES_TOTAL, 1) : 0,
  );

  let unlistenMonth: UnlistenFn | undefined;
  listen<{ source: string; done: boolean }>("month:source", (e) => {
    if (e.payload.done) monthSourcesDone += 1;
  }).then((fn) => (unlistenMonth = fn));

  onDestroy(() => unlistenMonth?.());

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
    monthSourcesDone = 0;
    loadingMonth = true;

    const result = await commands.getDayCountsForMonth(month.year, month.month);
    if (result.status === "ok") {
      Object.assign(navState.dayCounts, result.data);
    }

    loadingMonth = false;
    monthSourcesDone = 0;
  }

  let visibleMonthLabel = $derived.by(() => {
    const m = calendarMonth ?? pickerValue;
    return new Date(m.year, m.month - 1, 1).toLocaleString(langLocale(), {
      month: "long",
      year: "numeric",
    });
  });
</script>

<div class="flex items-center gap-5 mb-8 relative z-2">
  <Button variant="outline" size="icon" onclick={() => onShift(-1)}>
    <ChevronLeft />
  </Button>

  <Popover.Root bind:open={pickerOpen}>
    <Popover.Trigger
      class="font-head min-w-0 text-xl cursor-pointer border-2 border-transparent px-2 py-0.5 transition hover:border-border hover:bg-accent hover:text-accent-foreground outline-hidden focus-visible:border-border"
      aria-label={t("timeline.pick_date")}
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
        maxValue={today(getLocalTimeZone())}
        bind:placeholder={calendarMonth}
      />
      <div class="px-1 pb-1 space-y-2">
        <div class="flex items-center justify-center gap-1.5">
          <div class="size-2.5 bg-muted/50 border border-border/40"></div>
          <div class="size-2.5 bg-primary/15 border border-border/40"></div>
          <div class="size-2.5 bg-primary/30 border border-border/40"></div>
          <div class="size-2.5 bg-primary/50 border border-border/40"></div>
          <div class="size-2.5 bg-primary/70 border border-border/40"></div>
          <span class="text-[0.6rem] text-muted-foreground">{t("timeline.less_more_activity")}</span>
        </div>

        <Button
                variant="outline"
                size="sm"
                class="load-month-btn relative w-full overflow-hidden text-xs"
                disabled={loadingMonth}
                onclick={loadMonth}
        >
          {#if loadingMonth}
            <span
              class="load-month-progress pointer-events-none absolute inset-y-0 left-0 bg-primary/20"
              style="width: {monthProgress * 100}%"
              aria-hidden="true"
            ></span>
          {/if}
          <span class="relative">
            {loadingMonth ? t("timeline.loading_month") : t("timeline.load_month", { month: visibleMonthLabel })}
          </span>
        </Button>
      </div>
    </Popover.Content>
  </Popover.Root>

  {#if !atToday}
    <Button variant="outline" size="icon" onclick={() => onShift(1)}>
      <ChevronRight />
    </Button>

    <Button variant="outline" class="text-sm" onclick={onGoToday}>
      {t("timeline.today")}
    </Button>
  {/if}
</div>

<style>
  :global(.load-month-btn) .load-month-progress {
    transition: width 300ms ease-out;
  }
</style>
