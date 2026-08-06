use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use common::ext::ToOk;
use common::{
    Result,
    error::AppError,
    ext::{OptionExt, ResultErrExt, UintExt},
    metrics_group, metrics_name, metrics_success,
    models::CursorPage,
    utils::{DbUtils, MetricsTimerExt},
};
use insight_face_rs::types::{DIMS, FaceEmbedding};
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
        dto::person::{
            MergePersonParam, PersonCursorParam, PersonPhotoCursorParam, RenamePersonParam,
        },
        dto::photo::PhotoView,
        face,
        person::{self, NewPerson, PersonId, PersonRecord},
        photo::PhotoId,
    },
};

use crate::{
    PhotoState,
    mappers::{
        face_mapper::FaceMapper,
        person_mapper::{PersonCoverUpdate, PersonMapper},
        photo_mapper::PhotoMapper,
    },
    services::photo_service::PhotoService,
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
        user_id.ensure_admin()?;

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
                    let (weight, centroid) = FaceEmbedding::weighted_sum(
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
        param: RenamePersonParam,
    ) -> Result<()> {
        // 校验人物存在
        PersonMapper::rename(&state.db, person_id, param.new_name.into_inner())
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
    pub async fn merge_person(state: &PhotoState, param: MergePersonParam) -> Result<PersonView> {
        let MergePersonParam {
            source_person_id,
            target_person_id,
        } = param;

        DbUtils::write(&state.db, |txn| {
            Box::pin(async move {
                let (source, target) = DbUtils::ensure_lock_two_ordered(
                    txn,
                    source_person_id,
                    target_person_id,
                    PersonMapper::lock_by_id,
                    |person| {
                        person.ok_or_warn_bad_request(
                            "person_not_found",
                            "合并人物时, 人物不存在",
                            "人物不存在",
                        )
                    },
                )
                .await?;

                // 转移人脸归属
                FaceMapper::move_person_faces(txn, source_person_id, target_person_id).await?;

                // 封面: 合并后取两者封面中 score 更高者(集合取并, 极值 = max(两个封面))
                let cover = {
                    let faces = FaceMapper::query_by_ids(
                        txn,
                        &[source.cover_face_id, target.cover_face_id],
                    )
                    .await?;

                    let source_cover_face = faces
                        .iter()
                        .find(|f| f.id == source.cover_face_id)
                        .ok_or_error(
                            "person_cover_face_not_found",
                            "人物封面人脸不存在",
                            AppError::InternalServerError,
                        )?;

                    let target_cover_face = faces
                        .iter()
                        .find(|f| f.id == target.cover_face_id)
                        .ok_or_error(
                            "person_cover_face_not_found",
                            "人物封面人脸不存在",
                            AppError::InternalServerError,
                        )?;

                    if source_cover_face.score > target_cover_face.score {
                        let file_id =
                            PhotoMapper::query_file_id_by_id(txn, source_cover_face.photo_id)
                                .await?
                                .ok_or_error(
                                    "person_cover_photo_not_found",
                                    "封面人脸所属照片不存在",
                                    AppError::InternalServerError,
                                )?;
                        Some(PersonCoverUpdate {
                            cover_face_id: source_cover_face.id,
                            cover_photo_id: source_cover_face.photo_id,
                            cover_file_id: file_id,
                            cover_bbox: bbox_from_insight(source_cover_face.bbox),
                        })
                    } else {
                        None
                    }
                };

                // 目标人物: 数量/权重/质心合并(封面可能替换)
                PersonMapper::update_stats(
                    txn,
                    target_person_id,
                    target.face_count + source.face_count,
                    target.weight + source.weight,
                    target.centroid.add(&source.centroid),
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
        param: PersonCursorParam,
    ) -> Result<CursorPage<PersonView, PersonId>> {
        let persons = PersonMapper::query(&state.db, param.cursor, param.size + 1).await?;
        let views = persons
            .into_iter()
            .map(|person| Self::to_view(state, person))
            .collect::<Vec<_>>();
        let page = CursorPage::from_oversize_fn(views, param.size, |person| Ok(person.id))?;
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
        param: PersonPhotoCursorParam,
    ) -> Result<CursorPage<PhotoView, TimeIdCursor<PhotoId>>> {
        metrics_group!();

        let photo_ids =
            FaceMapper::query_photo_ids_cursor_page(&state.db, person_id, param.cursor, param.size)
                .timed(metrics_name!("query_photo_ids"))
                .await?;
        if photo_ids.is_empty() {
            metrics_success!();
            return Ok(CursorPage::empty());
        }

        let photos = PhotoService::load_photos_info(state, user_id, &photo_ids)
            .timed(metrics_name!("load_photos_info"))
            .await?;

        let page = CursorPage::from_oversize_fn(photos, param.size, |photo| {
            TimeIdCursor {
                created_at: photo.created_at,
                id: photo.id,
            }
            .to_ok()
        })?;

        metrics_success!();
        Ok(page)
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
