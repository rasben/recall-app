<script lang="ts">
    import { Label } from "$lib/components/ui/label/index.js";
    import { Checkbox } from "$lib/components/ui/checkbox/index.js";
    import { toast } from "svelte-sonner";
    import { onMount } from "svelte";
    import { commands, type SettingsGitHub, type GitHubEvent } from "../../bindings";
    import * as Select from "$lib/components/ui/select/index.js";

    const eventTypeMap: Record<string, { type: GitHubEvent; label: string }> = {
        pullRequestEvent: { type: "PullRequestEvent", label: "Pull Request (PR)" },
        pullRequestReviewEvent: { type: "PullRequestReviewEvent", label: "PR: Review" },
        pullRequestReviewCommentEvent: { type: "PullRequestReviewCommentEvent", label: "PR: Review Comment" },
        issuesEvent: { type: "IssuesEvent", label: "Issue" },
        issueCommentEvent: { type: "IssueCommentEvent", label: "Issue: Comment" },
    };

    function eventTypeLabel(event: GitHubEvent): string {
        return Object.values(eventTypeMap).find((e) => e.type === event)?.label ?? event;
    }

    const defaultSettings: SettingsGitHub = {
        enabled: false,
        use_cli: true,
        token: "",
        enabled_events: [eventTypeMap.pullRequestEvent.type],
    };

    let settings = $state<SettingsGitHub>(defaultSettings);

    onMount(async () => {
        settings = await commands.getSettingsGithub() ?? defaultSettings;
    });

    async function persist(partial: Partial<SettingsGitHub>) {
        const next: SettingsGitHub = { ...settings, ...partial };
        const result = await commands.setSettingsGithub(next);
        if (result.status === "error") return false;
        settings = next;
        return true;
    }

    async function toggleEnabled(checked: boolean) {
        const ok = await persist({ enabled: checked });
        if (!ok) toast.error("Could not enable GitHub source");
    }

    async function toggleCli(checked: boolean) {
        const ok = await persist({ use_cli: checked });
        if (!ok) toast.error("Cannot use GH CLI. Is it installed?");
    }

    async function setEnabledEvents(value: string[] | undefined) {
        const ok = await persist({ enabled_events: (value ?? []) as GitHubEvent[] });
        if (!ok) toast.error("Could not set chosen events");
    }
</script>

<fieldset class="border-2 p-4 mt-6">
    <legend>GitHub</legend>

    <div class="flex items-center gap-2 mb-4">
        <Checkbox
            id="github-enabled"
            checked={settings.enabled}
            onCheckedChange={(v) => toggleEnabled(v === true)}
        />
        <Label for="github-enabled">Enable GitHub source</Label>
    </div>

    {#if settings.enabled}
        <div class="flex items-center gap-2 mb-4">
            <Checkbox
                id="github-use-cli"
                checked={settings.use_cli}
                onCheckedChange={(v) => toggleCli(v === true)}
            />
            <Label for="github-use-cli">Use GH CLI</Label>
        </div>

        {#if !settings.use_cli}
            <div class="-mt-2 mb-4 font-bold text-red-600">
                For now, only GH CLI is supported. In the future, we will support GH PAT tokens.
            </div>
        {/if}

        <Label for="github-enabled-events-trigger" class="mb-2">Events to show</Label>
        <Select.Root
            type="multiple"
            bind:value={settings.enabled_events}
            onValueChange={setEnabledEvents}
        >
            <Select.Trigger id="github-enabled-events-trigger" class="w-full">
                {settings.enabled_events.length === 0
                    ? "No events chosen"
                    : [...settings.enabled_events]
                        .sort((a, b) => eventTypeLabel(a).localeCompare(eventTypeLabel(b)))
                        .map(eventTypeLabel)
                        .join(", ")}
            </Select.Trigger>
            <Select.Content class="max-h-[300px]">
                {#each Object.entries(eventTypeMap) as [, { type, label }]}
                    <Select.Item value={type} {label} />
                {/each}
            </Select.Content>
        </Select.Root>
    {/if}
</fieldset>
