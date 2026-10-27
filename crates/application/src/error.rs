use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("数据库锁不可用")]
    DatabaseLocked,
    #[error("游戏可执行文件不存在: {0}")]
    ExecutableMissing(PathBuf),
    #[error("工作目录不存在: {0}")]
    WorkingDirMissing(PathBuf),
    #[error("游戏不存在")]
    GameNotFound,
    #[error("游戏正在运行，不能删除")]
    GameRunning,
    #[error("游戏正在运行，不能删除 Prefix")]
    PrefixInUseByRunningGame,
    #[error("该 Prefix 被多个游戏共享，不能删除")]
    PrefixShared,
    #[error("该 Prefix 被其他游戏使用")]
    PrefixUsedByOtherGame,
    #[error("该 Runner 是全局默认，请先更换默认 Runner")]
    DefaultRunnerInUse,
    #[error("仍有 {0} 个游戏使用该 Runner")]
    RunnerInUse(u64),
    #[error("已启用 DXVK-NVAPI 自动安装，但未选择目录")]
    DxvkNvapiSourceMissing,
    #[error("等待游戏进程失败: {0}")]
    ProcessWait(#[source] std::io::Error),
    #[error(transparent)]
    Storage(#[from] storage::StorageError),
    #[error(transparent)]
    Prefix(#[from] prefix::PrefixError),
    #[error(transparent)]
    Runner(#[from] runner_wine::RunnerError),
    #[error(transparent)]
    Launch(#[from] launch::LaunchError),
    #[error("后台任务失败: {0}")]
    Join(#[from] tokio::task::JoinError),
}
