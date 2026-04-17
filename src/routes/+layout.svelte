<script lang="ts">
	import './layout.css';
    import { Toaster } from "$lib/components/ui/sonner/index.js";
    const { children } = $props();
</script>

<div>
    <Toaster />

    {@render children?.()}
</div>
