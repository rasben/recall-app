<script lang="ts">
  import { fade } from "svelte/transition";
  import { Button } from "$lib/components/ui/button/index.js";
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import CloseIcon from "@lucide/svelte/icons/x";
  import Settings from "../components/settings/Settings.svelte";
  import Main from "../components/Main.svelte";

  let settingsOpen = $state(false);
</script>

<Button
  variant={settingsOpen ? "secondary" : "outline"}
  size="icon"
  class="fixed top-8 right-8 z-50"
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
