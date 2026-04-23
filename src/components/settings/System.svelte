<script lang="ts">
    import { onMount } from "svelte";
    import { toast } from "svelte-sonner";
    import { commands } from "../../bindings";
    import { Button } from "$lib/components/ui/button/index.js";
    import { t } from "$lib/i18n.svelte";

    let clearing = $state(false);
    let cachedDays = $state<number | null>(null);
    let bytes = $state<number | null>(null);
    let { onShowWelcome }: { onShowWelcome?: () => void } = $props();

    function formatBytes(b: number): string {
        if (b < 1024) return `${b} B`;
        if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
        return `${(b / (1024 * 1024)).toFixed(1)} MB`;
    }

    async function loadSize() {
        const result = await commands.getCacheSize();
        if (result.status === "ok") {
            bytes = result.data.bytes;
            cachedDays = result.data.cached_days;
        }
    }

    async function clearCaches() {
        clearing = true;
        const result = await commands.clearAllCaches();
        clearing = false;

        if (result.status === "error") {
            toast.error(t("settings.cache.error"));
        } else {
            toast.success(t("settings.cache.cleared"));
            await loadSize();
        }
    }

    onMount(loadSize);
</script>

<fieldset class="border-2 p-4 mt-6">
    <legend>{t("settings.system.legend")}</legend>
    <div class="flex justify-center">
        <div>
            <Button variant="outline" disabled={clearing} onclick={clearCaches}>
                {clearing ? t("settings.cache.clearing") : t("settings.cache.clear")}
            </Button>
            {#if cachedDays !== null && bytes !== null}
                <div class="text-xs text-muted-foreground mt-4">
                    {cachedDays} {cachedDays === 1 ? t("settings.cache.day") : t("settings.cache.days")} &middot; {formatBytes(Number(bytes))} {t("settings.cache.on_disk")}
                </div>
            {/if}
        </div>
        <figure class="mx-8 w-[2px] bg-muted"></figure>
        <div>
            <Button variant="outline" onclick={onShowWelcome}>
                {t("settings.welcome.show")}
            </Button>
            <div class="text-xs text-muted-foreground mt-4">
                {t("settings.welcome.description")}
            </div>
        </div>
    </div>


</fieldset>
