use std::collections::HashMap;
use std::sync::OnceLock;

pub mod errors;

pub struct Language {
    pub code: &'static str,
    pub native_name: &'static str,
    catalogue: &'static str,
}

impl std::fmt::Debug for Language {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Language")
            .field("code", &self.code)
            .finish()
    }
}

impl PartialEq for Language {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
    }
}

impl Eq for Language {}

pub type Lang = &'static Language;

pub const ZH_CN: Language = Language {
    code: "zh-CN",
    native_name: "简体中文",
    catalogue: include_str!("../../locales/zh-CN.json"),
};

pub const EN_US: Language = Language {
    code: "en-US",
    native_name: "English",
    catalogue: include_str!("../../locales/en-US.json"),
};

pub const LANGUAGES: &[Language] = &[ZH_CN, EN_US];
pub const FALLBACK: Lang = &EN_US;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguagePref {
    #[default]
    Auto,
    Fixed(Lang),
}

impl LanguagePref {
    pub fn from_setting(value: Option<&str>) -> Self {
        match value.and_then(language_by_code) {
            Some(lang) => Self::Fixed(lang),
            None => Self::Auto,
        }
    }

    pub fn as_setting(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Fixed(lang) => lang.code,
        }
    }

    pub fn resolve(self) -> Lang {
        match self {
            Self::Auto => detect_system_language(),
            Self::Fixed(lang) => lang,
        }
    }
}

pub fn language_by_code(code: &str) -> Option<Lang> {
    LANGUAGES
        .iter()
        .find(|lang| lang.code.eq_ignore_ascii_case(code))
}

pub fn detect_system_language() -> Lang {
    for var in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        let Ok(raw) = std::env::var(var) else {
            continue;
        };
        for tag in raw.split(':') {
            if let Some(lang) = language_from_tag(tag) {
                return lang;
            }
        }
    }
    FALLBACK
}

fn language_from_tag(raw: &str) -> Option<Lang> {
    let base = raw.split(['.', '@']).next().unwrap_or("").replace('_', "-");
    if base.is_empty() || base.eq_ignore_ascii_case("C") || base.eq_ignore_ascii_case("POSIX") {
        return None;
    }
    if let Some(lang) = language_by_code(&base) {
        return Some(lang);
    }
    let primary = base.split('-').next().unwrap_or("").to_ascii_lowercase();
    let code = match primary.as_str() {
        "zh" => "zh-CN",
        "en" => "en-US",
        _ => return None,
    };
    language_by_code(code)
}

fn catalogues() -> &'static HashMap<&'static str, &'static HashMap<String, String>> {
    static CATALOGUES: OnceLock<HashMap<&'static str, &'static HashMap<String, String>>> =
        OnceLock::new();
    CATALOGUES.get_or_init(|| {
        LANGUAGES
            .iter()
            .map(|lang| {
                let map: &'static HashMap<String, String> =
                    Box::leak(Box::new(parse_catalogue(lang.catalogue, lang.code)));
                (lang.code, map)
            })
            .collect()
    })
}

fn parse_catalogue(raw: &str, code: &str) -> HashMap<String, String> {
    match serde_json::from_str(raw) {
        Ok(map) => map,
        Err(error) => {
            tracing::error!(%error, language = code, "embedded locale catalogue is invalid");
            HashMap::new()
        }
    }
}

fn lookup(lang: Lang, key: &str) -> Option<&'static str> {
    catalogues().get(lang.code)?.get(key).map(String::as_str)
}

pub fn tr(lang: Lang, key: &'static str) -> &'static str {
    lookup(lang, key)
        .or_else(|| lookup(FALLBACK, key))
        .unwrap_or(key)
}

pub fn tr_args(lang: Lang, key: &'static str, args: &[(&str, &str)]) -> String {
    let template = tr(lang, key);
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let name = &after[..close];
        match args.iter().find(|(arg, _)| *arg == name) {
            Some((_, value)) => out.push_str(value),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

pub fn tr_plural(lang: Lang, one: &'static str, other: &'static str, n: u64) -> &'static str {
    tr(lang, if n == 1 { one } else { other })
}

pub fn tr_plural_args(
    lang: Lang,
    one: &'static str,
    other: &'static str,
    n: u64,
    args: &[(&str, &str)],
) -> String {
    tr_args(lang, if n == 1 { one } else { other }, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    fn parse(raw: &str) -> BTreeMap<String, String> {
        serde_json::from_str(raw).expect("locale catalogue must be valid JSON")
    }

    fn placeholders(value: &str) -> BTreeSet<&str> {
        let mut found = BTreeSet::new();
        let mut rest = value;
        while let Some(open) = rest.find('{') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else { break };
            found.insert(&after[..close]);
            rest = &after[close + 1..];
        }
        found
    }

    #[test]
    fn catalogues_share_keys_and_placeholders() {
        let reference = parse(FALLBACK.catalogue);
        let reference_keys: BTreeSet<_> = reference.keys().collect();
        for lang in LANGUAGES {
            let catalogue = parse(lang.catalogue);
            let keys: BTreeSet<_> = catalogue.keys().collect();
            assert_eq!(
                keys, reference_keys,
                "locale key sets differ for {}",
                lang.code
            );
            for (key, reference_value) in &reference {
                assert!(
                    !reference_value.trim().is_empty(),
                    "empty {} value for {key}",
                    FALLBACK.code
                );
                let value = catalogue.get(key).expect("missing key");
                assert!(
                    !value.trim().is_empty(),
                    "empty {} value for {key}",
                    lang.code
                );
                assert_eq!(
                    placeholders(value),
                    placeholders(reference_value),
                    "placeholder mismatch for {key} in {}",
                    lang.code
                );
            }
        }
    }

    #[test]
    fn lookup_returns_translations_and_falls_back_to_key() {
        assert_eq!(tr(&ZH_CN, "common.save"), "保存");
        assert_eq!(tr(&EN_US, "common.save"), "Save");
        assert_eq!(tr(&EN_US, "missing.key"), "missing.key");
    }

    #[test]
    fn args_replace_placeholders() {
        assert_eq!(
            tr_args(&EN_US, "launch.exited_with_code", &[("code", "1")]),
            "Game exited unexpectedly (exit code 1)"
        );
        assert_eq!(
            tr_args(
                &EN_US,
                "app.fatal.database",
                &[("path", "/tmp/{error}"), ("error", "boom")]
            ),
            "Cannot open database /tmp/{error}: boom"
        );
        assert_eq!(
            tr_args(&EN_US, "app.fatal.database", &[("error", "boom")]),
            "Cannot open database {path}: boom"
        );
    }

    #[test]
    fn parses_locale_tags() {
        assert_eq!(
            language_from_tag("zh_CN.UTF-8").map(|l| l.code),
            Some("zh-CN")
        );
        assert_eq!(
            language_from_tag("en_US.UTF-8").map(|l| l.code),
            Some("en-US")
        );
        assert_eq!(language_from_tag("en").map(|l| l.code), Some("en-US"));
        assert_eq!(language_from_tag("zh_TW").map(|l| l.code), Some("zh-CN"));
        assert_eq!(language_from_tag("C"), None);
        assert_eq!(language_from_tag("POSIX"), None);
        assert_eq!(language_from_tag("de_DE.UTF-8"), None);
        assert_eq!(language_from_tag("ZH_CN").map(|l| l.code), Some("zh-CN"));
        assert_eq!(
            language_from_tag("en-US@euro").map(|l| l.code),
            Some("en-US")
        );
    }

    #[test]
    fn keys_used_in_source_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = Vec::new();
        for file in rust_files(&root) {
            let source = std::fs::read_to_string(&file).expect("read source file");
            for key in keys_in(&source) {
                if lookup(&ZH_CN, &key).is_none() || lookup(&EN_US, &key).is_none() {
                    missing.push(format!("{}: {key}", file.display()));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "missing or partially translated keys:\n{}",
            missing.join("\n")
        );
    }

    fn rust_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).expect("read source dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                files.extend(rust_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
        files
    }

    fn keys_in(source: &str) -> Vec<String> {
        let bytes = source.as_bytes();
        let mut keys = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b'"' {
                index += 1;
                continue;
            }
            let start = index + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b'"' {
                end += if bytes[end] == b'\\' { 2 } else { 1 };
            }
            if end >= bytes.len() {
                break;
            }
            if let Ok(literal) = std::str::from_utf8(&bytes[start..end]) {
                if looks_like_key(literal) && preceded_by_tr_marker(source, index) {
                    keys.push(literal.to_owned());
                }
            }
            index = end + 1;
        }
        keys
    }

    const TR_MARKERS: &[&str] = &["tr(", "tr_args(", "tr_plural(", "tr_plural_args("];

    fn preceded_by_tr_marker(source: &str, quote: usize) -> bool {
        let mut begin = quote.saturating_sub(200);
        while begin > 0 && !source.is_char_boundary(begin) {
            begin -= 1;
        }
        let before = &source[begin..quote];
        TR_MARKERS.iter().any(|marker| before.contains(marker))
    }

    const KEY_NAMESPACES: &[&str] = &[
        "app.",
        "choices.",
        "common.",
        "delete.",
        "detail.",
        "dialogs.",
        "draft.",
        "errors.",
        "launch.",
        "library.",
        "logs.",
        "nav.",
        "runners.",
        "settings.",
        "time.",
        "widgets.",
    ];

    fn looks_like_key(value: &str) -> bool {
        !value.is_empty()
            && KEY_NAMESPACES
                .iter()
                .any(|namespace| value.starts_with(namespace))
            && value.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            })
    }

    #[test]
    fn language_pref_round_trips() {
        assert_eq!(LanguagePref::from_setting(None), LanguagePref::Auto);
        assert_eq!(LanguagePref::from_setting(Some("")), LanguagePref::Auto);
        assert_eq!(LanguagePref::from_setting(Some("nope")), LanguagePref::Auto);
        assert_eq!(
            LanguagePref::from_setting(Some("en-US")),
            LanguagePref::Fixed(&EN_US)
        );
        assert_eq!(LanguagePref::Fixed(&ZH_CN).as_setting(), "zh-CN");
        assert_eq!(LanguagePref::Auto.as_setting(), "auto");
        assert_eq!(LanguagePref::Fixed(&ZH_CN).resolve().code, "zh-CN");
    }
}
