<script lang="ts">
  import { Popover as PopoverPrimitive } from "bits-ui";
  import { cn, type WithoutChildrenOrChild } from "$lib/utils.js";

  let {
    ref = $bindable(null),
    class: className,
    sideOffset = 6,
    align = "start",
    portalProps,
    children,
    ...restProps
  }: WithoutChildrenOrChild<PopoverPrimitive.ContentProps> & {
    portalProps?: PopoverPrimitive.PortalProps;
    children?: import("svelte").Snippet;
  } = $props();
</script>

<PopoverPrimitive.Portal {...portalProps}>
  <PopoverPrimitive.Content
    bind:ref
    data-slot="popover-content"
    {sideOffset}
    {align}
    class={cn(
      "bg-popover text-popover-foreground z-50 w-auto origin-(--bits-popover-content-transform-origin) border-2 border-border bg-background p-3 shadow-md outline-hidden",
      "data-[state=open]:animate-in data-[state=closed]:animate-out",
      "data-[state=open]:fade-in-0 data-[state=closed]:fade-out-0",
      "data-[state=open]:zoom-in-95 data-[state=closed]:zoom-out-95",
      "data-[side=bottom]:slide-in-from-top-2 data-[side=top]:slide-in-from-bottom-2",
      "data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2",
      className,
    )}
    {...restProps}
  >
    {@render children?.()}
  </PopoverPrimitive.Content>
</PopoverPrimitive.Portal>
