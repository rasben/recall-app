<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { Button } from "$lib/components/ui/button/index.js";
  import LanguageSwitcher from "./ui/LanguageSwitcher.svelte";
  import { t } from "$lib/i18n.svelte";

  let {
    onGetStarted
  }: {
    onGetStarted: () => void;
  } = $props();

  const sources = [
    { name: "Calendar", done: true },
    { name: "GitHub", done: true },
    { name: "Git", done: true },
    { name: "Jira", done: true },
    { name: "Zulip", done: true },
    { name: "Gmail", done: false },
    { name: "Google Drive", done: false },
  ];
</script>

<div
  class="flex min-h-screen items-center justify-center p-8"
  in:fade={{ duration: 260 }}
  out:fade={{ duration: 180 }}
>
  <div class="w-full max-w-md space-y-8">

    <div
      class="space-y-2"
      in:fly={{ y: 24, duration: 480, delay: 60, easing: quintOut }}
    >
      <h1 class="font-head font-bold text-6xl text-foreground text-outlined">RECALL</h1>
      <p class="font-head text-lg text-muted-foreground">{t("welcome.tagline")}</p>
    </div>

    <div
      class="space-y-3 border-l-2 border-border pl-4 text-sm leading-relaxed"
      in:fly={{ y: 24, duration: 480, delay: 140, easing: quintOut }}
    >
      <p>{t("welcome.description1")}</p>
      <p>
        {t("welcome.description2")} <strong>{t("welcome.description2.done")}</strong>
      </p>
    </div>

    <div
      class="space-y-3"
      in:fly={{ y: 24, duration: 480, delay: 220, easing: quintOut }}
    >
      <p class="text-xs font-head tracking-wide text-muted-foreground uppercase">{t("welcome.sources_label")}</p>
      <div class="flex flex-wrap gap-2">
        {#each sources as source}
          <span
            class="inline-flex items-center gap-1.5 border-2 px-2 py-0.5 text-xs font-head
              {source.done
                ? 'border-border bg-primary text-primary-foreground'
                : 'border-border/40 text-muted-foreground'}"
          >
            {source.name}
            {#if !source.done}
              <span class="text-[0.6rem] opacity-60">{t("welcome.planned")}</span>
            {/if}
          </span>
        {/each}
      </div>
    </div>

    <div
      class="space-y-3 pt-2"
      in:fly={{ y: 24, duration: 480, delay: 300, easing: quintOut }}
    >
      <Button
        class="w-full shadow-sm"
        onclick={onGetStarted}
      >
        {t("welcome.cta_setup")}
      </Button>
    </div>

    <div in:fly={{ y: 24, duration: 480, delay: 360, easing: quintOut }}>
      <LanguageSwitcher />
    </div>

  </div>
</div>
