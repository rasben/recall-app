<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { toast } from "svelte-sonner";
  import { t } from "$lib/i18n.svelte";
  import type { Result } from "../../bindings";

  let { test }: { test: () => Promise<Result<null, string>> } = $props();

  let testing = $state(false);

  async function run() {
    testing = true;
    const result = await test();
    testing = false;
    if (result.status === "ok") {
      toast.success(t("settings.test_connection.ok"), {richColors: true});
    } else {
      toast.error(result.error, {richColors: true});
    }
  }
</script>

<Button
  class="absolute top-3 right-3"
  variant="outline"
  size="sm"
  onclick={run}
  disabled={testing}
>
  {t("settings.test_connection")}
</Button>
