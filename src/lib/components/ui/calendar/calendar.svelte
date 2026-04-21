<script lang="ts">
  import { Calendar as CalendarPrimitive } from "bits-ui";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { cn, type WithoutChildrenOrChild } from "$lib/utils.js";

  type Props = WithoutChildrenOrChild<CalendarPrimitive.RootProps> & {
    buttonVariant?: "default" | "outline" | "ghost";
    dayCounts?: Record<string, number>;
  };

  let {
    ref = $bindable(null),
    value = $bindable(),
    placeholder = $bindable(),
    weekdayFormat = "short",
    class: className,
    dayCounts = {},
    ...restProps
  }: Props = $props();

  function cellBgClass(iso: string): string {
    if (!(iso in dayCounts)) return "";
    const count = dayCounts[iso];
    if (count === 0) return "bg-muted/50";
    if (count <= 2)  return "bg-primary/15";
    if (count <= 5)  return "bg-primary/30";
    if (count <= 9)  return "bg-primary/50";
    return "bg-primary/70";
  }
</script>

<CalendarPrimitive.Root
  bind:ref
  bind:value={value as never}
  bind:placeholder
  {weekdayFormat}
  class={cn("select-none p-1", className)}
  {...restProps}
>
  {#snippet children({ months, weekdays })}
    <CalendarPrimitive.Header class="flex items-center justify-between pb-2">
      <CalendarPrimitive.PrevButton
        class="inline-flex size-7 items-center justify-center border-2 border-border bg-transparent transition hover:translate-y-0.5 hover:bg-accent hover:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-40"
      >
        <ChevronLeft class="size-4" />
      </CalendarPrimitive.PrevButton>

      <CalendarPrimitive.Heading class="font-head text-sm" />

      <CalendarPrimitive.NextButton
        class="inline-flex size-7 items-center justify-center border-2 border-border bg-transparent transition hover:translate-y-0.5 hover:bg-accent hover:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-40"
      >
        <ChevronRight class="size-4" />
      </CalendarPrimitive.NextButton>
    </CalendarPrimitive.Header>

    <div class="flex flex-col gap-4 sm:flex-row">
      {#each months as month (month.value)}
        <CalendarPrimitive.Grid class="w-full border-collapse space-y-1">
          <CalendarPrimitive.GridHead>
            <CalendarPrimitive.GridRow class="flex">
              {#each weekdays as weekday (weekday)}
                <CalendarPrimitive.HeadCell
                  class="w-8 rounded-none text-center text-[0.7rem] font-normal text-muted-foreground"
                >
                  {weekday.slice(0, 2)}
                </CalendarPrimitive.HeadCell>
              {/each}
            </CalendarPrimitive.GridRow>
          </CalendarPrimitive.GridHead>

          <CalendarPrimitive.GridBody>
            {#each month.weeks as weekDates (weekDates[0].toString())}
              <CalendarPrimitive.GridRow class="mt-1 flex w-full">
                {#each weekDates as date (date.toString())}
                  {@const iso = date.toString()}
                  {@const inMonth = date.month === month.value.month && date.year === month.value.year}
                  <CalendarPrimitive.Cell
                    {date}
                    month={month.value}
                    class={cn("relative size-8 p-0 text-center text-sm", inMonth && cellBgClass(iso))}
                  >
                    <CalendarPrimitive.Day
                      class={cn(
                        "inline-flex size-8 items-center justify-center border-2 border-transparent font-normal transition",
                        "hover:border-border hover:bg-accent hover:text-accent-foreground",
                        "data-[selected]:bg-primary data-[selected]:text-primary-foreground data-[selected]:border-border data-[selected]:shadow-xs data-[selected]:font-head",
                        "data-[today]:font-head data-[today]:underline",
                        "data-[outside-month]:text-muted-foreground/50 data-[outside-month]:pointer-events-none",
                        "data-[disabled]:pointer-events-none data-[disabled]:opacity-30",
                        "data-[unavailable]:text-destructive data-[unavailable]:line-through",
                      )}
                    />
                  </CalendarPrimitive.Cell>
                {/each}
              </CalendarPrimitive.GridRow>
            {/each}
          </CalendarPrimitive.GridBody>
        </CalendarPrimitive.Grid>
      {/each}
    </div>
  {/snippet}
</CalendarPrimitive.Root>
