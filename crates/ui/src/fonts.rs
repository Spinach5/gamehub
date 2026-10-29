use std::collections::BTreeMap;

/// A font family installed on the system, as reported by fontconfig.
///
/// The name is leaked once per scan so it can feed `Family::Name`, which
/// requires `&'static str`. Scans happen at startup and on explicit rescan,
/// so the retained memory is bounded in practice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontEntry {
    pub name: &'static str,
    pub cjk: bool,
    pub has_bold: bool,
}

impl FontEntry {
    pub fn matches(&self, query: &str) -> bool {
        query.is_empty() || self.name.to_lowercase().contains(&query.to_lowercase())
    }
}

pub fn scan() -> Result<Vec<FontEntry>, String> {
    let output = std::process::Command::new("fc-list")
        .args(["--format", "%{family[0]}\t%{style[0]}\t%{lang}\n"])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr.trim();
        return Err(if message.is_empty() {
            output.status.to_string()
        } else {
            message.to_owned()
        });
    }
    Ok(parse_fc_list(&String::from_utf8_lossy(&output.stdout)))
}

fn parse_fc_list(output: &str) -> Vec<FontEntry> {
    let mut families: BTreeMap<&str, FontEntry> = BTreeMap::new();
    for line in output.lines() {
        let mut parts = line.split('\t');
        let Some(name) = parts.next().map(str::trim).filter(|name| !name.is_empty()) else {
            continue;
        };
        // fontconfig exposes internal aliases as dot-prefixed families.
        if name.starts_with('.') {
            continue;
        }
        let style = parts.next().unwrap_or("");
        let langs = parts.next().unwrap_or("");
        let entry = families.entry(name).or_insert_with(|| FontEntry {
            name: Box::leak(name.to_owned().into_boxed_str()),
            cjk: false,
            has_bold: false,
        });
        entry.cjk |= langs
            .split('|')
            .any(|tag| tag.starts_with("zh-") || tag == "zh");
        entry.has_bold |= style.to_lowercase().contains("bold");
    }

    let mut entries: Vec<FontEntry> = families.into_values().collect();
    entries.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(right.name))
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Noto Sans CJK SC\tRegular\tzh-cn|ja|ko
Noto Sans CJK SC\tBold\tzh-cn|ja|ko
DejaVu Sans\tBook\t
DejaVu Sans\tBold Oblique\t
.AquaBase\tRegular\t
 WenQuanYi Zen Hei \tRegular\tzh-tw
";

    #[test]
    fn parser_dedupes_families_and_detects_cjk_and_bold() {
        let fonts = parse_fc_list(SAMPLE);

        let noto = fonts
            .iter()
            .find(|font| font.name == "Noto Sans CJK SC")
            .expect("noto family");
        assert!(noto.cjk);
        assert!(noto.has_bold);

        let dejavu = fonts
            .iter()
            .find(|font| font.name == "DejaVu Sans")
            .expect("dejavu family");
        assert!(!dejavu.cjk);
        assert!(dejavu.has_bold);

        let wenquanyi = fonts
            .iter()
            .find(|font| font.name == "WenQuanYi Zen Hei")
            .expect("trimmed family name");
        assert!(wenquanyi.cjk);
        assert!(!wenquanyi.has_bold);

        assert_eq!(
            fonts.len(),
            3,
            "hidden aliases and duplicate family lines collapse: {fonts:?}"
        );
    }

    #[test]
    fn parser_sorts_names_case_insensitively() {
        let fonts = parse_fc_list("Zeta\tRegular\t\nAlpha\tRegular\t\nalpha two\tRegular\t\n");
        let names: Vec<&str> = fonts.iter().map(|font| font.name).collect();
        assert_eq!(names, ["Alpha", "alpha two", "Zeta"]);
    }

    #[test]
    fn parser_skips_empty_family_names() {
        let fonts = parse_fc_list("\tRegular\tzh-cn\n\n");
        assert!(fonts.is_empty());
    }

    #[test]
    fn matching_is_case_insensitive() {
        let font = FontEntry {
            name: "Noto Sans CJK SC",
            cjk: true,
            has_bold: true,
        };
        assert!(font.matches(""));
        assert!(font.matches("noto"));
        assert!(font.matches("CJK sc"));
        assert!(!font.matches("dejavu"));
    }
}
