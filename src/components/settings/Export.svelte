<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands } from "../../bindings";
  import { t } from "$lib/i18n.svelte";
  import { defaultExportPrompt, isDefaultExportPrompt } from "$lib/export-prompt";

  /** Folded by default; the export popover's "Edit prompt" link opens it. */
  let { open = $bindable(false) }: { open?: boolean } = $props();

  let prompt = $state("");
  let loaded = $state(false);

  onMount(async () => {
    const settings = await commands.getSettingsExport();
    const stored = settings?.prompt ?? "";
    // Never show a blank box: fall back to the active-language default when
    // nothing custom is stored (or the stored value is itself a default).
    prompt = stored.trim() === "" || isDefaultExportPrompt(stored) ? defaultExportPrompt() : stored;
    loaded = true;
  });

  // While the textarea still holds a default, keep it synced to the active
  // language so switching language swaps in that language's default text.
  // Once the user types something custom this stops firing.
  $effect(() => {
    if (!loaded) return;
    if (isDefaultExportPrompt(prompt)) {
      const next = defaultExportPrompt();
      if (prompt !== next) prompt = next;
    }
  });

  let isDefault = $derived(prompt.trim() === "" || isDefaultExportPrompt(prompt));
  let firstLine = $derived(prompt.trim().split("\n")[0] ?? "");

  async function save() {
    // Persist an empty string whenever the box still holds a default, so the
    // stored value stays language-following and survives future edits to the
    // default text (a stored default copy would otherwise become a stale pin).
    const toStore = isDefault ? "" : prompt;
    const result = await commands.setSettingsExport({ prompt: toStore });
    if (result.status === "error") {
      toast.error(t("settings.export.error"), { richColors: true });
    } else {
      toast.success(t("settings.export.saved"));
    }
  }

  async function revert() {
    prompt = defaultExportPrompt();
    await save();
  }
</script>

<!-- min-w-0: a fieldset otherwise refuses to shrink below its content, and the
     one-line preview below is as wide as the whole prompt. -->
<fieldset class="mt-6 min-w-0 border-2 p-4">
  <legend>{t("settings.export.title")}</legend>
  <p class="mb-3 text-sm text-muted-foreground">{t("settings.export.description")}</p>

  {#if open}
    <Textarea bind:value={prompt} placeholder={defaultExportPrompt()} rows={10} class="mb-3 text-sm" />

    <div class="flex flex-wrap gap-2">
      <Button size="sm" onclick={save}>{t("settings.export.save")}</Button>
      <Button size="sm" variant="outline" disabled={isDefault} onclick={revert}>
        {t("settings.export.revert")}
      </Button>
      <Button size="sm" variant="ghost" onclick={() => (open = false)}>{t("settings.export.collapse")}</Button>
    </div>
  {:else}
    <button
      type="button"
      class="flex w-full items-center gap-3 border-2 border-dashed px-3 py-2 text-left text-sm hover:bg-muted"
      onclick={() => (open = true)}
    >
      <span class="shrink-0 font-head">{t(isDefault ? "settings.export.using_default" : "settings.export.using_custom")}</span>
      <span class="min-w-0 flex-1 truncate text-muted-foreground">{firstLine}</span>
      <ChevronDown class="size-4 shrink-0" />
    </button>
  {/if}
</fieldset>
