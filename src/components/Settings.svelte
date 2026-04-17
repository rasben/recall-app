<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
  import { toast } from "svelte-sonner";
  import { commands } from "../bindings";
  import { applyTheme } from "$lib/theme";
  import SettingsGit from "./SettingsGit.svelte";

  let theme = $state("system");

  $effect(() => {
    commands.getTheme().then((saved: string | null) => {
      theme = saved ?? "system";
    });
  });

  async function setTheme(value: string) {
    if (!value) return;
    theme = value;
    applyTheme(value);

    const result = await commands.setTheme(value);
    if (result.status === "error") {
      toast.error("Failed to save theme");
      return;
    }
    toast.success("Theme updated!");
  }
</script>

<fieldset class="border-2 p-4">
  <legend class="mb-2">Theme</legend>
  <ToggleGroup.Root
    variant="outlined"
    type="single"
    class="w-full"
    id="theme"
    value={theme}
    onValueChange={setTheme}
  >
    <ToggleGroup.Item value="light">Light</ToggleGroup.Item>
    <ToggleGroup.Item value="dark">Dark</ToggleGroup.Item>
    <ToggleGroup.Item value="system">System</ToggleGroup.Item>
  </ToggleGroup.Root>
</fieldset>

<SettingsGit />
