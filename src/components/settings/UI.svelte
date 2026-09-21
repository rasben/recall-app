<script lang="ts">
    import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
    import { toast } from "svelte-sonner";
    import { applyTheme } from "$lib/theme";
    import { commands } from "../../bindings";
    import type { SettingsUi } from "../../bindings";
    import { onMount } from "svelte";
    import { t } from "$lib/i18n.svelte";
    import LanguageSwitcher from "../ui/LanguageSwitcher.svelte";
    import { DEFAULT_SETTINGS_UI } from "$lib/view-state";

    let settings = $state<SettingsUi>({ ...DEFAULT_SETTINGS_UI });

    onMount(async () => {
        settings = await commands.getSettingsUi() ?? { ...DEFAULT_SETTINGS_UI };
    });

    async function setTheme(value: string) {
        settings.theme = value;
        applyTheme(value);
        // Re-read before writing: the timeline view also persists its own
        // fields (grouping mode, hidden sources) into this document.
        const current = await commands.getSettingsUi();
        settings = { ...(current ?? settings), theme: value };
        const result = await commands.setSettingsUi(settings);
        if (result.status === "error") {
            toast.error(t("settings.theme.error"), {richColors: true});
        } else {
            toast.success(t("settings.theme.saved"));
        }
    }
</script>

<fieldset class="border-2 p-4">
    <legend>{t("settings.theme.ui")}</legend>
    <LanguageSwitcher />

    <hr class="my-4 opacity-30 max-w-[80%] mx-auto" />

    <ToggleGroup.Root
        variant="outlined"
        type="single"
        class="w-full"
        id="theme"
        value={settings.theme}
        onValueChange={setTheme}
    >
        <ToggleGroup.Item value="light">{t("settings.theme.light")}</ToggleGroup.Item>
        <ToggleGroup.Item value="dark">{t("settings.theme.dark")}</ToggleGroup.Item>
        <ToggleGroup.Item value="system">{t("settings.theme.system")}</ToggleGroup.Item>
    </ToggleGroup.Root>
</fieldset>
