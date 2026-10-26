use std::sync::Arc;

use common_core::{
    error::{
        AppError, ContextualError,
        contextual::{
            self,
            ext::{IntoContextualExt, OptionExt},
        },
    },
    ext::ToOk,
    types::CursorPage,
};
use common_metrics::{MetricsTimerExt, metrics_name};
use file_validator::FileMetaData;
use tracing::instrument;

use crate::{
    mappers::visual_mapper::VisualMapper,
    repo::VisualRepo,
    services::visual_service::{
        AfterVisualDelete, AfterVisualUpload, VisualDeleteContext, publish_after_visual_delete,
        run_visual_delete_pipeline,
    },
    state::VisualState,
};
use audit::{AuditEvent, AuditRecorder};
use common_core::Result;
use types_visual::{
    dto::visual::{VisualCursorParam, VisualView},
    models::{DeleteVisualsParam, ExistsByHashBatchParam},
};

use types_core::cursor::TimeIdCursor;
use types_identity::auth::user::UserId;
use types_visual::visual::VisualId;

pub struct VisualService;

// 查询
impl VisualService {
    /// 获取单张影像的视图信息
    #[instrument(skip_all)]
    #[common_macros::metered]
    pub async fn get_visual_info(
        state: &VisualState,
        user_id: UserId,
        visual_id: VisualId,
    ) -> Result<VisualView> {
        Self::load_visuals_info(state, user_id, &[visual_id])
            .timed(metrics_name!("load_visuals_info"))
            .await?
            .pop()
            .ok_or_warn(
                "visual_not_exist",
                "用户尝试获取一个不存在的影像的信息",
                AppError::bad_request("影像不存在"),
            )?
            .to_ok()
    }

    /// 查询影像, 并生成包含访问令牌和点赞状态的视图
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, count = %visual_ids.len())
    )]
    pub async fn load_visuals_info(
        state: &VisualState,
        user_id: UserId,
        visual_ids: &[VisualId],
    ) -> Result<Vec<VisualView>> {
        // 获取影像记录 和 是否喜欢的id
        let (visuals, liked_visual_ids) =
            VisualRepo::load_visual_records(state, user_id, visual_ids).await?;

        // 组装结果
        let views = visuals
            .into_iter()
            .flatten()
            .map(|p| {
                let liked = liked_visual_ids.contains(&p.id);
                Ok(VisualView::from_record_with_tokens(p, user_id)?.with_liked(liked))
            })
            .collect::<contextual::Result<Vec<_>>>()?;
        Ok(views)
    }

    /// 游标获取影像列表
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn get_visual_cursor_page(
        state: &VisualState,
        user_id: UserId,
        req: VisualCursorParam,
    ) -> Result<CursorPage<VisualView, TimeIdCursor<VisualId>>> {
        // 获取visual_id
        let page = VisualRepo::query_visual_cursor_ids(state, req)
            .timed(metrics_name!("find_cursor_page_ids"))
            .await?;
        if page.records.is_empty() {
            return Ok(CursorPage::empty());
        }

        // 加载信息
        let visual_vos = Self::load_visuals_info(state, user_id, &page.records)
            .timed(metrics_name!("load_visuals_info"))
            .await?;

        // 组装结果
        Ok(page
            .replace_records(visual_vos)
            .with_next_cursor(|last_vo| TimeIdCursor {
                id: last_vo.id,
                time_at: last_vo.created_at,
            }))
    }

    /// 生成影像在对象存储中的 key（按日期分目录）
    #[inline]
    pub fn get_visual_s3_key(uuid: &str, metadata: &FileMetaData) -> String {
        let date_path = common_core::time::now().format("%Y/%m/%d");
        format!("visuals/{}/{}.{}", date_path, uuid, metadata.format)
    }
}

impl VisualService {
    /// 批量查询影像哈希值是否已存在
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(count = %req.hashes.len()))]
    pub async fn exists_by_hash_batch(
        state: &VisualState,
        req: ExistsByHashBatchParam,
    ) -> Result<Vec<bool>> {
        Ok(VisualRepo::exists_by_hash_batch(state, &req.hashes).await?)
    }

    /// 删除影像
    #[common_macros::metered]
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, count = %req.visual_ids.len())
    )]
    pub async fn delete_visuals(
        state: Arc<VisualState>,
        user_id: UserId,
        req: DeleteVisualsParam,
    ) -> Result<()> {
        // 查询属于用户的影像
        let visuals =
            VisualMapper::query_by_user_id_and_ids(&state.db, user_id, &req.visual_ids).await?;
        if visuals.len() != req.visual_ids.len() {
            Err(ContextualError::warn_without_source(
                "user_del_not_belong_visual",
                "用户尝试删除不属于自己的影像 或 影像不存在",
                AppError::bad_request("无法删除不属于自己的影像 或 影像不存在"),
            ))?;
        }
        let mut ctx = VisualDeleteContext { user_id, visuals };
        run_visual_delete_pipeline(&state.db, &mut ctx)
            .await
            .map_err(|error| {
                ContextualError::error(
                    "visual_delete_pipeline",
                    "执行影像删除事务失败",
                    error.to_string(),
                    error,
                )
            })?;

        // 发布删除后事件，缓存失效等后续操作不影响删除结果。
        publish_after_visual_delete(
            Arc::clone(&state),
            AfterVisualDelete {
                visuals: ctx.visuals,
            },
        );

        Ok(())
    }
}

/// 删除影像主表记录(受外键约束,`is_final` 使其恒在管道最后执行)
#[step_derive::declare_transaction_step(
    ctx = crate::services::visual_service::VisualDeleteContext,
    slice = crate::services::visual_service::MEDIA_DELETE_STEPS,
    name = "visual_record_delete",
    is_final = true,
)]
impl VisualService {
    /// 执行影像删除管道的最后一步, 删除影像主表记录
    async fn on_visual_delete(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        ctx: &mut VisualDeleteContext,
    ) -> common_core::error::contextual::Result<()> {
        let visual_ids = ctx.visual_ids();
        VisualMapper::delete_by_ids(txn, &visual_ids).await?;
        AuditRecorder::append(
            txn,
            AuditEvent::new("delete_visuals")
                .with_actor(ctx.user_id.0)
                .with_detail(serde_json::json!({
                    "visualIds": visual_ids.iter().map(|id| id.0).collect::<Vec<_>>()
                })),
        )
        .await?;
        Ok(())
    }
}

#[step_derive::declare_event_consumer(
    state = crate::state::VisualState,
    event = crate::services::visual_service::AfterVisualDelete,
    slice = crate::services::visual_service::AFTER_MEDIA_DELETE_CONSUMERS,
    name = "visual_delete_cache_invalidation",
)]
impl VisualService {
    /// 删除影像后
    #[instrument(name = "delete_visuals", skip_all)]
    async fn on_after_visual_delete(
        &self,
        state: Arc<VisualState>,
        event: Arc<AfterVisualDelete>,
    ) -> common_core::Result<()> {
        // 删除处理产物(processed/{file_id}/ 下的缩略/预览/裁剪变体)
        for visual in &event.visuals {
            let prefix = format!("processed/{}/", visual.file_id);
            match state.s3_client.list(&prefix).await {
                Ok(keys) if !keys.is_empty() => {
                    if let Err(error) = state.s3_client.delete_batch(keys).await {
                        tracing::warn!(%prefix, %error, "删除图片处理产物失败");
                    }
                }
                Ok(_) => {}
                Err(error) => tracing::warn!(%prefix, %error, "列出图片处理产物失败"),
            }
        }

        // 删除影像文件
        let file_ids = event
            .visuals
            .iter()
            .map(|visual| &visual.file_id)
            .collect::<Vec<_>>();
        state
            .s3_client
            .delete_batch(file_ids)
            .timed(metrics_name!("s3_delete_batch"))
            .await
            .into_contextual()?;

        VisualRepo::invalidate_deleted_visuals(state.as_ref(), &event.visuals).await;
        Ok(())
    }
}

#[step_derive::declare_event_consumer(
    state = crate::state::VisualState,
    event = crate::services::visual_service::AfterVisualUpload,
    slice = crate::services::visual_service::AFTER_MEDIA_UPLOAD_CONSUMERS,
    name = "visual_cursor_cache_invalidation",
)]
impl VisualService {
    /// 发布影像上传后的缓存失效事件
    #[instrument(name = "upload_visual", skip_all)]
    async fn on_after_visual_upload(
        &self,
        state: Arc<VisualState>,
        _event: Arc<AfterVisualUpload>,
    ) -> common_core::Result<()> {
        VisualRepo::after_visual_upload(&state).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::visual_service::{
        AFTER_MEDIA_DELETE_CONSUMERS, AFTER_MEDIA_UPLOAD_CONSUMERS, MEDIA_DELETE_STEPS,
    };
    use common_db::pipeline::Step;

    /// 验证 `linkme` 定义即注册:全部清理步骤均被收集,且存在唯一的 final 步骤(主表删除)
    #[test]
    fn step_registry_collects_all_steps() {
        let steps: Vec<&'static dyn Step<VisualDeleteContext>> = MEDIA_DELETE_STEPS.to_vec();

        #[cfg(feature = "face")]
        assert_eq!(steps.len(), 7);
        #[cfg(not(feature = "face"))]
        assert_eq!(steps.len(), 6);

        let finals: Vec<_> = steps.iter().filter(|step| step.is_final()).collect();
        assert_eq!(finals.len(), 1);
    }

    #[test]
    fn after_upload_registry_collects_all_consumers() {
        let consumers = AFTER_MEDIA_UPLOAD_CONSUMERS.to_vec();

        #[cfg(feature = "face")]
        assert_eq!(consumers.len(), 4);
        #[cfg(not(feature = "face"))]
        assert_eq!(consumers.len(), 3);
        assert!(
            consumers
                .iter()
                .any(|consumer| consumer.name() == "visual_cursor_cache_invalidation")
        );
        #[cfg(feature = "face")]
        assert!(
            consumers
                .iter()
                .any(|consumer| consumer.name() == "face_recognition")
        );
    }

    #[test]
    fn after_delete_registry_collects_cache_consumers() {
        let consumers = AFTER_MEDIA_DELETE_CONSUMERS.to_vec();

        assert_eq!(consumers.len(), 3);
        assert!(
            consumers
                .iter()
                .any(|consumer| consumer.name() == "visual_delete_cache_invalidation")
        );
        assert!(
            consumers
                .iter()
                .any(|consumer| consumer.name() == "timeline_stat_cache_invalidation")
        );
    }
}
