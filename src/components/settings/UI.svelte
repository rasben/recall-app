<script lang="ts">
    import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
    import { toast } from "svelte-sonner";
    import { applyTheme } from "$lib/theme";
    import { commands } from "../../bindings";
    import type { SettingsUi } from "../../bindings";
    import { onMount } from "svelte";
    import { t } from "$lib/i18n.svelte";
    import LanguageSwitcher from "../ui/LanguageSwitcher.svelte";

    let settings = $state<SettingsUi>({ theme: "system" });

    onMount(async () => {
        settings = await commands.getSettingsUi() ?? { theme: "system" };
    });

    async function setTheme(value: string) {
        settings.theme = value;
        applyTheme(value);
        const result = await commands.setSettingsUi(settings);
        if (result.status === "error") {
            toast.error(t("settings.theme.error"));
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
