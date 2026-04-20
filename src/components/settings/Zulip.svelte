<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { toast } from "svelte-sonner";
  import { onMount } from "svelte";
  import { commands, type SettingsZulip } from "../../bindings";
  import PasswordInput from "../ui/PasswordInput.svelte";

  const DEFAULT_REALM = "https://reload.zulipchat.com";
  const descriptionToken = `Find your API key in Zulip under
      <strong>Settings → Account &amp; privacy → API key</strong>.`

  const defaultSettings: SettingsZulip = {
    enabled: false,
    realm_url: DEFAULT_REALM,
    email: "",
    api_key: "",
  };

  let settings = $state<SettingsZulip>(defaultSettings);
  let enabled = $state(false);
  let realmUrl = $state(DEFAULT_REALM);
  let email = $state("");
  let apiKey = $state("");

  onMount(() => {
    getSettings();
  });

  async function getSettings() {
    settings = (await commands.getSettingsZulip()) ?? defaultSettings;
    enabled = settings.enabled;
    realmUrl = settings.realm_url || DEFAULT_REALM;
    email = settings.email ?? "";
    apiKey = settings.api_key ?? "";
  }

  async function persist(partial: Partial<SettingsZulip>) {
    const next: SettingsZulip = { ...settings, ...partial };
    const result = await commands.setSettingsZulip(next);
    if (result.status === "error") {
      toast.error("Could not save Zulip settings");
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
      toast.success("Zulip realm URL saved");
    }
  }

  async function saveEmail() {
    const original = settings.email ?? "";
    email = email.trim();
    const ok = await persist({ email });
    if (!ok) {
      email = original;
    } else {
      toast.success("Email saved");
    }
  }

  async function saveApiKey() {
    const original = settings.api_key ?? "";
    const ok = await persist({ api_key: apiKey });
    if (!ok) {
      apiKey = original;
      toast.error("Could not save API key");
    } else {
      toast.success("API key saved");
    }
  }
</script>

<fieldset class="border-2 p-4 mt-6">
  <legend>Zulip</legend>

  <div class="flex items-center gap-2 mb-4">
    <Checkbox
      id="zulip-enabled"
      checked={enabled}
      onCheckedChange={(v) => toggleEnabled(v === true)}
    />
    <Label for="zulip-enabled">Enable Zulip source</Label>
  </div>

  {#if enabled}
    <Label for="zulip-realm-url" class="mb-2">Realm URL</Label>
    <Input
      id="zulip-realm-url"
      type="url"
      class="mb-4"
      placeholder={DEFAULT_REALM}
      bind:value={realmUrl}
      onblur={saveRealmUrl}
    />

    <Label for="zulip-email" class="mb-2">Zulip account email</Label>
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
      label="API key"
      placeholder="Your Zulip API key…"
      inputId="zulip-api-key"
      description={descriptionToken}
    />

  {/if}
</fieldset>
