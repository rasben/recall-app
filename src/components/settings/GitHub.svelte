<script lang="ts">
    import { Label } from "$lib/components/ui/label/index.js";
    import { Checkbox } from "$lib/components/ui/checkbox/index.js";
    import { toast } from "svelte-sonner";
    import {onMount} from "svelte";
    import {commands, type SettingsGitHub, type GitHubEvent} from "../../bindings";
    import PasswordInput from "../ui/PasswordInput.svelte";
    import * as Select from "$lib/components/ui/select/index.js";

    const eventTypeMap: Record<string, { type: GitHubEvent, label: string }> = {
        "pullRequestEvent": { type: "PullRequestEvent", label: "Pull Request (PR)" },
        "pullRequestReviewEvent": { type: "PullRequestReviewEvent", label: "PR: Review" },
        "pullRequestReviewCommentEvent": { type: "PullRequestReviewCommentEvent", label: "PR: Review Comment" },
        "issuesEvent": { type: "IssuesEvent", label: "Issue" },
        "issueCommentEvent": { type: "IssueCommentEvent", label: "Issue: Comment" },
    };

    function eventTypeLabel(event: GitHubEvent): string {
        return Object.values(eventTypeMap).find((e) => e.type === event)?.label ?? event;
    }

    let defaultSettings = {
        enabled: false,
        use_cli: true,
        token: "",
        enabled_events: [
            eventTypeMap["pullRequestEvent"].type
        ]
    };

    let settings = $state<SettingsGitHub>(defaultSettings);

    let enabled = $state(false);
    let useCli = $state(true);
    let token = $state("");
    let enabledEvents = $state<GitHubEvent[]>([]);

    onMount(() => {
        getSettings();
    });

    async function getSettings() {
        settings = await commands.getSettingsGithub() ?? defaultSettings;
        enabled = settings.enabled;
        useCli = settings.use_cli;
        token = settings.token;
        enabledEvents = settings.enabled_events ?? [];
    }

    async function toggleEnabled(checked: boolean) {
        const original = settings.enabled;
        enabled = checked;
        settings.enabled = checked;

        const result = await commands.setSettingsGithub(settings)

        if (result.status === "error") {
            enabled = original;
            settings.enabled = original;

            toast.error("Could not enable GitHub source");
        }
    }

    async function toggleCli(checked: boolean) {
       const original = settings.use_cli;
        useCli = checked;
        settings.use_cli = checked;

        const result = await commands.setSettingsGithub(settings)

        if (result.status === "error") {
            useCli = original;
            settings.use_cli = original;
            toast.error("Cannot use GH cli. Is it installed?");
        }
    }

    async function setToken() {
        const original = settings.token;
        settings.token = token;

        const result = await commands.setSettingsGithub(settings)

        if (result.status === "error") {
            token = original;
            settings.token = original;
            toast.error("Could not use token - is it valid?");
        }
        else {
            toast.success("Token saved!")
        }
    }

    async function setEnabledEvents(value: string[] | undefined) {
        const original = settings.enabled_events;
        settings.enabled_events = (value ?? []) as GitHubEvent[];

        const result = await commands.setSettingsGithub(settings)

        if (result.status === "error") {
            enabledEvents = original;
            settings.enabled_events = original;
            toast.error("Could not set chosen events");
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
            <PasswordInput bind:password={token} saveAction={setToken} label="GitHub PAT" placeholder="Add token.." inputId="github-pat" />


        {/if}





        <label for="github-enabled-events-trigger" class="mb-2">
            Events to show
        </label>

        <Select.Root
                type="multiple"
                bind:value={enabledEvents}
                onValueChange={setEnabledEvents}
        >
            <Select.Trigger id="github-enabled-events-trigger" class="w-full">
                {enabledEvents.length === 0
                    ? "No events chosen"
                    : [...enabledEvents]
                        .sort((a, b) => eventTypeLabel(a).localeCompare(eventTypeLabel(b)))
                        .map(eventTypeLabel)
                        .join(", ")}
            </Select.Trigger>
            <Select.Content class="max-h-[300px]">
                {#each Object.entries(eventTypeMap) as [key, { type, label }]}
                    <Select.Item value={type} label={label} />
                {/each}
            </Select.Content>
        </Select.Root>



    {/if}

</fieldset>
