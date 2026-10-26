use launcher_core::{DllOverride, OverrideMode};

fn mode_value(mode: OverrideMode) -> &'static str {
    match mode {
        OverrideMode::Disabled => "disabled",
        OverrideMode::Native => "n",
        OverrideMode::Builtin => "b",
        OverrideMode::NativeBuiltin => "n,b",
    }
}

pub fn build_dll_overrides(overrides: &[DllOverride]) -> String {
    overrides
        .iter()
        .filter_map(|entry| {
            let name = entry.dll_name.trim();
            if name.is_empty() {
                None
            } else {
                Some(format!("{name}={}", mode_value(entry.mode)))
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{DllOverride, OverrideMode};

    #[test]
    fn all_modes_map_to_wine_syntax() {
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "a".into(),
                mode: OverrideMode::Disabled,
            }]),
            "a=disabled"
        );
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "b".into(),
                mode: OverrideMode::Native,
            }]),
            "b=n"
        );
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "c".into(),
                mode: OverrideMode::Builtin,
            }]),
            "c=b"
        );
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "d".into(),
                mode: OverrideMode::NativeBuiltin,
            }]),
            "d=n,b"
        );
    }

    #[test]
    fn multiple_overrides_are_semicolon_separated_in_input_order() {
        let overrides = vec![
            DllOverride {
                dll_name: "winmm".into(),
                mode: OverrideMode::NativeBuiltin,
            },
            DllOverride {
                dll_name: "steam_api64".into(),
                mode: OverrideMode::Native,
            },
        ];
        assert_eq!(build_dll_overrides(&overrides), "winmm=n,b;steam_api64=n");
    }

    #[test]
    fn empty_names_are_skipped() {
        let overrides = vec![
            DllOverride {
                dll_name: "  ".into(),
                mode: OverrideMode::Native,
            },
            DllOverride {
                dll_name: " winmm ".into(),
                mode: OverrideMode::Builtin,
            },
        ];
        assert_eq!(build_dll_overrides(&overrides), "winmm=b");
    }
}
