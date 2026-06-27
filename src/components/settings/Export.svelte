<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands } from "../../bindings";
  import { t } from "$lib/i18n.svelte";
  import { defaultExportPrompt, isDefaultExportPrompt } from "$lib/export-prompt";

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

<fieldset id="settings-export" class="border-2 p-4 mt-6">
  <legend>{t("settings.export.title")}</legend>
  <p class="mb-3 text-sm text-muted-foreground">{t("settings.export.description")}</p>

  <Textarea bind:value={prompt} placeholder={defaultExportPrompt()} rows={7} class="mb-3 text-sm" />

  <div class="flex gap-2">
    <Button size="sm" onclick={save}>{t("settings.export.save")}</Button>
    <Button size="sm" variant="outline" disabled={isDefault} onclick={revert}>
      {t("settings.export.revert")}
    </Button>
  </div>
</fieldset>
