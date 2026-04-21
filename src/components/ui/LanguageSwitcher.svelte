<script lang="ts">
    import { i18n, setLang, t } from "$lib/i18n.svelte";
    import type { Lang } from "$lib/translations";
    import { Button } from "$lib/components/ui/button/index.js";
    import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
    import {applyTheme} from "$lib/theme";
    import {commands, type SettingsUi} from "../../bindings";
    import {toast} from "svelte-sonner";
    import {onMount} from "svelte";

    let language = $state<string>("da");
    const langs: Lang[] = ["da", "en"];

    onMount(async () => {
        language = i18n.lang;
    });

    async function chooseLanguage(value: string) {
        let result = false;

        result = setLang(<"en" | "da">value)

        if (!result) {
            language = i18n.lang;
            toast.error(t("settings.language.error"));
        }
    }
</script>

<ToggleGroup.Root
        variant="outlined"
        type="single"
        class="w-full"
        id="language"
        value={language}
        onValueChange={chooseLanguage}
>
    {#each langs as lang}
        <ToggleGroup.Item value={lang}>
            {lang === 'en' ? 'English' : 'Dansk'}
        </ToggleGroup.Item>
    {/each}
</ToggleGroup.Root>
