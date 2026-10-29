<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands, type ClaudeSetupStatus } from "../../bindings";
  import { t } from "$lib/i18n.svelte";
  import { cliUnavailableKey } from "$lib/cli-availability";

  let status = $state<ClaudeSetupStatus | null>(null);
  let time = $state("08:30");
  let busy = $state(false);

  let unavailable = $derived(status ? cliUnavailableKey(status.availability) : null);

  async function loadStatus() {
    const result = await commands.getClaudeSetupStatus();
    if (result.status === "ok") status = result.data;
  }

  async function setup() {
    busy = true;
    const result = await commands.setupClaude(time);
    busy = false;
    if (result.status === "error") {
      toast.error(t("settings.claude.error"), { description: result.error, richColors: true });
    } else {
      toast.success(t("settings.claude.done"));
    }
    await loadStatus();
  }

  onMount(loadStatus);
</script>

<fieldset class="border-2 p-4 mt-6">
  <legend>{t("settings.claude.title")}</legend>
  <p class="mb-3 text-sm text-muted-foreground">{t("settings.claude.description")}</p>

  {#if status && unavailable}
    <p class="text-sm">{t(unavailable)}</p>
  {:else if status?.command}
    <ol class="mb-4 list-decimal space-y-2 pl-5 text-sm">
      <li>
        {t("settings.claude.step_skill")}
        <code class="break-all text-xs">{status.skill_path}</code>
        {#if status.skill_installed}<span class="text-muted-foreground">({t("settings.claude.installed")})</span>{/if}
      </li>
      <li>
        {t("settings.claude.step_rule")}
        <code class="break-all text-xs">{status.allow_rule}</code>
        → <code class="break-all text-xs">{status.settings_path}</code>
        {#if status.rule_installed}<span class="text-muted-foreground">({t("settings.claude.installed")})</span>{/if}
      </li>
      <li>{t("settings.claude.step_open")}</li>
    </ol>

    <div class="mb-3 flex items-center gap-2">
      <Label for="claude-time">{t("settings.claude.time_label")}</Label>
      <Input id="claude-time" type="time" bind:value={time} class="w-32" />
    </div>

    <Button size="sm" disabled={busy || !time} onclick={setup}>{t("settings.claude.setup")}</Button>
    <p class="mt-3 text-xs text-muted-foreground">{t("settings.claude.note")}</p>
  {/if}
</fieldset>
