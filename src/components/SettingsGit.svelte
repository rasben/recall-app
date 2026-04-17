<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { toast } from "svelte-sonner";
  import { commands } from "../bindings";
  import { open } from "@tauri-apps/plugin-dialog";
  import FolderOpen from "@lucide/svelte/icons/folder-open";

  let gitEnabled = $state(false);
  let gitScanPath = $state("");

  $effect(() => {
    commands.getGitEnabled().then((enabled: boolean) => {
      gitEnabled = enabled;
    });
    commands.getGitScanPath().then((path: string | null) => {
      gitScanPath = path ?? "";
    });
  });

  async function toggleGitEnabled(checked: boolean) {
    gitEnabled = checked;
    const result = await commands.setGitEnabled(checked);
    if (result.status === "error") {
      toast.error("Failed to save setting");
    }
  }

  async function pickGitFolder() {
    const selected = await open({ directory: true, multiple: false, title: "Choose git scan directory" });
    if (selected) {
      gitScanPath = selected;
      const result = await commands.setGitScanPath(selected);
      if (result.status === "error") {
        toast.error("Failed to save path");
        return;
      }
      toast.success("Scan path saved!");
    }
  }

  async function saveGitPath() {
    const result = await commands.setGitScanPath(gitScanPath);
    if (result.status === "error") {
      toast.error("Failed to save path");
      return;
    }
    toast.success("Scan path saved!");
  }
</script>

<fieldset class="border-2 p-4 mt-4">
  <legend class="mb-2">Local Git commits</legend>

  <div class="flex items-center gap-2 mb-4">
    <Checkbox
      id="git-enabled"
      checked={gitEnabled}
      onCheckedChange={(v) => toggleGitEnabled(v === true)}
    />
    <Label for="git-enabled">Enable local git source</Label>
  </div>

  {#if gitEnabled}
    <Label for="git-scan-path" class="mb-2">Directory to scan for git repos</Label>
    <div class="flex gap-2">
      <Input
        id="git-scan-path"
        placeholder="~/code"
        bind:value={gitScanPath}
        onblur={saveGitPath}
        class="flex-1"
      />
      <Button variant="outline" size="icon" onclick={pickGitFolder} title="Browse…">
        <FolderOpen />
      </Button>
    </div>
    {#if gitScanPath}
      <p class="text-muted-foreground text-sm mt-2">Will scan <strong>{gitScanPath}</strong> for git repositories.</p>
    {/if}
  {/if}
</fieldset>
