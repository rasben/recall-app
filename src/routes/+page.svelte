<script lang="ts">
  import { fade } from "svelte/transition";
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import CloseIcon from "@lucide/svelte/icons/x";
  import Settings from "../components/settings/Settings.svelte";
  import Main from "../components/Main.svelte";
  import Welcome from "../components/Welcome.svelte";
  import { commands } from "../bindings";

  let settingsOpen = $state(false);
  let welcomed = $state(true); // optimistic: don't flash welcome on repeat visits
  let welcomeChecked = $state(false);

  onMount(async () => {
    welcomed = !!localStorage.getItem("recall:welcomed");
    welcomeChecked = true;
    welcomed = false;

    const status = await commands.getIcalSyncStatus();
    const oneHourAgo = Date.now() - 60 * 60 * 1000;
    if (!status.syncing && (!status.last_synced_at || status.last_synced_at < oneHourAgo)) {
      commands.triggerIcalSync();
    }
  });

  function markWelcomed(openSettings = false) {
    localStorage.setItem("recall:welcomed", "1");
    welcomed = true;
    if (openSettings) settingsOpen = true;
  }
</script>

{#if welcomeChecked && !welcomed}
  <Welcome onGetStarted={() => markWelcomed(true)} onSkip={() => markWelcomed(false)} />
{:else}
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
{/if}
