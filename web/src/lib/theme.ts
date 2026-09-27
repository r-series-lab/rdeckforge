export type ThemeMode = "dark" | "light";

const THEME_STORAGE_KEY = "rdeckforge.theme";

export function readTheme(): ThemeMode {
  if (typeof localStorage === "undefined") {
    return "dark";
  }
  return localStorage.getItem(THEME_STORAGE_KEY) === "light" ? "light" : "dark";
}

export function applyTheme(theme: ThemeMode) {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
}

export function saveTheme(theme: ThemeMode) {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  }
  applyTheme(theme);
}
