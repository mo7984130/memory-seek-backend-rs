use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use common::{
    Result,
    error::AppError,
    ext::{OkExt, OptionExt, ResultErrExt, UintExt},
    metrics_group, metrics_name, metrics_success,
    models::CursorPage,
    utils::{DbUtils, MetricsTimerExt},
};
use insight_face_rs::types::DIMS;
use ndarray::Array2;
use petal_clustering::{Fit, HDbscan};
use sea_orm::{EntityName, EntityTrait};
use tokio::{spawn, task::spawn_blocking};
use tracing::{info, instrument};
use types::{
    auth::user::UserId,
    cursor::TimeIdCursor,
    photo::{
        ImageToken, PersonView,
        dto::face::bbox_from_insight,
        dto::person::MergePersonParam,
        dto::photo::PhotoView,
        face,
        models::PersonName,
        person::{self, NewPerson, PersonId, PersonRecord},
        photo::PhotoId,
    },
};

use crate::{
    PhotoState,
    mappers::{face_mapper::FaceMapper, person_mapper::PersonMapper, photo_mapper::PhotoMapper},
    services::{embedding_math, person_cover, photo_service::PhotoService},
};

pub struct PersonService;

// 创建
impl PersonService {
    pub async fn full_scan(state: Arc<PhotoState>, user_id: UserId) -> Result<()> {
        spawn(async move { Self::inner_full_scan(state, user_id).await });
        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn inner_full_scan(state: Arc<PhotoState>, user_id: UserId) -> Result<()> {
        if user_id != UserId(1) {
            return Err(AppError::forbidden("无权限"));
        }

        // 保存表(聚类会重建 person 并改写 photo_face.person_id, 两张表都备份)
        info!("开始保存 person/face 表");
        state
            .backup_storage
            .backup_tables(
                &state.db,
                &[person::Entity.table_name(), face::Entity.table_name()],
                backup::storage::BackupType::Manual,
            )
            .await?;
        info!("保存表完成");

        info!("加载照片数据");
        let faces = FaceMapper::query_all(&state.db).await?;

        // 一次性加载聚类涉及的全部照片, 构建 photo_id -> file_id 映射(封面冗余字段来源)
        let photo_file_ids: HashMap<PhotoId, String> = {
            let photo_ids: HashSet<PhotoId> = faces.iter().map(|f| f.photo_id).collect();
            PhotoMapper::query_by_ids(&state.db, &photo_ids.iter().copied().collect::<Vec<_>>())
                .await?
                .into_iter()
                .map(|photo| (photo.id, photo.file_id))
                .collect()
        };

        info!("加载照片数据完成");

        info!("开始聚类");
        let embedding_array = {
            let embeddings: Vec<f32> = faces
                .iter()
                .flat_map(|f| f.embedding.iter().copied())
                .collect();
            Array2::from_shape_vec((faces.len(), DIMS), embeddings)
                .trace_internal_err("ndarray_from_shape_error", "人脸聚类时, 转换 NdArray 错误")?
        };
        let cluster_result = spawn_blocking(move || {
            let mut hdbscan = HDbscan {
                alpha: 1.0,
                min_samples: 5,
                min_cluster_size: 5,
                boruvka: false,
                ..Default::default()
            };

            Ok::<_, AppError>(hdbscan.fit(&embedding_array, None))
        })
        .await??;
        info!("聚类完成");

        DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                info!("开始清除 person 表");
                person::Entity::delete_many().exec(txn).await?;
                // 清空所有人脸归属, 避免指向已删除人物的悬空引用
                // (离群/噪声人脸不在聚类结果中, 不会在下方循环里被重新指派)
                FaceMapper::clear_person_id(txn).await?;
                info!("清除完成");

                info!("开始插入人物");
                for cluster in cluster_result.0 {
                    let embedding_ids = cluster.1;
                    // 封面人脸: 取 cluster 内 score 最高的人脸
                    let cover_face = embedding_ids
                        .iter()
                        .max_by(|&&a, &&b| faces[a].score.total_cmp(&faces[b].score))
                        .map(|&idx| &faces[idx])
                        .expect("cluster should not be empty");
                    // score 加权质心: (weight, centroid) = (Σscore, Σ(score×embedding))
                    let (weight, centroid) = embedding_math::weighted_sum(
                        embedding_ids
                            .iter()
                            .map(|idx| (faces[*idx].score, &faces[*idx].embedding)),
                    );
                    // 插入人物
                    let person = NewPerson {
                        name: cluster.0.to_string(),
                        weight,
                        centroid,
                        cover_face_id: cover_face.id,
                        cover_photo_id: cover_face.photo_id,
                        cover_file_id: photo_file_ids
                            .get(&cover_face.photo_id)
                            .cloned()
                            .ok_or_error(
                                "person_cover_photo_not_found",
                                "封面人脸所属照片不存在",
                                AppError::InternalServerError,
                            )?,
                        cover_bbox: bbox_from_insight(cover_face.bbox),
                        face_count: embedding_ids.len() as u64,
                    };
                    let person = PersonMapper::insert(txn, person).await?;

                    // 更新人脸归属
                    FaceMapper::update_person_id(
                        txn,
                        person.id,
                        embedding_ids.iter().map(|idx| faces[*idx].id),
                    )
                    .await?;
                }
                info!("插入人物完成");
                info!("人脸聚类完成");
                Ok(())
            })
        })
        .await?;

        Ok(())
    }
}

// 修改
impl PersonService {
    /// 重命名人物
    #[tracing::instrument(skip_all)]
    pub async fn rename_person(
        state: &PhotoState,
        person_id: PersonId,
        new_name: PersonName,
    ) -> Result<()> {
        // 校验人物存在
        PersonMapper::query_by_id(&state.db, person_id)
            .await?
            .ok_or_error(
                "person_not_found",
                "人物不存在",
                AppError::not_found("人物不存在"),
            )?;

        PersonMapper::rename(&state.db, person_id, new_name.into_inner())
            .await?
            .no_zero_or_warn(
                "person_rename_fail",
                "重命名人物失败",
                AppError::bad_request("重命名人物失败"),
            )?;

        Ok(())
    }

    /// 合并人物: 将 source 的全部人脸归属转移到 target, 并删除 source
    #[tracing::instrument(skip_all)]
    pub async fn merge_person(
        state: &PhotoState,
        param: MergePersonParam,
    ) -> Result<PersonView> {
        let MergePersonParam {
            source_person_id,
            target_person_id,
        } = param;

        DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                // 按 id 升序加行锁两个人物(存在性校验在锁内完成, 避免 TOCTOU), 防并发死锁
                let (source, target) = if source_person_id.0 < target_person_id.0 {
                    let source = PersonMapper::query_by_id_for_update(txn, source_person_id)
                        .await?
                        .ok_or_error(
                            "person_not_found",
                            "源人物不存在",
                            AppError::not_found("源人物不存在"),
                        )?;
                    let target = PersonMapper::query_by_id_for_update(txn, target_person_id)
                        .await?
                        .ok_or_error(
                            "person_not_found",
                            "目标人物不存在",
                            AppError::not_found("目标人物不存在"),
                        )?;
                    (source, target)
                } else {
                    let target = PersonMapper::query_by_id_for_update(txn, target_person_id)
                        .await?
                        .ok_or_error(
                            "person_not_found",
                            "目标人物不存在",
                            AppError::not_found("目标人物不存在"),
                        )?;
                    let source = PersonMapper::query_by_id_for_update(txn, source_person_id)
                        .await?
                        .ok_or_error(
                            "person_not_found",
                            "源人物不存在",
                            AppError::not_found("源人物不存在"),
                        )?;
                    (source, target)
                };

                // 转移人脸归属
                FaceMapper::move_person_faces(txn, source_person_id, target_person_id).await?;

                // 封面: 合并后取两者封面中 score 更高者(集合取并, 极值 = max(两个封面))
                let source_cover_face = FaceMapper::query_by_id(txn, source.cover_face_id)
                    .await?
                    .ok_or_error(
                    "person_cover_face_not_found",
                    "人物封面人脸不存在",
                    AppError::InternalServerError,
                )?;
                let target_cover_face = FaceMapper::query_by_id(txn, target.cover_face_id)
                    .await?
                    .ok_or_error(
                    "person_cover_face_not_found",
                    "人物封面人脸不存在",
                    AppError::InternalServerError,
                )?;
                let cover = if source_cover_face.score > target_cover_face.score {
                    person_cover::cover_update_from_face(txn, &source_cover_face).await?
                } else {
                    None
                };

                // 目标人物: 数量/权重/质心合并(封面可能替换)
                PersonMapper::update_stats(
                    txn,
                    target_person_id,
                    target.face_count + source.face_count,
                    target.weight + source.weight,
                    embedding_math::add(&target.centroid, &source.centroid),
                    cover,
                )
                .await?;

                // 删除源人物
                PersonMapper::delete_by_id(txn, source_person_id).await?;
                Ok(())
            })
        })
        .await?;

        // 返回合并后的目标人物视图
        let person = PersonMapper::query_by_id(&state.db, target_person_id)
            .await?
            .ok_or_error(
                "person_not_found",
                "目标人物不存在",
                AppError::not_found("目标人物不存在"),
            )?;
        Ok(Self::to_view(state, person))
    }
}

// 查询
impl PersonService {
    pub async fn get_persons(
        state: &PhotoState,
        cursor: Option<PersonId>,
        size: u64,
    ) -> Result<CursorPage<PersonView, PersonId>> {
        let persons = PersonMapper::query(&state.db, cursor, size + 1).await?;
        let views = persons
            .into_iter()
            .map(|person| Self::to_view(state, person))
            .collect::<Vec<_>>();
        let page = CursorPage::from_oversize_fn(views, size, |person| person.id);
        Ok(page)
    }

    /// 构建人物视图: 使用封面冗余字段直接内存组装裁剪 token, 加密后返回
    /// (与 `CollectionView::with_generate_cover_token` / `PhotoView::with_tokens` 一致)
    fn to_view(state: &PhotoState, person: PersonRecord) -> PersonView {
        PersonView {
            id: person.id,
            name: person.name,
            cover_token: state
                .token_cipher
                .encrypt(
                    &ImageToken::crop(person.cover_file_id, person.cover_bbox),
                    Some(&person.id.to_string()),
                )
                .ok(),
            face_count: person.face_count,
        }
    }

    /// 获取人物的照片列表(游标分页, 参照 `CollectionPhotoService::get_photos`)
    #[tracing::instrument(skip_all)]
    pub async fn get_person_photos(
        state: &PhotoState,
        user_id: UserId,
        person_id: PersonId,
        cursor: Option<TimeIdCursor<PhotoId>>,
        size: u64,
    ) -> Result<CursorPage<PhotoView, String>> {
        metrics_group!();

        let photo_ids = FaceMapper::query_photo_ids_by_person_id(&state.db, person_id)
            .timed(metrics_name!("query_photo_ids"))
            .await?;
        if photo_ids.is_empty() {
            metrics_success!();
            return Ok(CursorPage::empty());
        }

        let photo_ids =
            PhotoMapper::query_ids_page_by_ids(&state.db, &photo_ids, cursor.as_ref(), size + 1)
                .timed(metrics_name!("query_photo_page_ids"))
                .await?;

        let CursorPage {
            records: photo_ids,
            has_more,
            ..
        } = CursorPage::from_oversize(photo_ids, size);

        let photo_vos = PhotoService::load_photos_info(state, user_id, &photo_ids)
            .timed(metrics_name!("load_photos_info"))
            .await?;
        let next_cursor = photo_vos.last().and_then(|vo| {
            PhotoId::parse_from_str_or_none(&vo.id).map(|id| {
                TimeIdCursor {
                    created_at: vo.created_at,
                    id,
                }
                .encode()
            })
        });

        metrics_success!();
        CursorPage {
            records: photo_vos,
            has_more,
            next_cursor,
        }
        .to_ok()
    }
}

// 删除
impl PersonService {
    /// 删除人物: 清空其所有人脸归属后删除人物
    #[tracing::instrument(skip_all)]
    pub async fn delete_person(state: &PhotoState, person_id: PersonId) -> Result<()> {
        DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                // 清空该人物所有人脸归属, 避免悬空引用
                FaceMapper::clear_person_id_by_person(txn, person_id).await?;
                // 删除人物
                PersonMapper::delete_by_id(txn, person_id)
                    .await?
                    .no_zero_or_warn(
                        "person_delete_fail",
                        "删除人物失败",
                        AppError::not_found("人物不存在"),
                    )?;
                Ok(())
            })
        })
        .await?;

        Ok(())
    }
}
