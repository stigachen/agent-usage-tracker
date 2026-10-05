use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
    #[default]
    #[serde(rename = "system", other)]
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Locale {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
}

#[derive(Clone, Serialize)]
pub struct LanguageSettings {
    pub preference: Language,
    pub locale: Locale,
}

impl Language {
    pub fn resolve(self, system_locale: Option<&str>) -> Locale {
        match self {
            Self::English => Locale::English,
            Self::SimplifiedChinese => Locale::SimplifiedChinese,
            Self::System => {
                // Only the first OS language is considered. An unsupported primary
                // language falls back to English, even if Chinese is further down.
                let tag = system_locale.unwrap_or_default().replace('_', "-").to_ascii_lowercase();
                let mut parts = tag.split('-');
                if parts.next() != Some("zh") {
                    return Locale::English;
                }
                match parts.next() {
                    // An explicit script takes precedence over the region.
                    Some("hans" | "cn" | "sg") => Locale::SimplifiedChinese,
                    _ => Locale::English,
                }
            }
        }
    }

    pub fn settings(self) -> LanguageSettings {
        LanguageSettings { preference: self, locale: self.resolve(sys_locale::get_locale().as_deref()) }
    }
}

impl Locale {
    pub fn tray_tooltip(self, title: Option<&str>) -> String {
        match (self, title) {
            (_, None) => "Agent Usage".into(),
            (Self::English, Some(title)) => format!("Agent Usage — {title} remaining"),
            (Self::SimplifiedChinese, Some(title)) => format!("Agent Usage — 剩余 {title}"),
        }
    }

    #[cfg(any(target_os = "windows", test))]
    pub fn tray_menu_labels(self) -> [&'static str; 3] {
        match self {
            Self::English => ["Open Agent Usage", "Refresh", "Quit"],
            Self::SimplifiedChinese => ["打开 Agent Usage", "刷新", "退出"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_simplified_chinese_system_languages_select_chinese() {
        for tag in ["zh-CN", "zh-SG", "zh-Hans", "zh-Hans-CN", "zh-Hans-SG", "zh-Hans-US", "ZH_cn"] {
            assert_eq!(Language::System.resolve(Some(tag)), Locale::SimplifiedChinese, "{tag}");
        }
        for tag in ["en", "en-US", "en-GB", "ja-JP", "fr-FR", "zh", "zh-TW", "zh-HK", "zh-MO", "zh-Hant", "zh-Hant-CN", "zh-Latn-CN", "", "invalid"] {
            assert_eq!(Language::System.resolve(Some(tag)), Locale::English, "{tag}");
        }
        assert_eq!(Language::System.resolve(None), Locale::English);
    }

    #[test]
    fn explicit_choice_overrides_the_system_language() {
        for tag in [None, Some("zh-CN"), Some("en-US"), Some("ja-JP")] {
            assert_eq!(Language::English.resolve(tag), Locale::English);
            assert_eq!(Language::SimplifiedChinese.resolve(tag), Locale::SimplifiedChinese);
        }
    }

    #[test]
    fn native_tray_text_uses_the_resolved_language() {
        assert_eq!(Locale::SimplifiedChinese.tray_tooltip(Some("75%")), "Agent Usage — 剩余 75%");
        assert_eq!(Locale::SimplifiedChinese.tray_tooltip(None), "Agent Usage");
        assert_eq!(Locale::SimplifiedChinese.tray_menu_labels(), ["打开 Agent Usage", "刷新", "退出"]);
        assert_eq!(Locale::English.tray_menu_labels(), ["Open Agent Usage", "Refresh", "Quit"]);
    }
}
