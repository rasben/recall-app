<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands, type SettingsZulip } from "../../bindings";
  import PasswordInput from "../ui/PasswordInput.svelte";
  import { t } from "$lib/i18n.svelte";

  const defaultRealmUrl = "https://reload.zulipchat.com";

  const defaultSettings: SettingsZulip = {
    enabled: false,
    realm_url: defaultRealmUrl,
    email: "",
    api_key: "",
  };

  let settings = $state<SettingsZulip>(defaultSettings);
  let enabled = $state(false);
  let realmUrl = $state(defaultRealmUrl);
  let email = $state("");
  let apiKey = $state("");

  onMount(() => {
    getSettings();
  });

  async function getSettings() {
    settings = (await commands.getSettingsZulip()) ?? defaultSettings;
    enabled = settings.enabled;
    realmUrl = settings.realm_url || defaultRealmUrl;
    email = settings.email ?? "";
    apiKey = settings.api_key ?? "";
  }

  async function persist(partial: Partial<SettingsZulip>) {
    const next: SettingsZulip = { ...settings, ...partial };
    const result = await commands.setSettingsZulip(next);
    if (result.status === "error") {
      toast.error(t("settings.zulip.error_save"));
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

  async function saveRealmUrl() {
    const original = settings.realm_url ?? "";
    realmUrl = realmUrl.trim().replace(/\/+$/, "");
    const ok = await persist({ realm_url: realmUrl });
    if (!ok) {
      realmUrl = original;
    } else {
      toast.success(t("settings.zulip.saved_url"));
    }
  }

  async function saveEmail() {
    const original = settings.email ?? "";
    email = email.trim();
    const ok = await persist({ email });
    if (!ok) {
      email = original;
    } else {
      toast.success(t("settings.zulip.saved_email"));
    }
  }

  async function saveApiKey() {
    const original = settings.api_key ?? "";
    const ok = await persist({ api_key: apiKey });
    if (!ok) {
      apiKey = original;
      toast.error(t("settings.zulip.error_api_key"));
    } else {
      toast.success(t("settings.zulip.saved_api_key"));
    }
  }
</script>

<fieldset class="border-2 p-4 mt-6">
  <legend>{t("settings.zulip.legend")}</legend>

  <div class="flex items-center gap-2 mb-4">
    <Checkbox
      id="zulip-enabled"
      checked={enabled}
      onCheckedChange={(v) => toggleEnabled(v === true)}
    />
    <Label for="zulip-enabled">{t("settings.zulip.enable")}</Label>
  </div>

  {#if enabled}
    <Label for="zulip-email" class="mb-2">{t("settings.zulip.email")}</Label>
    <Input
      id="zulip-email"
      type="email"
      autocomplete="username"
      class="mb-4"
      placeholder="you@example.com"
      bind:value={email}
      onblur={saveEmail}
    />

    <PasswordInput
      bind:password={apiKey}
      saveAction={saveApiKey}
      label={t("settings.zulip.api_key")}
      placeholder={t("settings.zulip.api_key_placeholder")}
      inputId="zulip-api-key"
      description={t("settings.zulip.token_description")}
    />

    <Label for="zulip-realm-url" class="mb-2">{t("settings.zulip.realm_url")}</Label>
    <Input
            id="zulip-realm-url"
            type="url"
            placeholder={defaultRealmUrl}
            bind:value={realmUrl}
            onblur={saveRealmUrl}
    />
  {/if}
</fieldset>
