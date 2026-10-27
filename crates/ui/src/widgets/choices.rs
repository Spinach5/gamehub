use application::MonitorOutput;
use launcher_core::{Arch, DisplayBackend, GamescopeMode, OverrideMode, Runner};
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

pub fn arch_choices() -> Vec<Choice<Arch>> {
    vec![
        Choice::new(Arch::Win64, "64 位 (win64)"),
        Choice::new(Arch::Win32, "32 位 (win32)"),
    ]
}

pub fn arch_choice(arch: Arch) -> Choice<Arch> {
    match arch {
        Arch::Win64 => Choice::new(arch, "64 位 (win64)"),
        Arch::Win32 => Choice::new(arch, "32 位 (win32)"),
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

pub fn backend_choices() -> Vec<Choice<DisplayBackend>> {
    vec![
        Choice::new(DisplayBackend::Auto, "自动"),
        Choice::new(DisplayBackend::X11, "X11"),
        Choice::new(DisplayBackend::Wayland, "Wayland"),
    ]
}

pub fn backend_choice(value: DisplayBackend) -> Choice<DisplayBackend> {
    backend_choices()
        .into_iter()
        .find(|choice| choice.value == value)
        .unwrap_or_else(|| Choice::new(DisplayBackend::Auto, "自动"))
}

pub fn gamescope_choices() -> Vec<Choice<GamescopeMode>> {
    vec![
        Choice::new(GamescopeMode::Off, "关闭"),
        Choice::new(GamescopeMode::Auto, "自动（Wayland 且可用时）"),
        Choice::new(GamescopeMode::Forced, "强制"),
    ]
}

pub fn gamescope_choice(value: GamescopeMode) -> Choice<GamescopeMode> {
    gamescope_choices()
        .into_iter()
        .find(|choice| choice.value == value)
        .unwrap_or_else(|| Choice::new(GamescopeMode::Auto, "自动（Wayland 且可用时）"))
}

pub fn override_mode_choices() -> Vec<Choice<OverrideMode>> {
    vec![
        Choice::new(OverrideMode::Disabled, "禁用 (disabled)"),
        Choice::new(OverrideMode::Native, "原生 (n)"),
        Choice::new(OverrideMode::Builtin, "内置 (b)"),
        Choice::new(OverrideMode::NativeBuiltin, "原生+内置 (n,b)"),
    ]
}

pub fn override_mode_choice(value: OverrideMode) -> Choice<OverrideMode> {
    override_mode_choices()
        .into_iter()
        .find(|choice| choice.value == value)
        .unwrap_or_else(|| Choice::new(OverrideMode::NativeBuiltin, "原生+内置 (n,b)"))
}

pub fn monitor_choices(outputs: &[MonitorOutput]) -> Vec<Choice<Option<String>>> {
    let mut choices = vec![Choice::new(None, "（自动）")];
    choices.extend(
        outputs
            .iter()
            .filter(|output| output.connected)
            .map(|output| Choice::new(Some(output.name.clone()), monitor_label(output))),
    );
    choices
}

pub fn monitor_choice(
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
        _ => Choice::new(None, "（自动）"),
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
