use std::{collections::HashMap, sync::Arc};

use crate::{
    PhotoRepo, PhotoState,
    mappers::{face_mapper::FaceMapper, person_mapper::PersonMapper},
    repo::{FaceRepo, PersonRepo},
    services::photo_service::PhotoService,
};
use audit::{AuditEvent, AuditRecorder};
use common::{
    Result, db_transaction,
    error::{
        AppError,
        contextual::ext::{IntoContextualExt, OptionExt, ResultContextualExt},
    },
    ext::ToOk,
    metrics_name,
    types::CursorPage,
    utils::MetricsTimerExt,
};
use insight_face_rs::{FaceEmbedding, types::DIMS};
use ndarray::Array2;
use petal_clustering::{Fit, HDbscan};
use tokio::{spawn, task::spawn_blocking};
use tracing::{info, instrument};
use types::{
    auth::user::{AdminId, UserId},
    cursor::{CountIdCursor, TimeIdCursor},
    photo::{
        MergePersonParam, PersonCursorParam, PersonPhotoCursorParam, PersonSearchParam, PersonView,
        PhotoView, RenamePersonParam, SecondaryClusterParam,
        face::FaceRecord,
        person::{NewPerson, PersonCover, PersonId},
        photo::PhotoId,
    },
};

pub struct PersonService;

// 创建
impl PersonService {
    /// 异步启动全量人物扫描任务.
    #[instrument(skip_all, fields(admin_user_id = %admin))]
    pub async fn full_scan(state: Arc<PhotoState>, admin: AdminId) -> Result<()> {
        let user_id = admin.into_inner();
        let admin = AdminId::new(user_id)?;
        spawn(async move { Self::inner_full_scan(state, admin).await });
        Ok(())
    }

    #[common_macros::metered(name = "person_full_scan")]
    /// 执行全量扫描, 重建人物聚类和人脸归属.
    pub async fn inner_full_scan(state: Arc<PhotoState>, admin: AdminId) -> Result<()> {
        let user_id = admin.into_inner();
        info!(user_id = %user_id, "管理员触发人物全量聚类");

        info!("加载照片数据");
        let (faces, photo_file_ids) = PersonRepo::load_faces_with_photo_files(&state).await?;

        info!("加载照片数据完成");

        info!("开始聚类");
        let embedding_array = {
            let embeddings: Vec<f32> = faces
                .iter()
                .flat_map(|f| f.embedding.iter().copied())
                .collect();
            Array2::from_shape_vec((faces.len(), DIMS), embeddings).context_err(
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
        FaceRepo::backup_and_truncate(&state).await?;
        info!("保存表完成");
        db_transaction!(scoped & state.db, |txn| {
            FaceMapper::clean_person_id(txn).await?;
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
                    cover: PersonCover {
                        face_id: cover_face.id,
                        photo_id: cover_face.photo_id,
                        face_score: cover_face.score,
                        file_id: photo_file_ids
                            .get(&cover_face.photo_id)
                            .cloned()
                            .ok_or_error(
                                "person_cover_photo_not_found",
                                "封面人脸所属照片不存在",
                                AppError::InternalServerError,
                            )?,
                        bbox: cover_face.bbox.into(),
                    },
                    face_count: embedding_ids.len() as u64,
                };
                let person = PersonMapper::insert(txn, person).await?;

                // 更新人脸归属
                FaceMapper::update_person_ids(
                    txn,
                    Some(person.id),
                    embedding_ids.iter().map(|idx| faces[*idx].id),
                )
                .await?;
            }
            info!("插入人物完成");
            info!("人脸聚类完成");
            AuditRecorder::append(
                txn,
                AuditEvent::new("person_full_scan").with_actor(user_id.0),
            )
            .await?;
            Ok(())
        })
        .await?;

        Ok(())
    }

    /// 二次聚类: 将全部未分配人脸(`person_id IS NULL`)按 centroid 余弦相似度
    /// 指派到已有人物, 低于阈值的人脸保持未分配。
    #[instrument(skip_all, fields(admin_id = %admin))]
    pub async fn assign_unassigned_faces(
        state: Arc<PhotoState>,
        admin: AdminId,
        req: SecondaryClusterParam,
    ) -> Result<()> {
        spawn(async move { Self::inner_assign_unassigned_faces(state, admin, req).await });
        Ok(())
    }

    #[common_macros::metered(name = "person_secondary_cluster")]
    /// 执行未分配人脸与人物质心的匹配和归属更新.
    async fn inner_assign_unassigned_faces(
        state: Arc<PhotoState>,
        admin: AdminId,
        req: SecondaryClusterParam,
    ) -> Result<()> {
        let SecondaryClusterParam { threshold } = req;
        info!(admin_id = %admin, threshold, "管理员触发二次聚类");
        db_transaction!(scoped & state.db, |txn| {
            AuditRecorder::append(
                txn,
                AuditEvent::new("assign_unassigned_faces_start").with_actor(admin.into_inner()),
            )
            .await?;
            Ok(())
        })
        .await?;

        let faces = FaceRepo::lock_unassigned_faces(&state).await?;
        let persons = PersonRepo::query_all(&state).await?;
        info!(
            "加载完成: 未分配人脸 {} 张, 人物 {} 个",
            faces.len(),
            persons.len()
        );
        if faces.is_empty() || persons.is_empty() {
            return Ok(());
        }

        // 内存分类(CPU 密集, 放入阻塞线程池; faces, persons 原样带回)
        let classified = spawn_blocking(move || {
            let face_embeddings = faces.iter().map(|f| &f.embedding).collect::<Vec<_>>();
            let person_refs = persons
                .iter()
                .map(|p| (p.id, &p.centroid))
                .collect::<Vec<_>>();
            let classified = Self::classify_unassigned(&face_embeddings, &person_refs, threshold);
            (classified, faces, persons)
        })
        .await
        .into_contextual()?;
        let (classified, faces, persons) = classified;

        let matched = classified.len();
        let unmatched = faces.len() - matched;
        info!("分类完成: 匹配 {} 张, 未匹配 {} 张", matched, unmatched);
        if classified.is_empty() {
            return Ok(());
        }

        // 按人物分组聚合
        let mut person_map = persons
            .into_iter()
            .map(|p| (p.id, p))
            .collect::<HashMap<_, _>>();
        let mut per_person: HashMap<PersonId, Vec<FaceRecord>> = HashMap::new();
        for (face_index, face) in faces.into_iter().enumerate() {
            if let Some(person_id) = classified.get(&face_index) {
                per_person.entry(*person_id).or_default().push(face);
            }
        }
        for (person_id, faces) in per_person {
            if let Some(person) = person_map.remove(&person_id) {
                PersonRepo::add_faces(&state, person, faces).await?;
            }
        }

        db_transaction!(scoped & state.db, |txn| {
            AuditRecorder::append(
                txn,
                AuditEvent::new("assign_unassigned_faces_finish").with_actor(admin.into_inner()),
            )
            .await?;
            Ok(())
        })
        .await?;

        Ok(())
    }

    /// 最近质心分类: 每张未分配人脸取与各人物 centroid(已归一化)余弦相似度最高者,
    /// 相似度 `>= threshold` 才指派, 否则保持未分配。
    /// 返回(index, person_id)
    fn classify_unassigned(
        face_embeddings: &[&FaceEmbedding],
        persons: &[(PersonId, &FaceEmbedding)],
        threshold: f32,
    ) -> HashMap<usize, PersonId> {
        let mut assignments = HashMap::new();
        for (index, embedding) in face_embeddings.iter().enumerate() {
            let mut best: Option<(f32, PersonId)> = None;
            for (person_id, centroid) in persons {
                let sim = embedding.cosine_similarity(centroid);
                if sim >= threshold && best.is_none_or(|(b, _)| sim > b) {
                    best = Some((sim, *person_id));
                }
            }
            if let Some((_, person_id)) = best {
                assignments.insert(index, person_id);
            }
        }
        assignments
    }
}

// 修改
impl PersonService {
    /// 重命名人物
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(person_id = %person_id))]
    pub async fn rename_person(
        state: &PhotoState,
        person_id: PersonId,
        req: RenamePersonParam,
        user_id: UserId,
    ) -> Result<()> {
        let new_name = req.new_name.into_inner();
        let new_name_initials = Self::compute_name_initials(&new_name);
        PersonRepo::rename_person(state, person_id, new_name, new_name_initials, user_id).await?;

        Ok(())
    }

    /// 合并人物
    /// 将 source 的全部人脸归属转移到 target, 并删除 source
    /// 仅可管理员执行
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(admin_user_id = %admin))]
    pub async fn merge_person(
        state: &PhotoState,
        admin: AdminId,
        req: MergePersonParam,
    ) -> Result<PersonView> {
        let person = PersonRepo::merge_person(state, admin, req).await?;

        let file_id = &person.cover.file_id.clone();
        PersonView::from_record(
            person,
            admin.into_inner(),
            PhotoRepo::get_photo_dimensions(state, file_id).await?,
        )
        .to_ok()
    }
}

// 查询
impl PersonService {
    #[common_macros::metered]
    #[tracing::instrument(skip_all, fields(user_id = %user_id))]
    /// 查询人物列表.
    pub async fn get_persons(
        state: &PhotoState,
        user_id: UserId,
        req: PersonCursorParam,
    ) -> Result<CursorPage<PersonView, CountIdCursor<PersonId>>> {
        let CursorPage {
            records, has_more, ..
        } = PersonRepo::query_page(state, req.cursor, req.size).await?;

        let mut views = Vec::with_capacity(records.len());
        for person in records.into_iter() {
            let file_id = person.cover.file_id.clone();
            views.push(PersonView::from_record(
                person,
                user_id,
                PhotoRepo::get_photo_dimensions(state, &file_id).await?,
            ));
        }
        CursorPage::from_has_more(views, has_more)
            .with_next_cursor(|view| CountIdCursor {
                count: view.face_count,
                id: view.id,
            })
            .to_ok()
    }

    /// 按关键词前缀搜索人物(匹配完整名字或姓名首字母)
    #[common_macros::metered]
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
        let CursorPage {
            records, has_more, ..
        } = PersonRepo::search_person_page(state, &keyword, cursor, size).await?;

        let mut views = Vec::with_capacity(records.len());
        for person in records.into_iter() {
            let file_id = person.cover.file_id.clone();
            views.push(PersonView::from_record(
                person,
                user_id,
                PhotoRepo::get_photo_dimensions(state, &file_id).await?,
            ));
        }
        CursorPage::from_has_more(views, has_more)
            .with_next_cursor(|view| view.id)
            .to_ok()
    }

    /// 获取人物的照片列表(游标分页, 参照 `CollectionPhotoService::get_photos`)
    #[common_macros::metered]
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
        let page = FaceRepo::query_person_photo_ids(state, person_id, &req)
            .timed(metrics_name!("query_photo_ids"))
            .await?;
        if page.records.is_empty() {
            return Ok(CursorPage::empty());
        }

        let photos = PhotoService::load_photos_info(state, user_id, &page.records)
            .timed(metrics_name!("load_photos_info"))
            .await?;

        Ok(page
            .replace_records(photos)
            .with_next_cursor(|photo| TimeIdCursor {
                time_at: photo.created_at,
                id: photo.id,
            }))
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
        PersonRepo::delete_person(state, person_id, admin).await?;
        Ok(())
    }
}

impl PersonService {
    /// 计算姓名首字母(大写, 如 张三 -> ZS, Alice Wang -> AW)
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
}
