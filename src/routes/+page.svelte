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
  import { t } from "$lib/i18n.svelte";

  let settingsOpen = $state(false);
  let welcomed = $state(true); // optimistic: don't flash welcome on repeat visits
  let welcomeChecked = $state(false);

  onMount(async () => {
    welcomed = !!localStorage.getItem("recall:welcomed");
    welcomeChecked = true;

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
  <Welcome onGetStarted={() => markWelcomed(true)} />
{:else}
  <Button
    variant={settingsOpen ? "secondary" : "outline"}
    size="icon"
    class="absolute top-8 right-8 z-50"
    aria-pressed={settingsOpen}
    aria-label={settingsOpen ? t("page.close_settings") : t("page.open_settings")}
    onclick={() => (settingsOpen = !settingsOpen)}
  >
    {#if settingsOpen}
      <CloseIcon />
    {:else}
      <SettingsIcon />
    {/if}
  </Button>

  <main class="relative min-h-screen">
    {#if settingsOpen}
      <div class="absolute inset-0 flex flex-col p-8 mb-8" in:fade={{ duration: 220 }} out:fade={{ duration: 160 }}>
        <Settings />
        <Button
            class="w-full max-h-none h-[50px] shadow-sm mt-8"
            aria-label={settingsOpen ? t("page.close_settings") : t("page.open_settings")}
            onclick={() => (settingsOpen = false)}>
          {t("settings.go_to_data")}
        </Button>
        <div class="pt-8"></div>
      </div>
    {:else}
      <div class="absolute inset-0 flex flex-col p-8 mb-8" in:fade={{ duration: 220 }} out:fade={{ duration: 160 }}>
        <Main />
      </div>
    {/if}
  </main>
{/if}
