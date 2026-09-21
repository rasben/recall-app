<script lang="ts">
    import { onMount } from "svelte";
    import { toast } from "svelte-sonner";
    import { commands } from "../../bindings";
    import { Button } from "$lib/components/ui/button/index.js";
    import { Checkbox } from "$lib/components/ui/checkbox/index.js";
    import { Label } from "$lib/components/ui/label/index.js";
    import { openUrl } from "@tauri-apps/plugin-opener";
    import { t } from "$lib/i18n.svelte";

    const TELEMETRY_DOCS_URL = "https://github.com/rasben/recall-app#telemetry";

    let clearing = $state(false);
    let telemetryEnabled = $state(true);
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
            toast.error(t("settings.cache.error"), {richColors: true});
        } else {
            toast.success(t("settings.cache.cleared"));
            await loadSize();
        }
    }

    async function loadTelemetry() {
        const settings = await commands.getSettingsTelemetry();
        telemetryEnabled = settings.enabled;
    }

    async function toggleTelemetry(checked: boolean) {
        telemetryEnabled = checked;
        const result = await commands.setSettingsTelemetry({ enabled: checked });
        if (result.status === "error") {
            telemetryEnabled = !checked;
            toast.error(t("settings.telemetry.error"), { richColors: true });
        }
    }

    onMount(() => {
        loadSize();
        loadTelemetry();
    });
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

<fieldset class="border-2 p-4 mt-6">
    <legend>{t("settings.telemetry.legend")}</legend>
    <div class="flex items-center gap-2 mb-2">
        <Checkbox
            id="telemetry-enabled"
            checked={telemetryEnabled}
            onCheckedChange={(v) => toggleTelemetry(v === true)}
        />
        <Label for="telemetry-enabled">{t("settings.telemetry.enable")}</Label>
    </div>
    <p class="text-muted-foreground text-sm">
        {t("settings.telemetry.description")}
        <button
            type="button"
            class="underline underline-offset-2 hover:text-foreground"
            onclick={() => openUrl(TELEMETRY_DOCS_URL)}
        >
            {t("settings.telemetry.learn_more")}
        </button>
    </p>
</fieldset>
