<script lang="ts">
    import { onMount } from "svelte";
    import { toast } from "svelte-sonner";
    import { commands } from "../../bindings";

    let clearing = $state(false);
    let cachedDays = $state<number | null>(null);
    let bytes = $state<number | null>(null);

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
            toast.error("Failed to clear caches");
        } else {
            toast.success("Caches cleared");
            await loadSize();
        }
    }

    onMount(loadSize);
</script>

<fieldset class="border-2 p-4 mt-6">
    <legend>Cache</legend>
    <p class="text-muted-foreground mb-3 text-sm">
        Clear all cached timeline data. Loading will be slower until caches are rebuilt.
    </p>
    {#if cachedDays !== null && bytes !== null}
        <p class="text-muted-foreground mb-3 text-sm">
            {cachedDays} cached {cachedDays === 1 ? "day" : "days"} &middot; {formatBytes(Number(bytes))} on disk
        </p>
    {/if}
    <button
        class="border-2 px-3 py-1 text-sm hover:bg-muted disabled:opacity-50"
        disabled={clearing}
        onclick={clearCaches}
    >
        {clearing ? "Clearing…" : "Clear all caches"}
    </button>
</fieldset>
