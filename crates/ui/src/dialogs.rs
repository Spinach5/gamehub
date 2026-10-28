use crate::app::Message;
use iced::Task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogIntent {
    PickExe,
    PickWorkingDir,
    PickProtonDir,
    PickPrefixDir,
    PickDxvkDir,
    PickDxvkNvapiDir,
    PickPrefixRoot,
    PickCover,
}

fn dialog_impl(intent: DialogIntent, lang: crate::i18n::Lang) -> Option<std::path::PathBuf> {
    let dialog = rfd::FileDialog::new();
    match intent {
        DialogIntent::PickExe => dialog
            .add_filter(crate::i18n::tr(lang, "dialogs.filter.windows"), &["exe"])
            .pick_file(),
        DialogIntent::PickCover => dialog
            .add_filter(
                crate::i18n::tr(lang, "dialogs.filter.cover"),
                &["png", "jpg", "jpeg", "webp", "bmp"],
            )
            .pick_file(),
        DialogIntent::PickWorkingDir
        | DialogIntent::PickProtonDir
        | DialogIntent::PickPrefixDir
        | DialogIntent::PickDxvkDir
        | DialogIntent::PickDxvkNvapiDir
        | DialogIntent::PickPrefixRoot => dialog.pick_folder(),
    }
}

pub fn pick(intent: DialogIntent, lang: crate::i18n::Lang) -> Task<Message> {
    Task::perform(
        async move {
            match tokio::task::spawn_blocking(move || dialog_impl(intent, lang)).await {
                Ok(path) => Some((intent, path)),
                Err(_) => None,
            }
        },
        move |result| match result {
            Some((intent, path)) => Message::DialogResult { intent, path },
            None => Message::Error(crate::i18n::tr(lang, "dialogs.error.failed").to_owned()),
        },
    )
}
