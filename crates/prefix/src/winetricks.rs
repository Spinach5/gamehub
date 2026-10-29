use crate::{PrefixError, PrefixPlan, windows_root};
use launcher_core::RunnerKind;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const MAX_VERB_LEN: usize = 64;

/// Verbs that control winetricks itself instead of installing a component.
/// Only the system winetricks is called, so these would act on the whole
/// installation rather than on the selected prefix.
const RESERVED_VERBS: &[&str] = &[
    "apps",
    "benchmarks",
    "gui",
    "help",
    "list-all",
    "list-categories",
    "list-dlls",
    "list-installed",
    "self-update",
    "settings",
    "show-menu",
    "upgrade",
    "version",
    "wineprefixcreate",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinetricksCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

pub fn winetricks_path() -> Option<PathBuf> {
    winetricks_path_in(std::env::var_os("PATH").as_deref())
}

pub fn winetricks_path_in(path_env: Option<&OsStr>) -> Option<PathBuf> {
    let path_env = path_env?;
    std::env::split_paths(path_env)
        .map(|dir| dir.join("winetricks"))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Trims and validates a verb. Leading dashes are rejected so a verb can never
/// be smuggled in as a winetricks flag.
pub fn normalize_verb(verb: &str) -> Result<String, PrefixError> {
    let verb = verb.trim();
    let valid = !verb.is_empty()
        && verb.len() <= MAX_VERB_LEN
        && !verb.starts_with('-')
        && verb.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-' | b'=')
        })
        && !RESERVED_VERBS.contains(&verb);
    if valid {
        Ok(verb.to_owned())
    } else {
        Err(PrefixError::WinetricksVerbInvalid(verb.to_owned()))
    }
}

/// Reads the verbs winetricks recorded in the prefix itself. The file is only
/// a hint: components can be installed or removed outside gameHub.
pub fn installed_verbs(plan: &PrefixPlan) -> Vec<String> {
    let log = windows_root(&plan.path, plan.runner.kind).join("winetricks.log");
    let Ok(text) = std::fs::read_to_string(log) else {
        return Vec::new();
    };
    let mut verbs: Vec<String> = Vec::new();
    for token in text.split_whitespace() {
        if token.starts_with("w_workaround") {
            continue;
        }
        if !verbs.iter().any(|verb| verb == token) {
            verbs.push(token.to_owned());
        }
    }
    verbs
}

pub fn command_for(plan: &PrefixPlan, winetricks: &Path, verb: &str) -> WinetricksCommand {
    let mut env = BTreeMap::new();
    match plan.runner.kind {
        RunnerKind::Wine => {
            env.insert(
                "WINEPREFIX".into(),
                plan.path.to_string_lossy().into_owned(),
            );
            env.insert("WINEARCH".into(), plan.arch.as_str().into());
            env.insert(
                "WINE".into(),
                plan.runner.binary_path.to_string_lossy().into_owned(),
            );
        }
        RunnerKind::Proton => {
            // Proton keeps the actual Wine prefix under pfx/ and exposes wine
            // inside its own files/ tree; winetricks needs both to run at all.
            env.insert(
                "WINEPREFIX".into(),
                windows_root(&plan.path, RunnerKind::Proton)
                    .to_string_lossy()
                    .into_owned(),
            );
            env.insert(
                "WINE".into(),
                plan.runner
                    .root_dir
                    .join("files/bin/wine")
                    .to_string_lossy()
                    .into_owned(),
            );
            env.insert(
                "STEAM_COMPAT_DATA_PATH".into(),
                plan.path.to_string_lossy().into_owned(),
            );
            env.insert(
                "STEAM_COMPAT_CLIENT_INSTALL_PATH".into(),
                plan.steam_compat_client.to_string_lossy().into_owned(),
            );
            env.insert(
                "PROTONPATH".into(),
                plan.runner.root_dir.to_string_lossy().into_owned(),
            );
        }
    }
    WinetricksCommand {
        program: winetricks.to_path_buf(),
        args: vec!["-q".into(), verb.into()],
        env,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, Runner, RunnerKind, RunnerSource};
    use std::fs;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn runner(kind: RunnerKind, root: &Path) -> Runner {
        Runner {
            id: Uuid::new_v4(),
            kind,
            name: "test".into(),
            binary_path: match kind {
                RunnerKind::Wine => root.join("bin/wine"),
                RunnerKind::Proton => root.join("proton"),
            },
            root_dir: root.to_path_buf(),
            arch: Arch::Win64,
            source: RunnerSource::System,
            verified: true,
            version_string: None,
        }
    }

    fn plan(dir: &Path, kind: RunnerKind) -> PrefixPlan {
        PrefixPlan::new(
            dir.join("prefix"),
            runner(kind, dir),
            Arch::Win64,
            dir.join("steam-client"),
        )
    }

    fn write_executable(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn finds_only_executable_winetricks_on_path() {
        let dir = tempdir().unwrap();
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        fs::write(plain.join("winetricks"), "not executable").unwrap();

        let bin = dir.path().join("bin");
        write_executable(&bin.join("winetricks"));

        let path_env = format!("{}:{}", plain.display(), bin.display());
        assert_eq!(
            winetricks_path_in(Some(OsStr::new(&path_env))),
            Some(bin.join("winetricks"))
        );
        assert_eq!(winetricks_path_in(None), None);
    }

    #[test]
    fn normalizes_valid_verbs() {
        assert_eq!(normalize_verb(" vcrun2019 ").unwrap(), "vcrun2019");
        assert_eq!(normalize_verb("fontsmooth-rgb").unwrap(), "fontsmooth-rgb");
        assert_eq!(normalize_verb("dotnet48").unwrap(), "dotnet48");
    }

    #[test]
    fn rejects_flags_commands_and_bad_characters() {
        let long = "a".repeat(MAX_VERB_LEN + 1);
        for verb in [
            "",
            "  ",
            "--force",
            "-q",
            "VCrun2019",
            "vcrun 2019",
            "vcrun2019;rm",
            "list-all",
            "wineprefixcreate",
            long.as_str(),
        ] {
            assert!(
                matches!(
                    normalize_verb(verb),
                    Err(PrefixError::WinetricksVerbInvalid(_))
                ),
                "expected {verb:?} to be rejected"
            );
        }
    }

    #[test]
    fn wine_command_targets_the_plan_prefix() {
        let dir = tempdir().unwrap();
        let plan = plan(dir.path(), RunnerKind::Wine);
        let command = command_for(&plan, Path::new("/usr/bin/winetricks"), "vcrun2019");

        assert_eq!(command.program, PathBuf::from("/usr/bin/winetricks"));
        assert_eq!(command.args, vec!["-q", "vcrun2019"]);
        assert_eq!(
            command.env["WINEPREFIX"],
            dir.path().join("prefix").to_string_lossy()
        );
        assert_eq!(command.env["WINEARCH"], "win64");
        assert_eq!(
            command.env["WINE"],
            dir.path().join("bin/wine").to_string_lossy()
        );
        assert!(!command.env.contains_key("STEAM_COMPAT_DATA_PATH"));
    }

    #[test]
    fn proton_command_uses_pfx_and_the_proton_wine() {
        let dir = tempdir().unwrap();
        let plan = plan(dir.path(), RunnerKind::Proton);
        let command = command_for(&plan, Path::new("/usr/bin/winetricks"), "dotnet48");

        assert_eq!(
            command.env["WINEPREFIX"],
            dir.path().join("prefix/pfx").to_string_lossy()
        );
        assert_eq!(
            command.env["WINE"],
            dir.path().join("files/bin/wine").to_string_lossy()
        );
        assert_eq!(command.env["PROTONPATH"], dir.path().to_string_lossy());
        assert_eq!(
            command.env["STEAM_COMPAT_DATA_PATH"],
            dir.path().join("prefix").to_string_lossy()
        );
        assert!(!command.env.contains_key("WINEARCH"));
    }

    #[test]
    fn installed_verbs_are_read_deduplicated_and_scoped_to_the_runner() {
        let dir = tempdir().unwrap();
        let proton_plan = plan(dir.path(), RunnerKind::Proton);
        fs::create_dir_all(proton_plan.path.join("pfx")).unwrap();
        fs::write(
            proton_plan.path.join("pfx/winetricks.log"),
            "vcrun2019\ncorefonts\nvcrun2019\n",
        )
        .unwrap();

        assert_eq!(
            installed_verbs(&proton_plan),
            vec!["vcrun2019".to_string(), "corefonts".to_string()]
        );

        let wine_plan = plan(dir.path(), RunnerKind::Wine);
        assert!(installed_verbs(&wine_plan).is_empty());
    }
}
