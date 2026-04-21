<script lang="ts">
    import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
    import { toast } from "svelte-sonner";
    import { applyTheme } from "$lib/theme";
    import { commands } from "../../bindings";
    import type { SettingsUi } from "../../bindings";
    import { onMount } from "svelte";

    let settings = $state<SettingsUi>({ theme: "system" });

    onMount(async () => {
        settings = await commands.getSettingsUi() ?? { theme: "system" };
    });

    async function setTheme(value: string) {
        settings.theme = value;
        applyTheme(value);
        const result = await commands.setSettingsUi(settings);
        if (result.status === "error") {
            toast.error("Failed to save theme");
        } else {
            toast.success("Theme updated!");
        }
    }
</script>

<fieldset class="border-2 p-4">
    <legend>Theme</legend>
    <ToggleGroup.Root
        variant="outlined"
        type="single"
        class="w-full"
        id="theme"
        value={settings.theme}
        onValueChange={setTheme}
    >
        <ToggleGroup.Item value="light">Light</ToggleGroup.Item>
        <ToggleGroup.Item value="dark">Dark</ToggleGroup.Item>
        <ToggleGroup.Item value="system">System</ToggleGroup.Item>
    </ToggleGroup.Root>
</fieldset>
