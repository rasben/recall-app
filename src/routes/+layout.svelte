<script lang="ts">
	import '@fontsource/archivo-black/400.css';
	import '@fontsource/space-grotesk/300.css';
	import '@fontsource/space-grotesk/400.css';
	import '@fontsource/space-grotesk/500.css';
	import '@fontsource/space-grotesk/600.css';
	import '@fontsource/space-grotesk/700.css';
	import './tailwind-base.css';
	import './layout.scss';
    import { Toaster } from "$lib/components/ui/sonner/index.js";
    import { commands } from "../bindings";
    import { applyTheme } from "$lib/theme";
    import { onMount } from "svelte";
    // Import i18n so the reactive lang state is initialized at app boot
    import "$lib/i18n.svelte";

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
