use axum::extract::State;
use axum::routing::get;
use axum::Router;
use common::error::AppError;
use common::r::R;
use std::sync::Arc;

use crate::state::PhotoState;
use crate::models::common::PhotoTimelineStatVO;
use crate::services::timeline_stat_service::TimelineStatService;

/// 时间线控制器，处理照片时间线统计查询
pub struct TimelineController;

impl TimelineController {
    /// 构建时间线模块的路由表
    ///
    /// # 返回
    /// 返回时间线相关的 axum Router，包含统计查询接口
    pub fn routes() -> Router<Arc<PhotoState>> {
        Router::new().route("/stats", get(Self::get_stats))
    }

    /// 获取照片时间线统计信息
    ///
    /// # 参数
    /// - `state`: 应用状态，包含数据库连接等依赖
    ///
    /// # 返回
    /// 返回按时间维度聚合的照片统计数据列表
    ///
    /// # 错误
    /// - `AppError`: 数据库查询失败等内部错误
    async fn get_stats(
        State(state): State<Arc<PhotoState>>,
    ) -> Result<R<Vec<PhotoTimelineStatVO>>, AppError> {
        let stats = TimelineStatService::get_stats(&state).await?;
        Ok(R::ok(stats))
    }
}
