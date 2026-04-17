<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { toast } from "svelte-sonner";
  import { open } from "@tauri-apps/plugin-dialog";
  import {onMount} from "svelte";
  import { commands } from "../../bindings";
  import type { SettingsGit } from "../../bindings"
  import FolderOpen from "@lucide/svelte/icons/folder-open";

  let defaultSettings = {
      enabled: false,
      path: "~/code"
  };

  let settings = $state<SettingsGit>(defaultSettings);

  let enabled = $state(false);
  let path = $state("");

  onMount(() => {
      getSettings();
  });

  async function getSettings() {
      settings = await commands.getSettingsGit() ?? defaultSettings;
      enabled = settings.enabled;
      path = settings.path;
  }

  async function toggleEnabled(checked: boolean) {
      enabled = checked;
      settings.enabled = checked;

      const result = await commands.setSettingsGit(settings)

      if (result.status === "error") {
          toast.error("Could not enable Git source");
      }
  }

  async function setPath() {
      settings.path = path;

      const result = await commands.setSettingsGit(settings)

      if (result.status === "error") {
          toast.error("Could not save Git path");
      }
      else {
          toast.success("Git path saved!")
      }
  }

  async function pickFolder() {
      const selected = await open({ directory: true, multiple: false, title: "Choose git scan directory" });
      if (selected) {
          path = selected;
          await setPath();
      }
  }


</script>


<fieldset class="border-2 p-4 mt-4">
    <legend class="mb-2">Local Git commits</legend>

    <div class="flex items-center gap-2 mb-4">
        <Checkbox
                id="git-enabled"
                checked={enabled}
                onCheckedChange={(v) => toggleEnabled(v === true)}
        />
        <Label for="git-enabled">Enable local git source</Label>
    </div>

    {#if enabled}
        <Label for="git-scan-path" class="mb-2">Directory to scan for git repos</Label>
        <div class="flex gap-2">
            <Input
                    id="git-scan-path"
                    placeholder="~/code"
                    bind:value={path}
                    onblur={setPath}
                    class="flex-1"
            />
            <Button variant="outline" size="icon" onclick={pickFolder} title="Browse…">
                <FolderOpen />
            </Button>
        </div>
        {#if path}
            <p class="text-muted-foreground text-sm mt-2">Will scan <strong>{path}</strong> for git repositories.</p>
        {/if}
    {/if}
</fieldset>
