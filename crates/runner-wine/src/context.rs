use launcher_core::{Arch, Runner, RunnerKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::process::{Child, Command};

use crate::RunnerError;

#[derive(Debug, Clone)]
pub struct WineContext {
    pub runner: Runner,
    pub prefix: PathBuf,
    pub arch: Arch,
    pub steam_compat_client: PathBuf,
}

impl WineContext {
    pub fn base_env(&self) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        match self.runner.kind {
            RunnerKind::Wine => {
                env.insert(
                    "WINEPREFIX".into(),
                    self.prefix.to_string_lossy().into_owned(),
                );
                env.insert("WINEARCH".into(), self.arch.as_str().into());
                env.insert("WINEDEBUG".into(), "-all".into());
            }
            RunnerKind::Proton => {
                env.insert(
                    "STEAM_COMPAT_DATA_PATH".into(),
                    self.prefix.to_string_lossy().into_owned(),
                );
                env.insert(
                    "STEAM_COMPAT_CLIENT_INSTALL_PATH".into(),
                    self.steam_compat_client.to_string_lossy().into_owned(),
                );
                env.insert(
                    "PROTONPATH".into(),
                    self.runner.root_dir.to_string_lossy().into_owned(),
                );
            }
        }
        env
    }

    pub fn build_command(&self, exe: &Path, args: &[String]) -> (PathBuf, Vec<String>) {
        match self.runner.kind {
            RunnerKind::Wine => {
                let mut command_args = Vec::with_capacity(args.len() + 1);
                command_args.push(exe.to_string_lossy().into_owned());
                command_args.extend(args.iter().cloned());
                (self.runner.binary_path.clone(), command_args)
            }
            RunnerKind::Proton => {
                let mut command_args = Vec::with_capacity(args.len() + 2);
                command_args.push("run".into());
                command_args.push(exe.to_string_lossy().into_owned());
                command_args.extend(args.iter().cloned());
                (self.runner.binary_path.clone(), command_args)
            }
        }
    }

    pub async fn spawn(
        &self,
        exe: &Path,
        args: &[String],
        extra_env: &BTreeMap<String, String>,
    ) -> Result<Child, RunnerError> {
        let (program, command_args) = self.build_command(exe, args);
        let mut command = Command::new(&program);
        command
            .args(&command_args)
            .envs(self.base_env())
            .envs(extra_env)
            .kill_on_drop(false);
        command
            .spawn()
            .map_err(|source| RunnerError::Spawn { program, source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, RunnerKind, RunnerSource};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn wine_context() -> WineContext {
        WineContext {
            runner: Runner {
                id: Uuid::new_v4(),
                kind: RunnerKind::Wine,
                name: "System Wine".into(),
                binary_path: PathBuf::from("/usr/bin/wine"),
                root_dir: PathBuf::from("/usr/bin"),
                arch: Arch::Win64,
                source: RunnerSource::System,
                verified: true,
                version_string: Some("wine-10.0".into()),
            },
            prefix: PathBuf::from("/data/prefixes/game"),
            arch: Arch::Win64,
            steam_compat_client: PathBuf::from("/data/steam-compat-client"),
        }
    }

    fn proton_context() -> WineContext {
        WineContext {
            runner: Runner {
                id: Uuid::new_v4(),
                kind: RunnerKind::Proton,
                name: "GE-Proton9-1".into(),
                binary_path: PathBuf::from("/opt/GE-Proton9-1/proton"),
                root_dir: PathBuf::from("/opt/GE-Proton9-1"),
                arch: Arch::Win64,
                source: RunnerSource::Imported {
                    original_path: PathBuf::from("/opt/GE-Proton9-1"),
                },
                verified: true,
                version_string: Some("GE-Proton9-1".into()),
            },
            prefix: PathBuf::from("/data/prefixes/game"),
            arch: Arch::Win64,
            steam_compat_client: PathBuf::from("/data/steam-compat-client"),
        }
    }

    #[test]
    fn wine_base_env_is_exact() {
        let env = wine_context().base_env();
        assert_eq!(env.len(), 3);
        assert_eq!(env["WINEPREFIX"], "/data/prefixes/game");
        assert_eq!(env["WINEARCH"], "win64");
        assert_eq!(env["WINEDEBUG"], "-all");
        assert!(!env.contains_key("PROTONPATH"));
    }

    #[test]
    fn wine_win32_arch_is_reflected() {
        let mut context = wine_context();
        context.arch = Arch::Win32;
        assert_eq!(context.base_env()["WINEARCH"], "win32");
    }

    #[test]
    fn proton_base_env_is_exact() {
        let env = proton_context().base_env();
        assert_eq!(env.len(), 3);
        assert_eq!(env["STEAM_COMPAT_DATA_PATH"], "/data/prefixes/game");
        assert_eq!(
            env["STEAM_COMPAT_CLIENT_INSTALL_PATH"],
            "/data/steam-compat-client"
        );
        assert_eq!(env["PROTONPATH"], "/opt/GE-Proton9-1");
        assert!(!env.contains_key("WINEPREFIX"));
    }

    #[test]
    fn wine_command_is_binary_plus_exe_and_args() {
        let context = wine_context();
        let (program, args) = context.build_command(
            &PathBuf::from("C:/Games/game.exe"),
            &["--windowed".to_string(), "-skip".to_string()],
        );
        assert_eq!(program, PathBuf::from("/usr/bin/wine"));
        assert_eq!(
            args,
            vec![
                "C:/Games/game.exe".to_string(),
                "--windowed".to_string(),
                "-skip".to_string()
            ]
        );
    }

    #[test]
    fn proton_command_uses_run_subcommand() {
        let context = proton_context();
        let (program, args) =
            context.build_command(&PathBuf::from("C:/Games/game.exe"), &["-skip".to_string()]);
        assert_eq!(program, PathBuf::from("/opt/GE-Proton9-1/proton"));
        assert_eq!(
            args,
            vec![
                "run".to_string(),
                "C:/Games/game.exe".to_string(),
                "-skip".to_string()
            ]
        );
    }

    #[test]
    fn build_command_does_not_touch_the_filesystem() {
        let context = proton_context();
        let (program, _) = context.build_command(&PathBuf::from("/does/not/exist.exe"), &[]);
        assert_eq!(program, PathBuf::from("/opt/GE-Proton9-1/proton"));
    }
}
