<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import CheckIcon from "@lucide/svelte/icons/circle-check";
  import AlertIcon from "@lucide/svelte/icons/triangle-alert";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands, type ClaudeSetupStatus, type CliInstallStatus } from "../../bindings";
  import { i18n, t } from "$lib/i18n.svelte";
  import { cliUnavailableKey } from "$lib/cli-availability";

  /** `onDone` fires after a successful setup (the setup guide moves on). */
  let { onDone }: { onDone?: () => void } = $props();

  let status = $state<ClaudeSetupStatus | null>(null);
  let cli = $state<CliInstallStatus | null>(null);
  let time = $state("08:30");
  let editing = $state(false);
  let installCommand = $state(true);
  let busy = $state(false);

  let unavailable = $derived(status ? cliUnavailableKey(status.availability) : null);
  let complete = $derived(!!status && status.skill_installed && status.rule_installed);
  /** Set up before, but this version of Recall would write something else. */
  let outdated = $derived(!!status?.scheduled_time && !complete);
  let showForm = $derived(editing || !status?.scheduled_time);
  /** The terminal command is separate from Claude (which uses the full path),
   *  but setting both up together saves a trip to the System tab. */
  let canInstallCommand = $derived(
    cli?.availability === "Linkable" && (cli.link === "NotInstalled" || cli.link === "Stale"),
  );

  async function loadStatus() {
    const [result, cliStatus] = await Promise.all([
      commands.getClaudeSetupStatus(i18n.lang),
      commands.getCliInstallStatus(),
    ]);
    if (result.status === "ok") {
      status = result.data;
      if (!editing && status.scheduled_time) time = status.scheduled_time;
    }
    cli = cliStatus;
  }

  async function setup() {
    busy = true;
    if (showForm && canInstallCommand && installCommand) {
      const installed = await commands.installCli();
      if (installed.status === "error") {
        toast.error(t("settings.cli.error"), { description: installed.error, richColors: true });
      }
    }
    const result = await commands.setupClaude(time, i18n.lang);
    busy = false;
    if (result.status === "error") {
      toast.error(t("settings.claude.error"), { description: result.error, richColors: true });
    } else {
      toast.success(t("settings.claude.done"));
      editing = false;
      onDone?.();
    }
    await loadStatus();
  }

  onMount(loadStatus);
</script>

<fieldset class="border-2 p-4 mt-6">
  <legend>{t("settings.claude.title")}</legend>

  {#if status && unavailable}
    <p class="text-sm">{t(unavailable)}</p>
  {:else if status}
    {#if !showForm && complete}
      <div class="flex items-start gap-3">
        <CheckIcon class="mt-0.5 size-5 shrink-0 text-green-600 dark:text-green-400" />
        <div>
          <p class="font-head">{t("settings.claude.ready", { time })}</p>
          <p class="mt-1 text-sm text-muted-foreground">{t("settings.claude.ready_hint")}</p>
        </div>
      </div>
      <div class="mt-4 flex flex-wrap gap-2">
        <Button size="sm" variant="outline" onclick={() => (editing = true)}>{t("settings.claude.change_time")}</Button>
        <Button size="sm" variant="outline" disabled={busy} onclick={setup}>{t("settings.claude.again")}</Button>
      </div>
    {:else if !showForm && outdated}
      <div class="flex items-start gap-3">
        <AlertIcon class="mt-0.5 size-5 shrink-0 text-amber-600 dark:text-amber-400" />
        <p class="text-sm">{t("settings.claude.outdated")}</p>
      </div>
      <Button size="sm" class="mt-4" disabled={busy} onclick={setup}>{t("settings.claude.update")}</Button>
    {:else}
      <p class="mb-4 text-sm text-muted-foreground">{t("settings.claude.description")}</p>

      <div class="mb-3 flex items-center gap-2">
        <Label for="claude-time">{t("settings.claude.time_label")}</Label>
        <Input id="claude-time" type="time" bind:value={time} class="w-32" />
      </div>

      {#if canInstallCommand}
        <div class="mb-4 flex items-center gap-2">
          <Checkbox id="claude-install-cli" bind:checked={installCommand} />
          <Label for="claude-install-cli" class="text-sm font-normal">{t("settings.claude.install_cli")}</Label>
        </div>
      {/if}

      <div class="flex flex-wrap gap-2">
        <Button size="sm" disabled={busy || !time} onclick={setup}>
          {t(status.scheduled_time ? "settings.claude.save_time" : "settings.claude.setup")}
        </Button>
        {#if editing}
          <Button size="sm" variant="outline" onclick={() => (editing = false)}>{t("settings.claude.cancel")}</Button>
        {/if}
      </div>
      <p class="mt-3 text-xs text-muted-foreground">{t("settings.claude.note")}</p>
    {/if}

    <details class="mt-4 text-sm">
      <summary class="cursor-pointer text-muted-foreground">{t("settings.claude.details")}</summary>
      <ol class="mt-2 list-decimal space-y-2 pl-5">
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
    </details>
  {/if}
</fieldset>
