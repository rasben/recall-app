<script lang="ts">
  import { fade } from "svelte/transition";
  import { Button } from "$lib/components/ui/button/index.js";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import Welcome from "./Welcome.svelte";
  import TimelineEvent from "./TimelineEvent.svelte";
  import SettingsGit from "./settings/Git.svelte";
  import SettingsGitHub from "./settings/GitHub.svelte";
  import SettingsJira from "./settings/Jira.svelte";
  import SettingsZulip from "./settings/Zulip.svelte";
  import SettingsCalendar from "./settings/Calendar.svelte";
  import SettingsClaude from "./settings/Claude.svelte";
  import { commands, type ExportResult } from "../bindings";
  import { GUIDE_STEPS, countBySource, lastWorkdayIso, type GuideStep } from "$lib/setup-guide";
  import { SOURCE_LABELS, formatDayHeading, todayIso } from "$lib/timeline";
  import { langLocale, t } from "$lib/i18n.svelte";
  import { track } from "$lib/telemetry";

  /** Called when the user finishes or leaves the guide. */
  let { onFinish }: { onFinish: () => void } = $props();

  let step = $state<GuideStep>("welcome");
  let preview = $state<ExportResult | null>(null);
  let previewError = $state<string | null>(null);

  const previewDay = lastWorkdayIso(todayIso());
  let previewEvents = $derived(preview?.days.flatMap((d) => d.events) ?? []);

  const STEP_LABELS = {
    sources: "guide.step.sources",
    preview: "guide.step.preview",
    claude: "guide.step.claude",
  } as const;

  function go(next: GuideStep) {
    step = next;
    track(`guide.${next}`);
    if (next === "preview") loadPreview();
    window.scrollTo({ top: 0 });
  }

  function finish(how: "done" | "skip") {
    track(`guide.${how}`);
    onFinish();
  }

  async function loadPreview() {
    preview = null;
    previewError = null;
    const result = await commands.exportTimelineForRange(previewDay, previewDay);
    if (result.status === "ok") preview = result.data;
    else previewError = result.error;
  }
</script>

{#if step === "welcome"}
  <Welcome
    onGetStarted={() => {
      track("welcome.get_started");
      go("sources");
    }}
  />
{:else}
  <div class="mx-auto flex min-h-screen w-full max-w-2xl flex-col p-8" in:fade={{ duration: 220 }}>
    <ol class="mb-8 flex flex-wrap items-center gap-2 text-sm">
      {#each GUIDE_STEPS as s, i (s)}
        {@const current = GUIDE_STEPS.indexOf(step as (typeof GUIDE_STEPS)[number])}
        <li class="flex items-center gap-2">
          <span
            class="inline-flex size-7 items-center justify-center border-2 font-head
              {i === current ? 'bg-primary text-primary-foreground' : i < current ? 'bg-accent' : 'text-muted-foreground'}"
          >
            {i + 1}
          </span>
          <span class={i === current ? "font-head" : "text-muted-foreground"}>{t(STEP_LABELS[s])}</span>
          {#if i < GUIDE_STEPS.length - 1}<span class="mx-1 text-muted-foreground">—</span>{/if}
        </li>
      {/each}
    </ol>

    {#if step === "sources"}
      <h1 class="font-head text-3xl">{t("guide.sources.title")}</h1>
      <p class="mt-2 text-muted-foreground">{t("guide.sources.description")}</p>
      <div class="[&>*:first-child]:mt-4">
        <SettingsCalendar />
        <SettingsGitHub />
        <SettingsGit />
        <SettingsJira />
        <SettingsZulip />
      </div>
    {:else if step === "preview"}
      <h1 class="font-head text-3xl">{t("guide.preview.title")}</h1>
      <p class="mt-2 text-muted-foreground">
        {t("guide.preview.description", { day: formatDayHeading(previewDay, langLocale()) })}
      </p>

      <div class="mt-6">
        {#if previewError}
          <p class="text-sm text-destructive">{previewError}</p>
        {:else if !preview}
          <p class="text-sm text-muted-foreground">{t("guide.preview.loading")}</p>
        {:else}
          {#if preview.errors.length > 0}
            <div class="mb-4 border-2 border-amber-500 p-3 text-sm">
              <p class="font-head">{t("guide.preview.errors")}</p>
              <ul class="mt-1 list-disc pl-5">
                {#each preview.errors as e (e.source)}
                  <li><strong>{e.source}</strong>: {e.error}</li>
                {/each}
              </ul>
            </div>
          {/if}

          {#if previewEvents.length === 0}
            <p class="border-2 border-dashed p-6 text-center text-sm text-muted-foreground">{t("guide.preview.empty")}</p>
          {:else}
            <div class="mb-4 flex flex-wrap gap-2">
              {#each countBySource(previewEvents) as [source, count] (source)}
                <span class="border-2 bg-accent px-2 py-0.5 font-head text-xs">{SOURCE_LABELS[source]} · {count}</span>
              {/each}
            </div>
            <div class="max-h-[50vh] space-y-2 overflow-y-auto pr-1">
              {#each previewEvents as event (event.id)}
                <TimelineEvent {event} />
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    {:else if step === "claude"}
      <h1 class="font-head text-3xl">{t("guide.claude.title")}</h1>
      <p class="mt-2 text-muted-foreground">{t("guide.claude.description")}</p>
      <SettingsClaude />
    {/if}

    <div class="mt-auto flex items-center gap-2 pt-10">
      <Button
        variant="outline"
        onclick={() => go(step === "sources" ? "welcome" : GUIDE_STEPS[GUIDE_STEPS.indexOf(step as (typeof GUIDE_STEPS)[number]) - 1])}
      >
        <ArrowLeft class="size-4" />
        {t("guide.back")}
      </Button>
      <div class="flex-1"></div>
      {#if step === "claude"}
        <Button variant="ghost" onclick={() => finish("skip")}>{t("guide.skip")}</Button>
        <Button onclick={() => finish("done")}>{t("guide.finish")}</Button>
      {:else}
        <Button onclick={() => go(step === "sources" ? "preview" : "claude")}>
          {t("guide.next")}
          <ArrowRight class="size-4" />
        </Button>
      {/if}
    </div>
  </div>
{/if}
