<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Popover from "$lib/components/ui/popover/index.js";
  import { Calendar } from "$lib/components/ui/calendar/index.js";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { parseDate, type DateValue } from "@internationalized/date";
  import { formatDayHeadingParts, todayIso } from "$lib/timeline";

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
  /** bits-ui Calendar works on a CalendarDate; derive it from the ISO string. */
  let pickerValue = $derived(parseDate(selectedDate));

  function handlePick(next: DateValue | undefined) {
    if (!next) return;
    const iso = next.toString(); // CalendarDate.toString() → "YYYY-MM-DD"
    if (iso !== selectedDate) onPick(iso);
    pickerOpen = false;
  }
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
      />
    </Popover.Content>
  </Popover.Root>

  {#if !atToday}
    <Button variant="outline" size="icon" onclick={() => onShift(1)}>
      <ChevronRight />
    </Button>

    <Button variant="outline" class="max-h-[32px] text-sm" onclick={onGoToday}>
      Today
    </Button>
  {/if}

</div>
