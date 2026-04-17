<script lang="ts">
  import * as Card from "$lib/components/ui/card/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
  import { toast } from "svelte-sonner";

  let theme = $state(localStorage.getItem("theme") || "system");

  function setTheme(value: string) {
    if (!value) return;
    theme = value;

    if (value === "system") {
      localStorage.removeItem("theme");
    } else {
      localStorage.setItem("theme", value);
    }

    if (value === "dark" || (value === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches)) {
      document.documentElement.classList.add("dark");
    } else {
      document.documentElement.classList.remove("dark");
    }

    toast.success("Theme updated!");
  }
</script>

<Card.Root class="mt-4">
  <Card.Header>
    <Card.Title>Settings</Card.Title>
  </Card.Header>
  <Card.Content>
    <Label for="theme" class="mb-2">Theme</Label>
    <ToggleGroup.Root
      variant="outlined"
      type="single"
      class="w-full"
      id="theme"
      value={theme}
      onValueChange={setTheme}
    >
      <ToggleGroup.Item value="light">Light</ToggleGroup.Item>
      <ToggleGroup.Item value="dark">Dark</ToggleGroup.Item>
      <ToggleGroup.Item value="system">System</ToggleGroup.Item>
    </ToggleGroup.Root>
  </Card.Content>
</Card.Root>
