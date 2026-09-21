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
    import { loadViewState, navState, persistViewState } from "$lib/nav-state.svelte";
    import { viewStatePatch } from "$lib/view-state";
    import { onMount } from "svelte";
    // Import i18n so the reactive lang state is initialized at app boot
    import "$lib/i18n.svelte";

    const { children } = $props();

    // Grouping mode and hidden sources live in settings_ui next to the theme.
    // Load them once here; the effect below writes them back whenever they change.
    let viewStateLoaded = $state(false);
    let lastPersisted = "";

    onMount(async () => {
        const settingsUI = await commands.getSettingsUi();
        applyTheme(settingsUI?.theme ?? "system");
        loadViewState(settingsUI);
        lastPersisted = JSON.stringify(viewStatePatch(navState.groupMode, navState.hiddenSources));
        viewStateLoaded = true;
    });

    $effect(() => {
        if (!viewStateLoaded) return;
        const snapshot = JSON.stringify(viewStatePatch(navState.groupMode, navState.hiddenSources));
        if (snapshot === lastPersisted) return;
        lastPersisted = snapshot;
        persistViewState();
    });
</script>

<div>
    <Toaster />

    {@render children?.()}
</div>
