<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { toast } from "svelte-sonner";
  import { t } from "$lib/i18n.svelte";
  import type { Result } from "../../bindings";
  import BugPlay from "@lucide/svelte/icons/bug-play";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import Check from "@lucide/svelte/icons/check";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";

  let { test }: { test: () => Promise<Result<null, string>> } = $props();

  type Status = "idle" | "testing" | "ok" | "error";
  let status = $state<Status>("idle");

  async function run() {
    status = "testing";
    const result = await test();
    if (result.status === "ok") {
      status = "ok";
    } else {
      status = "error";
      toast.error(result.error, { richColors: true });
    }
  }
</script>

<Button
  class="absolute top-0 right-3 text-xs"
  variant="outline"
  size="icon"
  onclick={run}
  disabled={status === "testing"}
  title={t("settings.test_connection")}
>
  {#if status === "testing"}
    <LoaderCircle class="animate-spin" />
  {:else if status === "ok"}
    <Check class="text-green-600" />
  {:else if status === "error"}
    <TriangleAlert class="text-red-600" />
  {:else}
    <BugPlay />
  {/if}
</Button>
