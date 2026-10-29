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
    #[error("游戏已在运行")]
    GameAlreadyRunning,
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
    #[error("无效的 winetricks 组件: {0}")]
    ComponentVerbInvalid(String),
    #[error("winetricks 执行失败: {0}")]
    ComponentIo(#[source] std::io::Error),
    #[error("封面必须是 PNG / JPEG / WebP / BMP 图片: {0}")]
    CoverUnsupportedFormat(PathBuf),
    #[error("无法读取封面图片 {path}: {detail}")]
    CoverInvalidImage { path: PathBuf, detail: String },
    #[error("封面尺寸过大（{width}×{height}），单边不能超过 8192 像素")]
    CoverTooLarge { width: u32, height: u32 },
    #[error("复制封面失败: {0}")]
    CoverCopy(#[source] std::io::Error),
    #[error("等待游戏进程失败: {0}")]
    ProcessWait(#[source] std::io::Error),
    #[error("结束游戏进程失败: {0}")]
    Stop(#[source] launch::LaunchError),
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
