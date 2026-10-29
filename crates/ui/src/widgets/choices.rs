use crate::i18n::{Lang, tr};
use application::{Arch, DisplayBackend, GamescopeMode, MonitorOutput, OverrideMode, Runner};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice<T> {
    pub value: T,
    pub label: String,
}

impl<T> Choice<T> {
    pub fn new(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

impl<T> fmt::Display for Choice<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label)
    }
}

pub fn arch_choices(lang: Lang) -> Vec<Choice<Arch>> {
    vec![
        Choice::new(Arch::Win64, tr(lang, "choices.arch.win64")),
        Choice::new(Arch::Win32, tr(lang, "choices.arch.win32")),
    ]
}

pub fn arch_choice(lang: Lang, arch: Arch) -> Choice<Arch> {
    match arch {
        Arch::Win64 => Choice::new(arch, tr(lang, "choices.arch.win64")),
        Arch::Win32 => Choice::new(arch, tr(lang, "choices.arch.win32")),
    }
}

pub fn runner_choices(runners: &[Runner], none_label: &str) -> Vec<Choice<Option<Uuid>>> {
    let mut choices = vec![Choice::new(None, none_label)];
    choices.extend(
        runners
            .iter()
            .map(|runner| Choice::new(Some(runner.id), runner.name.clone())),
    );
    choices
}

pub fn runner_choice(
    runners: &[Runner],
    none_label: &str,
    selected: Option<Uuid>,
) -> Choice<Option<Uuid>> {
    match selected {
        Some(id) => runners
            .iter()
            .find(|runner| runner.id == id)
            .map(|runner| Choice::new(Some(id), runner.name.clone()))
            .unwrap_or_else(|| Choice::new(None, none_label)),
        None => Choice::new(None, none_label),
    }
}

pub fn backend_choices(lang: Lang) -> Vec<Choice<DisplayBackend>> {
    vec![
        Choice::new(DisplayBackend::Auto, tr(lang, "common.auto")),
        Choice::new(DisplayBackend::X11, "X11"),
        Choice::new(DisplayBackend::Wayland, "Wayland"),
    ]
}

pub fn backend_choice(lang: Lang, value: DisplayBackend) -> Choice<DisplayBackend> {
    backend_choices(lang)
        .into_iter()
        .find(|choice| choice.value == value)
        .unwrap_or_else(|| Choice::new(DisplayBackend::Auto, tr(lang, "common.auto")))
}

pub fn gamescope_choices(lang: Lang) -> Vec<Choice<GamescopeMode>> {
    vec![
        Choice::new(GamescopeMode::Off, tr(lang, "choices.gamescope.off")),
        Choice::new(GamescopeMode::Auto, tr(lang, "choices.gamescope.auto")),
        Choice::new(GamescopeMode::Forced, tr(lang, "choices.gamescope.forced")),
    ]
}

pub fn gamescope_choice(lang: Lang, value: GamescopeMode) -> Choice<GamescopeMode> {
    gamescope_choices(lang)
        .into_iter()
        .find(|choice| choice.value == value)
        .unwrap_or_else(|| Choice::new(GamescopeMode::Auto, tr(lang, "choices.gamescope.auto")))
}

pub fn override_mode_choices(lang: Lang) -> Vec<Choice<OverrideMode>> {
    vec![
        Choice::new(
            OverrideMode::Disabled,
            tr(lang, "choices.override.disabled"),
        ),
        Choice::new(OverrideMode::Native, tr(lang, "choices.override.native")),
        Choice::new(OverrideMode::Builtin, tr(lang, "choices.override.builtin")),
        Choice::new(
            OverrideMode::NativeBuiltin,
            tr(lang, "choices.override.native_builtin"),
        ),
    ]
}

pub fn override_mode_choice(lang: Lang, value: OverrideMode) -> Choice<OverrideMode> {
    override_mode_choices(lang)
        .into_iter()
        .find(|choice| choice.value == value)
        .unwrap_or_else(|| {
            Choice::new(
                OverrideMode::NativeBuiltin,
                tr(lang, "choices.override.native_builtin"),
            )
        })
}

pub fn component_choices(lang: Lang) -> Vec<Choice<String>> {
    [
        ("vcrun2019", "detail.component.vcrun2019"),
        ("dotnet48", "detail.component.dotnet48"),
        ("corefonts", "detail.component.corefonts"),
        ("d3dx9", "detail.component.d3dx9"),
        ("xact", "detail.component.xact"),
        ("msxml6", "detail.component.msxml6"),
    ]
    .into_iter()
    .map(|(verb, key)| Choice::new(verb.to_owned(), tr(lang, key)))
    .collect()
}

pub fn component_choice(lang: Lang, verb: &str) -> Choice<String> {
    component_choices(lang)
        .into_iter()
        .find(|choice| choice.value == verb)
        .unwrap_or_else(|| Choice::new(verb.to_owned(), verb.to_owned()))
}

pub fn monitor_choices(lang: Lang, outputs: &[MonitorOutput]) -> Vec<Choice<Option<String>>> {
    let mut choices = vec![Choice::new(None, tr(lang, "choices.monitor.auto"))];
    choices.extend(
        outputs
            .iter()
            .filter(|output| output.connected)
            .map(|output| Choice::new(Some(output.name.clone()), monitor_label(output))),
    );
    choices
}

pub fn monitor_choice(
    lang: Lang,
    outputs: &[MonitorOutput],
    selected: Option<String>,
) -> Choice<Option<String>> {
    match selected {
        Some(name)
            if outputs
                .iter()
                .any(|output| output.connected && output.name == name) =>
        {
            let label = outputs
                .iter()
                .find(|output| output.name == name)
                .map(monitor_label)
                .unwrap_or_else(|| name.clone());
            Choice::new(Some(name), label)
        }
        _ => Choice::new(None, tr(lang, "choices.monitor.auto")),
    }
}

fn monitor_label(output: &MonitorOutput) -> String {
    match output.current_mode {
        Some((width, height, Some(refresh))) => {
            format!("{} ({}x{} @ {}Hz)", output.name, width, height, refresh)
        }
        Some((width, height, None)) => format!("{} ({}x{})", output.name, width, height),
        None => output.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_are_translated() {
        use crate::i18n::{EN_US, ZH_CN};
        assert_eq!(arch_choices(&ZH_CN)[0].label, "64 位 (win64)");
        assert_eq!(arch_choices(&EN_US)[0].label, "64-bit (win64)");
        assert_eq!(backend_choices(&EN_US)[0].label, "Automatic");
        assert_eq!(component_choices(&ZH_CN)[0].value, "vcrun2019");
        assert_eq!(
            component_choices(&EN_US)[0].label,
            "Visual C++ runtime 2015-2019"
        );
        assert_eq!(component_choice(&EN_US, "customverb").value, "customverb");
    }
}
