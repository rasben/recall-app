<script lang="ts">
    import { Label } from "$lib/components/ui/label/index.js";
    import { Checkbox } from "$lib/components/ui/checkbox/index.js";
    import { toast } from "svelte-sonner";
    import {onMount} from "svelte";
    import {commands, type SettingsGitHub} from "../../bindings";
    import PasswordInput from "../ui/PasswordInput.svelte";

    let defaultSettings = {
        enabled: false,
        use_cli: true,
        token: "",
    };

    let settings = $state<SettingsGitHub>(defaultSettings);

    let enabled = $state(false);
    let useCli = $state(true);
    let token = $state("");

    onMount(() => {
        getSettings();
    });

    async function getSettings() {
        settings = await commands.getSettingsGithub() ?? defaultSettings;
        enabled = settings.enabled;
        useCli = settings.use_cli;
        token = settings.token;
    }

    async function toggleEnabled(checked: boolean) {
        enabled = checked;
        settings.enabled = checked;

        const result = await commands.setSettingsGithub(settings)

        if (result.status === "error") {
            toast.error("Could not enable GitHub source");
        }
    }

    async function toggleCli(checked: boolean) {
        useCli = checked;
        settings.use_cli = checked;

        const result = await commands.setSettingsGithub(settings)

        if (result.status === "error") {
            toast.error("Cannot use GH cli. Is it installed?");
        }
    }

    async function setToken() {
        settings.token = token;

        const result = await commands.setSettingsGithub(settings)

        if (result.status === "error") {
            toast.error("Could not use token - is it valid?");
        }
        else {
            toast.success("Token saved!")
        }
    }


</script>


<fieldset class="border-2 p-4 mt-4">
    <legend class="mb-2">GitHub</legend>

    <div class="flex items-center gap-2 mb-4">
        <Checkbox
                id="github-enabled"
                checked={enabled}
                onCheckedChange={(v) => toggleEnabled(v === true)}
        />
        <Label for="github-enabled">Enable GitHub source</Label>
    </div>

    {#if enabled}
        <div class="flex items-center gap-2 mb-4">
            <Checkbox
                    id="github-use-cli"
                    checked={useCli}
                    onCheckedChange={(v) => toggleCli(v === true)}
            />
            <Label for="github-use-cli">Use GH CLI</Label>

        </div>

        {#if !useCli }
            <div class="-mt-2 mb-4 text-red-600 font-bold">
                For now, only GH CLI is supported. In the future, we will support GH PAT tokens.
            </div>
            <PasswordInput bind:password={token} saveAction={setToken} label="GitHub PAT" placeholder="Add token.." />


        {/if}
    {/if}

</fieldset>
