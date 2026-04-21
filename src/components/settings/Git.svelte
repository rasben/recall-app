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

  let settings = $state<SettingsGit>({ enabled: false, path: "~/code" });

  onMount(async () => {
      settings = await commands.getSettingsGit() ?? { enabled: false, path: "~/code" };
  });

  async function save() {
      const result = await commands.setSettingsGit(settings);
      if (result.status === "error") return false;
      return true;
  }

  async function toggleEnabled(checked: boolean) {
      settings.enabled = checked;
      const ok = await save();
      if (!ok) {
          settings.enabled = !checked;
          toast.error("Could not enable Git source");
      }
  }

  async function setPath() {
      const ok = await save();
      if (!ok) toast.error("Could not save Git path");
      else toast.success("Git path saved!");
  }

  async function pickFolder() {
      const selected = await open({ directory: true, multiple: false, title: "Choose git scan directory" });
      if (selected) {
          settings.path = selected;
          await setPath();
      }
  }
</script>


<fieldset class="border-2 p-4 mt-6">
    <legend>Local Git commits</legend>

    <div class="flex items-center gap-2 mb-4">
        <Checkbox
            id="git-enabled"
            checked={settings.enabled}
            onCheckedChange={(v) => toggleEnabled(v === true)}
        />
        <Label for="git-enabled">Enable local git source</Label>
    </div>

    {#if settings.enabled}
        <Label for="git-scan-path" class="mb-2">Directory to scan for git repos</Label>
        <div class="flex gap-2">
            <Input
                id="git-scan-path"
                placeholder="~/code"
                bind:value={settings.path}
                onblur={setPath}
                class="flex-1"
            />
            <Button variant="outline" size="icon" onclick={pickFolder} title="Browse…">
                <FolderOpen />
            </Button>
        </div>
        {#if settings.path}
            <p class="text-muted-foreground text-sm mt-2 px-4">Will scan <strong>{settings.path}</strong> for git repositories.</p>
        {/if}
    {/if}
</fieldset>
