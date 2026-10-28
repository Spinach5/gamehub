//! AppError 的本地化渲染。

use super::{Lang, tr, tr_args, tr_plural_args};
use application::AppError;

pub fn app_error(lang: Lang, error: &AppError) -> String {
    match error {
        AppError::DatabaseLocked => tr(lang, "errors.database_locked").to_owned(),
        AppError::ExecutableMissing(path) => tr_args(
            lang,
            "errors.executable_missing",
            &[("path", &path.display().to_string())],
        ),
        AppError::WorkingDirMissing(path) => tr_args(
            lang,
            "errors.working_dir_missing",
            &[("path", &path.display().to_string())],
        ),
        AppError::GameNotFound => tr(lang, "errors.game_not_found").to_owned(),
        AppError::GameAlreadyRunning => tr(lang, "errors.game_already_running").to_owned(),
        AppError::GameRunning => tr(lang, "errors.game_running").to_owned(),
        AppError::PrefixInUseByRunningGame => tr(lang, "errors.prefix_running_game").to_owned(),
        AppError::PrefixShared => tr(lang, "errors.prefix_shared").to_owned(),
        AppError::PrefixUsedByOtherGame => tr(lang, "errors.prefix_used_by_other").to_owned(),
        AppError::DefaultRunnerInUse => tr(lang, "errors.default_runner_in_use").to_owned(),
        AppError::RunnerInUse(count) => tr_plural_args(
            lang,
            "errors.runner_in_use.one",
            "errors.runner_in_use.other",
            *count,
            &[("count", &count.to_string())],
        ),
        AppError::DxvkNvapiSourceMissing => tr(lang, "errors.dxvk_nvapi_source_missing").to_owned(),
        AppError::CoverUnsupportedFormat(path) => tr_args(
            lang,
            "errors.cover_unsupported",
            &[("path", &path.display().to_string())],
        ),
        AppError::CoverInvalidImage { path, detail } => tr_args(
            lang,
            "errors.cover_invalid",
            &[("path", &path.display().to_string()), ("detail", detail)],
        ),
        AppError::CoverTooLarge { width, height } => tr_args(
            lang,
            "errors.cover_too_large",
            &[
                ("width", &width.to_string()),
                ("height", &height.to_string()),
            ],
        ),
        AppError::CoverCopy(source) => tr_args(
            lang,
            "errors.cover_copy",
            &[("detail", &source.to_string())],
        ),
        AppError::ProcessWait(source) => tr_args(
            lang,
            "errors.process_wait",
            &[("detail", &source.to_string())],
        ),
        AppError::Stop(source) => tr_args(lang, "errors.stop", &[("detail", &source.to_string())]),
        AppError::Storage(source) => {
            format!("{}: {source}", tr(lang, "errors.storage"))
        }
        AppError::Prefix(source) => {
            format!("{}: {source}", tr(lang, "errors.prefix"))
        }
        AppError::Runner(source) => {
            format!("{}: {source}", tr(lang, "errors.runner"))
        }
        AppError::Launch(source) => {
            format!("{}: {source}", tr(lang, "errors.launch"))
        }
        AppError::Join(source) => tr_args(lang, "errors.join", &[("detail", &source.to_string())]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{EN_US, ZH_CN};
    use std::path::PathBuf;

    #[test]
    fn static_errors_are_translated() {
        assert_eq!(
            app_error(&ZH_CN, &application::AppError::GameNotFound),
            "游戏不存在"
        );
        assert_eq!(
            app_error(&EN_US, &application::AppError::GameNotFound),
            "Game not found"
        );
    }

    #[test]
    fn parameterized_errors_fill_placeholders() {
        let error = application::AppError::ExecutableMissing(PathBuf::from("/games/a.exe"));
        assert_eq!(
            app_error(&EN_US, &error),
            "Game executable not found: /games/a.exe"
        );
    }

    #[test]
    fn count_errors_use_plural_forms() {
        assert_eq!(
            app_error(&EN_US, &application::AppError::RunnerInUse(1)),
            "1 game still uses this runner"
        );
        assert_eq!(
            app_error(&EN_US, &application::AppError::RunnerInUse(3)),
            "3 games still use this runner"
        );
    }
}
