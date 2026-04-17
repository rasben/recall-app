<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { formatDayHeading, todayIso } from "$lib/timeline";

  let { selectedDate, onShift }: { selectedDate: string; onShift: (days: number) => void } = $props();

  let heading = $derived(formatDayHeading(selectedDate));
  let atToday = $derived(selectedDate === todayIso());
</script>

<div class="flex items-center gap-4">
  <Button variant="outline" size="icon" onclick={() => onShift(-1)}>
    <ChevronLeft />
  </Button>

  <h2 class="font-head text-center text-xl">{heading}</h2>

  {#if !atToday}
    <Button variant="outline" size="icon" onclick={() => onShift(1)} disabled={atToday}>
      <ChevronRight />
    </Button>
  {/if}

</div>
