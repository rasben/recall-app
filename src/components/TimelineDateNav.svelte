<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { formatDayHeadingParts, todayIso } from "$lib/timeline";

  let {
    selectedDate,
    onShift,
    onGoToday,
  }: { selectedDate: string; onShift: (days: number) => void; onGoToday: () => void } = $props();

  let headingParts = $derived(formatDayHeadingParts(selectedDate));
  let atToday = $derived(selectedDate === todayIso());
</script>

<div class="flex items-center gap-5 mb-8">
  <Button variant="outline" size="icon" onclick={() => onShift(-1)}>
    <ChevronLeft />
  </Button>

  <h2 class="font-head min-w-0 text-xl">
    <span class="block text-muted-foreground xs:inline">{headingParts.weekday}</span>
    <span class="block xs:inline">{headingParts.monthDay}</span>
  </h2>

  {#if !atToday}
    <Button variant="outline" size="icon" onclick={() => onShift(1)}>
      <ChevronRight />
    </Button>

    <Button variant="outline" class="max-h-[32px] text-sm" onclick={onGoToday}>
      Today
    </Button>
  {/if}

</div>
