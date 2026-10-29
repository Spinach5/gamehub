//! AppError 的本地化渲染。

use super::{Lang, tr, tr_args, tr_plural_args};
use application::AppError;
use application::editor::{DraftError, DraftErrorKind, DraftField};

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
        AppError::ComponentVerbInvalid(verb) => {
            tr_args(lang, "errors.component_verb_invalid", &[("verb", verb)])
        }
        AppError::ComponentIo(source) => tr_args(
            lang,
            "errors.component_io",
            &[("detail", &source.to_string())],
        ),
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

fn draft_field(lang: Lang, field: DraftField) -> &'static str {
    match field {
        DraftField::Name => tr(lang, "draft.field.name"),
        DraftField::Executable => tr(lang, "draft.field.executable"),
        DraftField::WorkingDir => tr(lang, "draft.field.working_dir"),
        DraftField::Prefix => tr(lang, "draft.field.prefix"),
        DraftField::Resolution => tr(lang, "draft.field.resolution"),
        DraftField::RefreshRate => tr(lang, "draft.field.refresh_rate"),
        DraftField::Env => tr(lang, "draft.field.env"),
        DraftField::DllOverride => tr(lang, "draft.field.dll"),
    }
}

pub fn draft_error(lang: Lang, error: &DraftError) -> String {
    let message = match &error.kind {
        DraftErrorKind::NameEmpty => tr(lang, "draft.error.name_empty").to_owned(),
        DraftErrorKind::ExecutableMissing => tr(lang, "draft.error.executable_missing").to_owned(),
        DraftErrorKind::ExecutableNotAbsolute => {
            tr(lang, "draft.error.executable_not_absolute").to_owned()
        }
        DraftErrorKind::WorkingDirNotAbsolute => {
            tr(lang, "draft.error.working_dir_not_absolute").to_owned()
        }
        DraftErrorKind::PrefixNotAbsolute => tr(lang, "draft.error.prefix_not_absolute").to_owned(),
        DraftErrorKind::ResolutionPartial => tr(lang, "draft.error.resolution_partial").to_owned(),
        DraftErrorKind::ResolutionWidthInvalid => {
            tr(lang, "draft.error.resolution_width").to_owned()
        }
        DraftErrorKind::ResolutionHeightInvalid => {
            tr(lang, "draft.error.resolution_height").to_owned()
        }
        DraftErrorKind::ResolutionZero => tr(lang, "draft.error.resolution_zero").to_owned(),
        DraftErrorKind::RefreshRateInvalid => {
            tr(lang, "draft.error.refresh_rate_invalid").to_owned()
        }
        DraftErrorKind::RefreshRateZero => tr(lang, "draft.error.refresh_rate_zero").to_owned(),
        DraftErrorKind::EnvKeyEmpty => tr(lang, "draft.error.env_key_empty").to_owned(),
        DraftErrorKind::EnvReservedOverride => {
            tr(lang, "draft.error.env_reserved_override").to_owned()
        }
        DraftErrorKind::EnvKeyInvalid { key } => {
            tr_args(lang, "draft.error.env_key_invalid", &[("key", key)])
        }
        DraftErrorKind::EnvDuplicate { key } => {
            tr_args(lang, "draft.error.env_duplicate", &[("key", key)])
        }
        DraftErrorKind::DllNameInvalid { name } => {
            tr_args(lang, "draft.error.dll_name_invalid", &[("name", name)])
        }
        DraftErrorKind::DllDuplicate { name } => {
            tr_args(lang, "draft.error.dll_duplicate", &[("name", name)])
        }
    };
    tr_args(
        lang,
        "draft.format",
        &[
            ("field", draft_field(lang, error.field)),
            ("message", &message),
        ],
    )
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

    #[test]
    fn draft_errors_render_in_both_languages() {
        let error = application::editor::DraftError {
            field: application::editor::DraftField::Resolution,
            kind: application::editor::DraftErrorKind::ResolutionPartial,
        };
        assert_eq!(
            draft_error(&ZH_CN, &error),
            "分辨率：宽和高必须同时填写或同时留空"
        );
        assert_eq!(
            draft_error(&EN_US, &error),
            "Resolution: width and height must both be set or both be empty"
        );
    }

    #[test]
    fn duplicate_env_name_is_reported_in_english() {
        let rows = vec![
            application::editor::EnvRow {
                key: "A".into(),
                value: "1".into(),
            },
            application::editor::EnvRow {
                key: "A".into(),
                value: "2".into(),
            },
        ];
        let error = application::editor::env_rows_to_map(&rows).unwrap_err();
        assert_eq!(
            draft_error(&EN_US, &error),
            "Environment variables: Duplicate variable A"
        );
    }

    #[test]
    fn invalid_env_key_is_rendered_without_app_layer_text() {
        let rows = vec![application::editor::EnvRow {
            key: "1BAD".into(),
            value: "1".into(),
        }];
        let error = application::editor::env_rows_to_map(&rows).unwrap_err();
        assert_eq!(
            draft_error(&ZH_CN, &error),
            "环境变量：环境变量名无效：1BAD"
        );
        assert_eq!(
            draft_error(&EN_US, &error),
            "Environment variables: Invalid environment variable name: 1BAD"
        );
    }
}
