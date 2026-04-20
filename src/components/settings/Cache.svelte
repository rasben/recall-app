<script lang="ts">
    import { toast } from "svelte-sonner";
    import { commands } from "../../bindings";

    let clearing = $state(false);

    async function clearCaches() {
        clearing = true;
        const result = await commands.clearAllCaches();
        clearing = false;

        if (result.status === "error") {
            toast.error("Failed to clear caches");
        } else {
            toast.success("Caches cleared");
        }
    }
</script>

<fieldset class="border-2 p-4 mt-6">
    <legend>Cache</legend>
    <p class="text-muted-foreground mb-3 text-sm">
        Clear all cached timeline data. Loading will be slower until caches are rebuilt.
    </p>
    <button
        class="border-2 px-3 py-1 text-sm hover:bg-muted disabled:opacity-50"
        disabled={clearing}
        onclick={clearCaches}
    >
        {clearing ? "Clearing…" : "Clear all caches"}
    </button>
</fieldset>
