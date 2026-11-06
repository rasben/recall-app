<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands, type CliInstallStatus } from "../../bindings";
  import { t } from "$lib/i18n.svelte";
  import { cliUnavailableKey } from "$lib/cli-availability";

  let status = $state<CliInstallStatus | null>(null);
  let busy = $state(false);

  let unavailable = $derived(status ? cliUnavailableKey(status.availability) : null);

  async function loadStatus() {
    status = await commands.getCliInstallStatus();
  }

  async function run(action: "install" | "uninstall") {
    busy = true;
    const result = action === "install" ? await commands.installCli() : await commands.uninstallCli();
    busy = false;
    if (result.status === "error") {
      toast.error(t("settings.cli.error"), { description: result.error, richColors: true });
    } else {
      toast.success(t(action === "install" ? "settings.cli.installed_toast" : "settings.cli.uninstalled_toast"));
    }
    await loadStatus();
  }

  onMount(loadStatus);
</script>

<fieldset class="border-2 p-4 mt-6">
  <legend>{t("settings.cli.title")}</legend>
  <p class="mb-3 text-sm text-muted-foreground">{t("settings.cli.description")}</p>

  {#if status && unavailable}
    <p class="text-sm">{t(unavailable)}</p>
  {:else if status?.availability === "OnPath"}
    <p class="text-sm">{t("settings.cli.on_path")}</p>
  {:else if status?.availability === "Manual"}
    <p class="text-sm">{t("settings.cli.manual")}</p>
    <code class="mt-2 block break-all text-xs">{status.cli_path}</code>
  {:else if status?.availability === "Linkable"}
    {#if status.link === "Installed"}
      <p class="mb-3 text-sm">
        {t("settings.cli.linked")} <code class="break-all text-xs">{status.link_path}</code>
      </p>
      <Button size="sm" variant="outline" disabled={busy} onclick={() => run("uninstall")}>
        {t("settings.cli.uninstall")}
      </Button>
    {:else if status.link === "Foreign"}
      <p class="text-sm">
        <code class="break-all text-xs">{status.link_path}</code>
        {t("settings.cli.foreign")}
      </p>
    {:else}
      {#if status.link === "Stale"}
        <p class="mb-3 text-sm">{t("settings.cli.stale")}</p>
      {/if}
      <p class="mb-3 text-sm">
        {t("settings.cli.install_hint")} <code class="break-all text-xs">{status.link_path}</code>
      </p>
      <Button size="sm" disabled={busy} onclick={() => run("install")}>
        {t(status.link === "Stale" ? "settings.cli.update" : "settings.cli.install")}
      </Button>
    {/if}
  {/if}
</fieldset>
