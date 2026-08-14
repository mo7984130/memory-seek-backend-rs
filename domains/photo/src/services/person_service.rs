use std::collections::HashMap;
use std::sync::Arc;

use common::ext::{IntoContextualExt, ToOk};
use common::{
    Result,
    error::AppError,
    ext::{ContextResultExt, OptionExt, UintExt},
    metrics_name,
    models::CursorPage,
    utils::{DbUtils, MetricsTimerExt, token_cipher},
};
use insight_face_rs::types::{DIMS, FaceEmbedding};
use ndarray::Array2;
use petal_clustering::{Fit, HDbscan};
use sea_orm::EntityTrait;
use tokio::{spawn, task::spawn_blocking};
use tracing::{info, instrument};
use types::{
    auth::user::{AdminId, UserId},
    cursor::{FaceCountIdCursor, TimeIdCursor},
    photo::{
        ImageToken, PersonView,
        dto::face::bbox_from_insight,
        dto::person::{
            MergePersonParam, PersonCursorParam, PersonPhotoCursorParam, PersonSearchParam,
            RenamePersonParam, SecondaryClusterParam,
        },
        dto::photo::PhotoView,
        face::{self, FaceId},
        person::{self, NewPerson, PersonId},
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
    models::PersonBriefRow,
    services::photo_service::PhotoService,
};

pub struct PersonService;

// 创建
impl PersonService {
    /// 异步启动全量人物扫描任务.
    #[instrument(skip_all, fields(admin_user_id = %admin))]
    pub async fn full_scan(state: Arc<PhotoState>, admin: AdminId) -> Result<()> {
        spawn(async move { Self::inner_full_scan(state, admin).await });
        Ok(())
    }

    #[common::metered(name = "person_full_scan")]
    #[instrument(
        name = "person_full_scan",
        skip_all,
        fields(admin_user_id = %admin)
    )]
    /// 执行全量扫描, 重建人物聚类和人脸归属.
    pub async fn inner_full_scan(state: Arc<PhotoState>, admin: AdminId) -> Result<()> {
        let user_id = admin.into_inner();
        info!(user_id = %user_id, "管理员触发人物全量聚类");

        info!("加载照片数据");
        let (faces, photo_file_rows) = state.repo.load_faces_with_photo_files().await?;

        // 一次性加载聚类涉及的全部照片, 构建 photo_id -> file_id 映射(封面冗余字段来源)
        let photo_file_ids: HashMap<PhotoId, String> = { photo_file_rows.into_iter().collect() };

        info!("加载照片数据完成");

        info!("开始聚类");
        let embedding_array = {
            let embeddings: Vec<f32> = faces
                .iter()
                .flat_map(|f| f.embedding.iter().copied())
                .collect();
            Array2::from_shape_vec((faces.len(), DIMS), embeddings).context_error(
                "ndarray_from_shape_error",
                "人脸聚类时, 转换 NdArray 错误",
                AppError::InternalServerError,
            )?
        };
        let cluster_result = spawn_blocking(move || {
            let mut hdbscan = HDbscan {
                alpha: 1.0,
                min_samples: 5,
                min_cluster_size: 5,
                boruvka: false,
                ..Default::default()
            };

            hdbscan.fit(&embedding_array, None)
        })
        .await
        .into_contextual()?;
        info!("聚类完成");

        info!("开始保存 person/face 表");
        state.repo.backup_face_tables(&state.backup_storage).await?;
        info!("保存表完成");
        state
            .repo
            .transaction(|txn| {
                Box::pin(async move {
                    info!("开始清除 person 表");
                    // 保存表(聚类会重建 person 并改写 photo_face.person_id, 两张表都备份)
                    person::Entity::delete_many()
                        .exec(txn)
                        .await
                        .into_contextual()?;
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

    /// 二次聚类: 将全部未分配人脸(`person_id IS NULL`)按 centroid 余弦相似度
    /// 指派到已有人物, 低于阈值的人脸保持未分配。
    ///
    /// 人脸 embedding 已归一化(insight-face-rs 输出时 normalize);
    /// person centroid 为未归一化加权和, 匹配前先 normalize。
    #[instrument(skip_all, fields(admin_user_id = %admin))]
    pub async fn assign_unassigned_faces(
        state: Arc<PhotoState>,
        admin: AdminId,
        req: SecondaryClusterParam,
    ) -> Result<()> {
        spawn(async move { Self::inner_assign_unassigned_faces(state, admin, req).await });
        Ok(())
    }

    #[common::metered(name = "person_secondary_cluster")]
    #[instrument(
        name = "person_secondary_cluster",
        skip_all,
        fields(admin_user_id = %admin)
    )]
    /// 执行未分配人脸与人物质心的匹配和归属更新.
    async fn inner_assign_unassigned_faces(
        state: Arc<PhotoState>,
        admin: AdminId,
        req: SecondaryClusterParam,
    ) -> Result<()> {
        let user_id = admin.into_inner();
        let SecondaryClusterParam { threshold } = req;
        info!(user_id = %user_id, threshold, "管理员触发二次聚类");

        let (faces, persons) = state.repo.load_unassigned_faces_and_persons().await?;
        info!(
            "加载完成: 未分配人脸 {} 张, 人物 {} 个",
            faces.len(),
            persons.len()
        );
        if faces.is_empty() || persons.is_empty() {
            return Ok(());
        }

        // 内存分类(CPU 密集, 放入阻塞线程池; faces 原样带回供事务内使用)
        let classified = spawn_blocking(move || {
            let face_refs = faces
                .iter()
                .map(|f| (f.id, &f.embedding))
                .collect::<Vec<_>>();
            let person_refs = persons
                .iter()
                .map(|p| (p.id, &p.centroid))
                .collect::<Vec<_>>();
            let classified = Self::classify_unassigned(&face_refs, &person_refs, threshold);
            (classified, faces)
        })
        .await
        .into_contextual()?;
        let (classified, faces) = classified;

        let matched = classified.len();
        let unmatched = faces.len() - matched;
        info!("分类完成: 匹配 {} 张, 未匹配 {} 张", matched, unmatched);
        if classified.is_empty() {
            return Ok(());
        }

        state
            .repo
            .transaction(|txn| {
                Box::pin(async move {
                    // 按人物分组聚合(一次事务内批量写入, 保持不变量一致)
                    let face_by_id: HashMap<FaceId, &face::FaceRecord> =
                        faces.iter().map(|f| (f.id, f)).collect();
                    let mut per_person: HashMap<PersonId, Vec<&face::FaceRecord>> = HashMap::new();
                    for (face_id, person_id) in &classified {
                        per_person
                            .entry(*person_id)
                            .or_default()
                            .push(face_by_id[face_id]);
                    }

                    // 涉及的人物按 id 升序加行锁(与 change_face_belonging 加锁顺序一致, 防死锁)
                    let mut person_ids: Vec<PersonId> = per_person.keys().copied().collect();
                    person_ids.sort_unstable();
                    for person_id in person_ids {
                        let assigned_faces = &per_person[&person_id];
                        let person = PersonMapper::lock_by_id(txn, person_id)
                            .await?
                            .ok_or_error(
                                "person_not_found",
                                "二次聚类时, 人物不存在",
                                AppError::InternalServerError,
                            )?;

                        // Δweight / Δcentroid = Σscore / Σ(score×embedding)
                        let (delta_weight, delta_centroid) = FaceEmbedding::weighted_sum(
                            assigned_faces.iter().map(|f| (f.score, &f.embedding)),
                        );

                        // 封面: 集合新增多张人脸, 新封面 = max(当前封面, 组内 score 最高脸)
                        let cover = {
                            let top_in_group = assigned_faces
                                .iter()
                                .max_by(|&&a, &&b| a.score.total_cmp(&b.score))
                                .expect("assigned_faces should not be empty");
                            let current_cover_score =
                                FaceMapper::query_by_id(txn, person.cover_face_id)
                                    .await?
                                    .ok_or_error(
                                        "person_cover_face_not_found",
                                        "人物封面人脸不存在",
                                        AppError::InternalServerError,
                                    )?
                                    .score;
                            if top_in_group.score > current_cover_score {
                                let file_id =
                                    PhotoMapper::query_file_id_by_id(txn, top_in_group.photo_id)
                                        .await?
                                        .ok_or_error(
                                            "person_cover_photo_not_found",
                                            "封面人脸所属照片不存在",
                                            AppError::InternalServerError,
                                        )?;
                                Some(PersonCoverUpdate {
                                    cover_face_id: top_in_group.id,
                                    cover_photo_id: top_in_group.photo_id,
                                    cover_file_id: file_id,
                                    cover_bbox: bbox_from_insight(top_in_group.bbox),
                                })
                            } else {
                                None
                            }
                        };

                        PersonMapper::update_stats(
                            txn,
                            person_id,
                            person.face_count + assigned_faces.len() as u64,
                            person.weight + delta_weight,
                            person.centroid.add(&delta_centroid),
                            cover,
                        )
                        .await?;

                        FaceMapper::update_person_id(
                            txn,
                            person_id,
                            assigned_faces.iter().map(|f| f.id),
                        )
                        .await?;
                    }
                    Ok(())
                })
            })
            .await?;

        Ok(())
    }

    /// 最近质心分类: 每张未分配人脸取与各人物 centroid(已归一化)余弦相似度最高者,
    /// 相似度 `>= threshold` 才指派, 否则保持未分配。
    fn classify_unassigned(
        faces: &[(FaceId, &FaceEmbedding)],
        persons: &[(PersonId, &FaceEmbedding)],
        threshold: f32,
    ) -> Vec<(FaceId, PersonId)> {
        let mut assignments = Vec::new();
        for (face_id, embedding) in faces {
            let mut best: Option<(f32, PersonId)> = None;
            for (person_id, centroid) in persons {
                let sim = embedding.cosine_similarity(centroid);
                if sim >= threshold && best.is_none_or(|(b, _)| sim > b) {
                    best = Some((sim, *person_id));
                }
            }
            if let Some((_, person_id)) = best {
                assignments.push((*face_id, person_id));
            }
        }
        assignments
    }
}

// 修改
impl PersonService {
    /// 重命名人物(同步维护姓名首字母)
    #[common::metered]
    #[tracing::instrument(skip_all, fields(person_id = %person_id))]
    pub async fn rename_person(
        state: &PhotoState,
        person_id: PersonId,
        req: RenamePersonParam,
    ) -> Result<()> {
        let new_name = req.new_name.into_inner();
        let name_initials = Self::compute_name_initials(&new_name);
        state
            .repo
            .rename_person(person_id, new_name, name_initials)
            .await?;

        Ok(())
    }

    /// 合并人物（高危操作，仅管理员）: 将 source 的全部人脸归属转移到 target, 并删除 source
    #[common::metered]
    #[tracing::instrument(skip_all, fields(admin_user_id = %admin))]
    pub async fn merge_person(
        state: &PhotoState,
        admin: AdminId,
        req: MergePersonParam,
    ) -> Result<PersonView> {
        let MergePersonParam {
            source_person_id,
            target_person_id,
        } = req;

        state
            .repo
            .transaction(|txn| {
                Box::pin(async move {
                    let (source, target) = DbUtils::ensure_lock_two_ordered(
                        txn,
                        source_person_id,
                        target_person_id,
                        |db, id| async move { Ok(PersonMapper::lock_by_id(db, id).await?) },
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
        let person = state
            .repo
            .query_person(target_person_id)
            .await?
            .ok_or_error(
                "person_not_found",
                "目标人物不存在",
                AppError::not_found("目标人物不存在"),
            )?;

        // 失效源/目标人物缓存（L1 + L2），源人物已删除
        // 错误不返回
        state
            .repo
            .invalidate_persons(&[source_person_id, target_person_id])
            .await;

        Self::to_view(admin.into_inner(), person.into())
    }
}

// 查询
impl PersonService {
    /// 查询人物名称（审计记录改名前后用）
    #[tracing::instrument(skip_all, fields(person_id = %person_id))]
    pub async fn query_name(state: &PhotoState, person_id: PersonId) -> Result<Option<String>> {
        let person = state.repo.query_person(person_id).await?;
        Ok(person.map(|record| record.name))
    }

    #[common::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    /// 查询当前用户可见的人物列表.
    pub async fn get_persons(
        state: &PhotoState,
        user_id: UserId,
        req: PersonCursorParam,
    ) -> Result<CursorPage<PersonView, FaceCountIdCursor<PersonId>>> {
        let page = state.repo.query_person_page(req.cursor, req.size).await?;

        // 提取本页人物 ID, 通过三级缓存批量加载轻量摘要
        let person_ids = page.records.iter().map(|p| p.id).collect::<Vec<_>>();
        let briefs = state.repo.get_person_briefs(&person_ids).await?;

        let views = briefs
            .into_iter()
            .flatten()
            .map(|person| Self::to_view(user_id, person))
            .collect::<Result<Vec<_>>>()?;
        page.replace_records(views).with_next_cursor(|person| {
            FaceCountIdCursor {
                face_count: person.face_count,
                id: person.id,
            }
            .to_ok()
        })
    }

    /// 按关键词前缀搜索人物(匹配完整名字或姓名首字母)
    #[common::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    pub async fn search_persons(
        state: &PhotoState,
        user_id: UserId,
        req: PersonSearchParam,
    ) -> Result<CursorPage<PersonView, PersonId>> {
        let PersonSearchParam {
            keyword,
            cursor,
            size,
        } = req;
        let page = state
            .repo
            .search_person_page(&keyword, cursor, size)
            .await?;

        // 提取本页人物 ID, 通过三级缓存批量加载轻量摘要
        let person_ids = page.records.iter().map(|p| p.id).collect::<Vec<_>>();
        let briefs = state.repo.get_person_briefs(&person_ids).await?;

        let views = briefs
            .into_iter()
            .flatten()
            .map(|person| Self::to_view(user_id, person))
            .collect::<Result<Vec<_>>>()?;
        page.replace_records(views)
            .with_next_cursor(|person| Ok(person.id))
    }

    /// 计算姓名首字母(大写, 如 张三 -> ZS, Alice Wang -> AW)
    ///
    /// 逐字符处理: 汉字经拼音库取拼音首字母; ASCII 字母按单词取首字母;
    /// 其余字符(空白/标点/数字)作为分隔或忽略。无有效字符时返回 `None`。
    fn compute_name_initials(name: &str) -> Option<String> {
        use pinyin::ToPinyin;

        fn flush_word(ascii_word: &mut String, initials: &mut String) {
            if let Some(first) = ascii_word.chars().next() {
                initials.push(first.to_ascii_uppercase());
            }
            ascii_word.clear();
        }

        let mut initials = String::new();
        let mut ascii_word = String::new();
        for c in name.chars() {
            if c.is_ascii_alphabetic() {
                ascii_word.push(c);
                continue;
            }
            flush_word(&mut ascii_word, &mut initials);
            if let Some(py) = c.to_pinyin() {
                let first = py
                    .first_letter()
                    .chars()
                    .next()
                    .unwrap_or_default()
                    .to_ascii_uppercase();
                initials.push(first);
            }
        }
        flush_word(&mut ascii_word, &mut initials);

        if initials.is_empty() {
            None
        } else {
            Some(initials)
        }
    }

    /// 构建人物视图: 使用封面冗余字段直接内存组装裁剪 token, 加密后返回
    /// (与 `CollectionView::with_generate_cover_token` / `PhotoView::with_tokens` 一致)
    fn to_view(viewer: UserId, person: PersonBriefRow) -> Result<PersonView> {
        Ok(PersonView {
            id: person.id,
            name: person.name,
            cover_token: Some(token_cipher().encrypt(
                &ImageToken::crop(viewer, person.cover_file_id, person.cover_bbox),
                Some(&format!("{}:{}", person.id, viewer)),
            )?),
            face_count: person.face_count as u64,
        })
    }

    /// 获取人物的照片列表(游标分页, 参照 `CollectionPhotoService::get_photos`)
    #[common::metered]
    #[tracing::instrument(
        skip_all,
        fields(user_id = %user_id, person_id = %person_id)
    )]
    /// 查询人物关联的照片并生成照片视图.
    pub async fn get_person_photos(
        state: &PhotoState,
        user_id: UserId,
        person_id: PersonId,
        req: PersonPhotoCursorParam,
    ) -> Result<CursorPage<PhotoView, TimeIdCursor<PhotoId>>> {
        let page = state
            .repo
            .query_person_photo_ids(person_id, &req)
            .timed(metrics_name!("query_photo_ids"))
            .await?;
        if page.records.is_empty() {
            return Ok(CursorPage::empty());
        }

        let photos = PhotoService::load_photos_info(state, user_id, &page.records)
            .timed(metrics_name!("load_photos_info"))
            .await?;

        page.replace_records(photos).with_next_cursor(|photo| {
            TimeIdCursor {
                created_at: photo.created_at,
                id: photo.id,
            }
            .to_ok()
        })
    }
}

// 删除
impl PersonService {
    /// 删除人物（高危操作，仅管理员）: 清空其所有人脸归属后删除人物
    #[tracing::instrument(
        skip_all,
        fields(admin_user_id = %admin, person_id = %person_id)
    )]
    /// 删除人物并清理其人脸归属及相关缓存.
    pub async fn delete_person(
        state: &PhotoState,
        admin: AdminId,
        person_id: PersonId,
    ) -> Result<()> {
        state
            .repo
            .transaction(|txn| {
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

        // 失效人物缓存（L1 + L2）
        // 错误不返回
        state.repo.invalidate_persons(&[person_id]).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PersonService;
    use insight_face_rs::types::{DIMS, FaceEmbedding};
    use types::photo::{face::FaceId, person::PersonId};

    fn embedding_with(x: f32) -> FaceEmbedding {
        let mut arr = [0.0f32; DIMS];
        arr[0] = x;
        arr[1] = 1.0 - x;
        FaceEmbedding(arr).normalize()
    }

    fn person(id: i64, centroid: FaceEmbedding) -> (PersonId, FaceEmbedding) {
        (PersonId(id), centroid)
    }

    fn face(id: i64, embedding: FaceEmbedding) -> (FaceId, FaceEmbedding) {
        (FaceId(id), embedding)
    }

    fn classify(
        faces: &[(FaceId, FaceEmbedding)],
        persons: &[(PersonId, FaceEmbedding)],
        threshold: f32,
    ) -> Vec<(FaceId, PersonId)> {
        let face_refs = faces.iter().map(|(id, e)| (*id, e)).collect::<Vec<_>>();
        let person_refs = persons.iter().map(|(id, c)| (*id, c)).collect::<Vec<_>>();
        PersonService::classify_unassigned(&face_refs, &person_refs, threshold)
    }

    #[test]
    fn classify_assigns_to_most_similar_person_above_threshold() {
        let persons = vec![
            person(1, embedding_with(0.9)),
            person(2, embedding_with(0.1)),
        ];
        let faces = vec![face(10, embedding_with(0.95))];
        let result = classify(&faces, &persons, 0.55);
        assert_eq!(result, vec![(FaceId(10), PersonId(1))]);
    }

    #[test]
    fn classify_keeps_below_threshold_unassigned() {
        let persons = vec![person(1, embedding_with(0.9))];
        // (0,1) 与 (0.9,0.1) 归一化后余弦 ≈ 0.11, 低于阈值
        let faces = vec![face(10, embedding_with(0.0))];
        let result = classify(&faces, &persons, 0.55);
        assert!(result.is_empty());
    }

    #[test]
    fn classify_picks_highest_similarity_person() {
        let persons = vec![
            person(1, embedding_with(0.2)),
            person(2, embedding_with(0.8)),
        ];
        let faces = vec![face(10, embedding_with(0.85))];
        let result = classify(&faces, &persons, 0.5);
        assert_eq!(result, vec![(FaceId(10), PersonId(2))]);
    }

    #[test]
    fn classify_empty_inputs_returns_empty() {
        assert!(classify(&[], &[], 0.55).is_empty());
        let persons = vec![person(1, embedding_with(0.9))];
        assert!(classify(&[], &persons, 0.55).is_empty());
        let faces = vec![face(10, embedding_with(0.9))];
        assert!(classify(&faces, &[], 0.55).is_empty());
    }

    #[test]
    fn classify_exact_threshold_is_assigned() {
        let persons = vec![person(1, embedding_with(0.9))];
        let faces = vec![face(10, embedding_with(0.9))];
        // 相同单位向量余弦 = 1.0, 边界上应被指派
        let result = classify(&faces, &persons, 1.0);
        assert_eq!(result, vec![(FaceId(10), PersonId(1))]);
    }
}
