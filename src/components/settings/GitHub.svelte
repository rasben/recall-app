<script lang="ts">
    import { Label } from "$lib/components/ui/label/index.js";
    import { Checkbox } from "$lib/components/ui/checkbox/index.js";
    import { Input } from "$lib/components/ui/input/index.js";
    import { toast } from "svelte-sonner";
    import { onMount } from "svelte";
    import { commands, type SettingsGitHub, type GitHubEvent } from "../../bindings";
    import { t } from "$lib/i18n.svelte";
    import PasswordInput from "../ui/PasswordInput.svelte";
    import TestConnectionButton from "../ui/TestConnectionButton.svelte";

    const eventTypeMap: Record<string, { type: GitHubEvent; labelKey: string }> = {
        pullRequestEvent: { type: "PullRequestEvent", labelKey: "settings.github.event.pull_request" },
        pullRequestReviewEvent: { type: "PullRequestReviewEvent", labelKey: "settings.github.event.pr_review" },
        pullRequestReviewCommentEvent: { type: "PullRequestReviewCommentEvent", labelKey: "settings.github.event.pr_review_comment" },
        issuesEvent: { type: "IssuesEvent", labelKey: "settings.github.event.issue" },
        issueCommentEvent: { type: "IssueCommentEvent", labelKey: "settings.github.event.issue_comment" },
    };

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
        if (!ok) toast.error(t("settings.github.error_enable"), {richColors: true});
    }

    async function saveCredentials() {
        const ok = await persist({});
        if (ok) toast.success(t("settings.github.saved"));
        else toast.error(t("settings.github.error_save"), {richColors: true});
    }

    async function toggleEvent(type: GitHubEvent, checked: boolean) {
        const current = settings.enabled_events ?? [];
        const next = checked
            ? (current.includes(type) ? current : [...current, type])
            : current.filter((e) => e !== type);
        const ok = await persist({ enabled_events: next });
        if (!ok) toast.error(t("settings.github.error_events"), {richColors: true});
    }

</script>

<fieldset class="relative border-2 p-4 mt-6">
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

        <Label class="mb-2">{t("settings.github.events_label")}</Label>
        <div class="flex flex-col gap-2">
            {#each Object.entries(eventTypeMap) as [, { type, labelKey }]}
                <div class="flex items-center gap-2">
                    <Checkbox
                        id="github-event-{type}"
                        checked={settings.enabled_events.includes(type)}
                        onCheckedChange={(v) => toggleEvent(type, v === true)}
                    />
                    <Label for="github-event-{type}">{t(labelKey as Parameters<typeof t>[0])}</Label>
                </div>
            {/each}
        </div>

        <TestConnectionButton test={commands.testSettingsGithub} />
    {/if}
</fieldset>
