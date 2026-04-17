export function applyTheme(value: string) {
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const dark = value === "dark" || (value === "system" && prefersDark);

  document.documentElement.classList.toggle("dark", dark);
}
