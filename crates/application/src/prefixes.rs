use crate::AppError;
use launcher_core::{Game, Runner};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub use ::prefix::{DxvkSource, PrefixPlan, PrefixStatus};

pub fn plan_for_game(
    game: &Game,
    runners: &[Runner],
    default_runner_id: Option<Uuid>,
    prefix_root: &Path,
    steam_compat_client: PathBuf,
) -> Result<PrefixPlan, AppError> {
    let runner = ::launch::resolve_runner(game, runners, default_runner_id)?;
    Ok(::prefix::plan_for_game(
        game,
        runner,
        prefix_root,
        steam_compat_client,
    ))
}

pub async fn prefix_status(plan: PrefixPlan) -> PrefixStatus {
    tokio::task::spawn_blocking(move || ::prefix::status(&plan))
        .await
        .unwrap_or(PrefixStatus::NotInitialized)
}

pub async fn create_prefix(
    plan: PrefixPlan,
    dxvk: Option<DxvkSource>,
) -> Result<PathBuf, AppError> {
    plan.initialize().await?;
    if let Some(source) = dxvk {
        ::prefix::install_dxvk(&plan, &source).await?;
    }
    Ok(plan.path)
}

pub async fn install_dxvk(plan: PrefixPlan, source: DxvkSource) -> Result<(), AppError> {
    ::prefix::install_dxvk(&plan, &source).await?;
    Ok(())
}
