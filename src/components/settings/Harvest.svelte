<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands, type SettingsHarvest } from "../../bindings";
  import PasswordInput from "../ui/PasswordInput.svelte";
  import TestConnectionButton from "../ui/TestConnectionButton.svelte";
  import { t } from "$lib/i18n.svelte";

  const defaultSettings: SettingsHarvest = {
    enabled: false,
    access_token: "",
    account_id: "",
  };

  let settings = $state<SettingsHarvest>(defaultSettings);
  let enabled = $state(false);
  let accessToken = $state("");
  let accountId = $state("");

  onMount(() => {
    getSettings();
  });

  async function getSettings() {
    settings = (await commands.getSettingsHarvest()) ?? defaultSettings;
    enabled = settings.enabled;
    accessToken = settings.access_token ?? "";
    accountId = settings.account_id ?? "";
  }

  async function persist(partial: Partial<SettingsHarvest>) {
    const next: SettingsHarvest = { ...settings, ...partial };
    const result = await commands.setSettingsHarvest(next);
    if (result.status === "error") {
      toast.error(t("settings.harvest.error_save"), { richColors: true });
      return false;
    }
    settings = next;
    return true;
  }

  async function toggleEnabled(checked: boolean) {
    const original = settings.enabled;
    enabled = checked;
    const ok = await persist({ enabled: checked });
    if (!ok) enabled = original;
  }

  async function saveAccessToken() {
    const original = settings.access_token ?? "";
    accessToken = accessToken.trim();
    const ok = await persist({ access_token: accessToken });
    if (!ok) {
      accessToken = original;
      toast.error(t("settings.harvest.error_token"), { richColors: true });
    } else {
      toast.success(t("settings.harvest.saved_token"));
    }
  }

  async function saveAccountId() {
    const original = settings.account_id ?? "";
    accountId = accountId.trim();
    const ok = await persist({ account_id: accountId });
    if (!ok) {
      accountId = original;
      toast.error(t("settings.harvest.error_account_id"), { richColors: true });
    } else {
      toast.success(t("settings.harvest.saved_account_id"));
    }
  }
</script>

<fieldset class="relative border-2 p-4 mt-6">
  <legend>{t("settings.harvest.legend")}</legend>

  <div class="flex items-center gap-2 mb-4">
    <Checkbox
      id="harvest-enabled"
      checked={enabled}
      onCheckedChange={(v) => toggleEnabled(v === true)}
    />
    <Label for="harvest-enabled">{t("settings.harvest.enable")}</Label>
  </div>

  {#if enabled}
    <PasswordInput
      bind:password={accessToken}
      saveAction={saveAccessToken}
      label={t("settings.harvest.token")}
      placeholder={t("settings.harvest.token_placeholder")}
      inputId="harvest-access-token"
      description={t("settings.harvest.token_description")}
    />

    <PasswordInput
      bind:password={accountId}
      saveAction={saveAccountId}
      label={t("settings.harvest.account_id")}
      placeholder={t("settings.harvest.account_id_placeholder")}
      inputId="harvest-account-id"
      description={t("settings.harvest.account_id_description")}
    />

    <TestConnectionButton test={commands.testSettingsHarvest} />
  {/if}
</fieldset>
