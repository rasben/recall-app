<script lang="ts">
	import './layout.css';
    import { Toaster } from "$lib/components/ui/sonner/index.js";
    import { commands } from "../bindings";
    import { applyTheme } from "$lib/theme";
    import { onMount } from "svelte";

    const { children } = $props();

    onMount(async () => {
        const settingsUI = await commands.getSettingsUi();
        applyTheme(settingsUI?.theme ?? "system");
    });
</script>

<div>
    <Toaster />

    {@render children?.()}
</div>
