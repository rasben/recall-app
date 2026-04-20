<script lang="ts">
  import { fade } from "svelte/transition";
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import CloseIcon from "@lucide/svelte/icons/x";
  import Settings from "../components/settings/Settings.svelte";
  import Main from "../components/Main.svelte";
  import { commands } from "../bindings";

  let settingsOpen = $state(false);

  onMount(async () => {
    // Trigger a background iCal sync on startup, but only if the data is stale
    // (more than 1 hour old) to avoid re-syncing on every app restart.
    const status = await commands.getIcalSyncStatus();
    const oneHourAgo = Date.now() - 60 * 60 * 1000;
    if (!status.syncing && (!status.last_synced_at || status.last_synced_at < oneHourAgo)) {
      commands.triggerIcalSync();
    }
  });
</script>

<Button
  variant={settingsOpen ? "secondary" : "outline"}
  size="icon"
  class="absolute top-8 right-8 z-50"
  aria-pressed={settingsOpen}
  aria-label={settingsOpen ? "Close settings" : "Open settings"}
  onclick={() => (settingsOpen = !settingsOpen)}
>
  {#if settingsOpen}
    <CloseIcon />
  {:else}
    <SettingsIcon />
  {/if}
</Button>

<main class="flex min-h-screen flex-col p-8">
  {#if settingsOpen}
    <div in:fade={{ duration: 220 }} out:fade={{ duration: 160 }}>
      <Settings />
    </div>
  {:else}
    <div in:fade={{ duration: 220 }} out:fade={{ duration: 160 }}>
      <Main />
    </div>
  {/if}
</main>
