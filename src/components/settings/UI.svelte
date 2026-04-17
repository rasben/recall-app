<script lang="ts">
    import { Label } from "$lib/components/ui/label/index.js";
    import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
    import { toast } from "svelte-sonner";
    import { applyTheme } from "$lib/theme";

    import { commands } from "../../bindings";
    import type { SettingsUi } from "../../bindings"
    import {onMount} from "svelte";

    let theme = $state("system");
    let defaultSettings = {
        theme: theme
    };

    let settings = $state<SettingsUi>(defaultSettings);

    onMount(() => {
        getSettings();
    });

    async function getSettings() {
        settings = await commands.getSettingsUi() ?? defaultSettings;
        theme = settings?.theme;
    }

    async function setTheme(value: string) {
        settings.theme = value;
        applyTheme(value);

        const result = await commands.setSettingsUi(settings);

        if (result.status === "error") {
            toast.error("Failed to save theme");
        }
        else {
            toast.success("Theme updated!");
        }

    }
</script>

<fieldset class="border-2 p-4">
    <legend class="mb-2">Theme</legend>
    <ToggleGroup.Root
            variant="outlined"
            type="single"
            class="w-full"
            id="theme"
            value={theme}
            onValueChange={setTheme}
    >
        <ToggleGroup.Item value="light">Light</ToggleGroup.Item>
        <ToggleGroup.Item value="dark">Dark</ToggleGroup.Item>
        <ToggleGroup.Item value="system">System</ToggleGroup.Item>
    </ToggleGroup.Root>
</fieldset>
