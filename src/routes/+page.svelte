<script lang="ts">
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import Settings from "../components/Settings.svelte";
  import Main from "../components/Main.svelte";
</script>

<main class="p-8">
  <Tabs.Root value="main">
    <Tabs.List>
      <Tabs.Trigger value="main">Main</Tabs.Trigger>
      <Tabs.Trigger value="settings">Settings</Tabs.Trigger>
    </Tabs.List>
    <Tabs.Content value="main">
      <Main />
    </Tabs.Content>
    <Tabs.Content value="settings">
      <Settings />
    </Tabs.Content>
  </Tabs.Root>
</main>
