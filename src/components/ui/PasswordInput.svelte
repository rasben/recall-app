<script>
import {Input} from "$lib/components/ui/input/index.ts";
import {EyeClosedIcon, EyeIcon} from "@lucide/svelte";
import {Label} from "$lib/components/ui/label/index.ts";
let showPassword = $state(false);

let { password = $bindable(), saveAction, label, placeholder } = $props();
</script>

<fieldset class="mb-8">
    <Label for="token" class="mb-2">{label}</Label>
    <div class="relative">
        <Input type={showPassword ? 'text' : 'password'} id="token" placeholder={placeholder} bind:value={password} onblur={async () => await saveAction()} />
        <button onclick={() => showPassword = !showPassword} class="absolute hover:bg-primary-hover bg-primary border-2 px-1 right-0 top-0 bottom-0">
            {#if showPassword }
                <EyeClosedIcon class="text-black" />
            {:else }
                <EyeIcon class="text-black" />
            {/if}
        </button>
    </div>
</fieldset>
