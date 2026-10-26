use crate::app::Message;
use iced::Task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogIntent {
    PickExe,
    PickWorkingDir,
    PickProtonDir,
    PickPrefixDir,
    PickDxvkDir,
    PickPrefixRoot,
}

fn dialog_impl(intent: DialogIntent) -> Option<std::path::PathBuf> {
    let dialog = rfd::FileDialog::new();
    match intent {
        DialogIntent::PickExe => dialog.add_filter("Windows 程序", &["exe"]).pick_file(),
        DialogIntent::PickWorkingDir
        | DialogIntent::PickProtonDir
        | DialogIntent::PickPrefixDir
        | DialogIntent::PickDxvkDir
        | DialogIntent::PickPrefixRoot => dialog.pick_folder(),
    }
}

pub fn pick(intent: DialogIntent) -> Task<Message> {
    Task::perform(
        async move {
            match tokio::task::spawn_blocking(move || dialog_impl(intent)).await {
                Ok(path) => Some((intent, path)),
                Err(_) => None,
            }
        },
        |result| match result {
            Some((intent, path)) => Message::DialogResult { intent, path },
            None => Message::Error("文件对话框失败".into()),
        },
    )
}
