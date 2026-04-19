import { Popover as PopoverPrimitive } from "bits-ui";
import Content from "./popover-content.svelte";

const Root = PopoverPrimitive.Root;
const Trigger = PopoverPrimitive.Trigger;
const Close = PopoverPrimitive.Close;
const Portal = PopoverPrimitive.Portal;

export {
  Root,
  Trigger,
  Content,
  Close,
  Portal,
  //
  Root as Popover,
  Trigger as PopoverTrigger,
  Content as PopoverContent,
  Close as PopoverClose,
  Portal as PopoverPortal,
};
