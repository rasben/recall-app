<script lang="ts">
    import { Label } from "$lib/components/ui/label/index.js";
    import { Checkbox } from "$lib/components/ui/checkbox/index.js";
    import { Input } from "$lib/components/ui/input/index.js";
    import { toast } from "svelte-sonner";
    import { onMount } from "svelte";
    import { commands, type SettingsGitHub, type GitHubEvent } from "../../bindings";
    import * as Select from "$lib/components/ui/select/index.js";
    import { t } from "$lib/i18n.svelte";
    import PasswordInput from "../ui/PasswordInput.svelte";

    const eventTypeMap: Record<string, { type: GitHubEvent; labelKey: string }> = {
        pullRequestEvent: { type: "PullRequestEvent", labelKey: "settings.github.event.pull_request" },
        pullRequestReviewEvent: { type: "PullRequestReviewEvent", labelKey: "settings.github.event.pr_review" },
        pullRequestReviewCommentEvent: { type: "PullRequestReviewCommentEvent", labelKey: "settings.github.event.pr_review_comment" },
        issuesEvent: { type: "IssuesEvent", labelKey: "settings.github.event.issue" },
        issueCommentEvent: { type: "IssueCommentEvent", labelKey: "settings.github.event.issue_comment" },
    };

    function eventTypeLabel(event: GitHubEvent): string {
        const entry = Object.values(eventTypeMap).find((e) => e.type === event);
        return entry ? t(entry.labelKey as Parameters<typeof t>[0]) : event;
    }

    const defaultSettings: SettingsGitHub = {
        enabled: false,
        username: "",
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
        if (!ok) toast.error(t("settings.github.error_enable"));
    }

    async function saveCredentials() {
        const ok = await persist({});
        if (ok) toast.success(t("settings.github.saved"));
        else toast.error(t("settings.github.error_save"));
    }

    async function setEnabledEvents(value: string[] | undefined) {
        const ok = await persist({ enabled_events: (value ?? []) as GitHubEvent[] });
        if (!ok) toast.error(t("settings.github.error_events"));
    }
</script>

<fieldset class="border-2 p-4 mt-6">
    <legend>{t("settings.github.legend")}</legend>

    <div class="flex items-center gap-2 mb-4">
        <Checkbox
            id="github-enabled"
            checked={settings.enabled}
            onCheckedChange={(v) => toggleEnabled(v === true)}
        />
        <Label for="github-enabled">{t("settings.github.enable")}</Label>
    </div>

    {#if settings.enabled}
        <div class="mb-4">
            <Label for="github-username" class="mb-2">{t("settings.github.username")}</Label>
            <Input
                id="github-username"
                placeholder={t("settings.github.username_placeholder")}
                bind:value={settings.username}
                onblur={saveCredentials}
            />
        </div>

        <PasswordInput
            bind:password={settings.token}
            saveAction={saveCredentials}
            label={t("settings.github.token")}
            placeholder={t("settings.github.token_placeholder")}
            description={t("settings.github.token_description")}
            inputId="github-token"
        />

        <Label for="github-enabled-events-trigger" class="mb-2">{t("settings.github.events_label")}</Label>
        <Select.Root
            type="multiple"
            bind:value={settings.enabled_events}
            onValueChange={setEnabledEvents}
        >
            <Select.Trigger id="github-enabled-events-trigger" class="w-full">
                {settings.enabled_events.length === 0
                    ? t("settings.github.no_events")
                    : [...settings.enabled_events]
                        .sort((a, b) => eventTypeLabel(a).localeCompare(eventTypeLabel(b)))
                        .map(eventTypeLabel)
                        .join(", ")}
            </Select.Trigger>
            <Select.Content class="max-h-[300px]">
                {#each Object.entries(eventTypeMap) as [, { type, labelKey }]}
                    <Select.Item value={type} label={t(labelKey as Parameters<typeof t>[0])} />
                {/each}
            </Select.Content>
        </Select.Root>
    {/if}
</fieldset>
