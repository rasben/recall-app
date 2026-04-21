<script lang="ts">
import { Input } from "$lib/components/ui/input/index";
import { EyeClosedIcon, EyeIcon } from "@lucide/svelte";
import { Label } from "$lib/components/ui/label/index";

let showPassword = $state(false);

let {
    password = $bindable<string>(),
    saveAction,
    label,
    description = null as string | null,
    placeholder,
    inputId = "password-input",
}: {
    password: string;
    saveAction: () => Promise<void>;
    label: string;
    description?: string | null;
    placeholder: string;
    inputId?: string;
} = $props();
</script>

<fieldset class="mb-4">
    <Label for={inputId} class="mb-2">{label}</Label>
    <div class="relative">
        <Input type={showPassword ? 'text' : 'password'} id={inputId} placeholder={placeholder} bind:value={password} onblur={async () => await saveAction()} />
        <button onclick={() => showPassword = !showPassword} class="absolute hover:bg-primary-hover bg-primary border-2 px-1 right-0 top-0 bottom-0">
            {#if showPassword }
                <EyeClosedIcon class="text-black" />
            {:else }
                <EyeIcon class="text-black" />
            {/if}
        </button>
    </div>
    {#if description}
        <p class="text-muted-foreground text-sm mt-2 px-4">
            {@html description}
        </p>
    {/if}
</fieldset>
