import { invoke } from "@tauri-apps/api/core";
import { derived, writable } from "svelte/store";
import { providerText, translate, type LanguagePreference, type LanguageSettings, type Translator } from "./i18n";

export const language = writable<LanguageSettings & { ready: boolean; saving: boolean }>({
  preference: "system", locale: "en", ready: false, saving: false,
});
export const t = derived(language, ({ locale }): Translator => (message, params) => translate(locale, message, params));
export const localize = derived(language, ({ locale }) => (text: string) => providerText(locale, text));

let request = 0;
let saving = false;

export async function refreshLanguage(): Promise<void> {
  if (saving) return;
  const current = ++request;
  const settings = await invoke<LanguageSettings>("get_language");
  if (current === request) language.set({ ...settings, ready: true, saving: false });
}

export async function setLanguage(preference: LanguagePreference): Promise<void> {
  saving = true;
  ++request; // A startup/system-language read must not overwrite this choice.
  language.update((value) => ({ ...value, saving: true }));
  try {
    const settings = await invoke<LanguageSettings>("set_language", { language: preference });
    language.set({ ...settings, ready: true, saving: true });
  } finally {
    saving = false;
    language.update((value) => ({ ...value, saving: false }));
  }
}
