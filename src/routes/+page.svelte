<script lang="ts">
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import Settings from "../components/Settings.svelte";
  import Main from "../components/Main.svelte";
</script>

<main class="flex min-h-screen flex-col p-8">
  <Tabs.Root value="main" class="min-h-0 min-w-0 flex-1">
    <Tabs.List>
      <Tabs.Trigger value="main">Main</Tabs.Trigger>
      <Tabs.Trigger value="settings">Settings</Tabs.Trigger>
    </Tabs.List>
    <Tabs.Content value="main" class="mt-4 flex min-h-0 flex-col !border-0 !p-0">
      <Main />
    </Tabs.Content>
    <Tabs.Content value="settings" class="mt-4 flex min-h-0 flex-col !border-0 !p-0">
      <Settings />
    </Tabs.Content>
  </Tabs.Root>
</main>
