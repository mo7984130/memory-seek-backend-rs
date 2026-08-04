//! `FaceEmbedding` 纯数学工具(无状态, 供 service 层复用)
//!
//! 约定: `photo_person.centroid` 列存储 **score 加权未归一化向量和**
//! Σ(score × embedding), 读取/检索时再 normalize。
//! 归一化会丢失模长, 无法由归一化质心反推加权和, 因此增量维护必须基于原始和
//! (详见 `docs/change-face-belonging-plan.md`)。

use insight_face_rs::{FaceEmbedding, types::DIMS};

/// score 加权和: 返回 `(weight, centroid)`
///
/// - `weight` = Σscore(f64, 对应 `photo_person.weight` 列)
/// - `centroid` = Σ(score × embedding)(未归一化)
pub(crate) fn weighted_sum<'a>(
    items: impl IntoIterator<Item = (f32, &'a FaceEmbedding)>,
) -> (f64, FaceEmbedding) {
    let mut weight = 0.0f64;
    let mut sum = [0.0f32; DIMS];
    for (score, embedding) in items {
        weight += score as f64;
        for (i, value) in embedding.iter().enumerate() {
            sum[i] += score * value;
        }
    }
    (weight, FaceEmbedding(sum))
}

/// 向量和: c + d(合并人物用)
pub(crate) fn add(c: &FaceEmbedding, d: &FaceEmbedding) -> FaceEmbedding {
    let mut out = [0.0f32; DIMS];
    for (o, (x, y)) in out.iter_mut().zip(c.0.iter().zip(d.0.iter())) {
        *o = x + y;
    }
    FaceEmbedding(out)
}

/// 向量增量加: c + score × e(转移人脸到新人物用)
pub(crate) fn add_scaled(c: &FaceEmbedding, e: &FaceEmbedding, score: f32) -> FaceEmbedding {
    let mut out = [0.0f32; DIMS];
    for (o, (x, y)) in out.iter_mut().zip(c.0.iter().zip(e.0.iter())) {
        *o = x + score * y;
    }
    FaceEmbedding(out)
}

/// 向量增量减: c − score × e(从旧人物移除人脸用)
pub(crate) fn sub_scaled(c: &FaceEmbedding, e: &FaceEmbedding, score: f32) -> FaceEmbedding {
    let mut out = [0.0f32; DIMS];
    for (o, (x, y)) in out.iter_mut().zip(c.0.iter().zip(e.0.iter())) {
        *o = x - score * y;
    }
    FaceEmbedding(out)
}
