use ndarray::{Array1, ArrayView1};
use rayon::prelude::*;

/// 欧几里得距离（使用 ndarray SIMD 优化）
/// 
/// # 参数
/// - `a`: 第一个向量
/// - `b`: 第二个向量
/// 
/// # 返回
/// 返回两个向量之间的欧几里得距离
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f64 {
    let a_arr = ArrayView1::from(a).mapv(|x| x as f64);
    let b_arr = ArrayView1::from(b).mapv(|x| x as f64);
    (&a_arr - &b_arr).mapv(|x| x * x).sum().sqrt()
}

/// L2 归一化（使用 ndarray）
/// 
/// # 参数
/// - `vec`: 待归一化的向量
/// 
/// # 返回
/// 返回归一化后的向量
pub fn l2_normalize(vec: &[f32]) -> Vec<f32> {
    let arr = Array1::from_vec(vec.to_vec());
    let norm = arr.mapv(|x| x * x).sum().sqrt();
    if norm == 0.0 {
        return vec.to_vec();
    }
    (arr / norm).to_vec()
}

/// 余弦相似度（使用 ndarray）
/// 
/// # 参数
/// - `a`: 第一个向量
/// - `b`: 第二个向量
/// 
/// # 返回
/// 返回两个向量之间的余弦相似度
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let a_arr = ArrayView1::from(a);
    let b_arr = ArrayView1::from(b);
    let dot = a_arr.dot(&b_arr);
    let norm_a = a_arr.mapv(|x| x * x).sum().sqrt();
    let norm_b = b_arr.mapv(|x| x * x).sum().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// 批量欧几里得距离计算（使用 rayon 并行）
/// 
/// # 参数
/// - `query`: 查询向量
/// - `targets`: 目标向量列表
/// 
/// # 返回
/// 返回查询向量与每个目标向量之间的距离列表
pub fn euclidean_distance_batch(query: &[f32], targets: &[&[f32]]) -> Vec<f64> {
    targets
        .par_iter()
        .map(|t| euclidean_distance(query, t))
        .collect()
}

/// 计算质心（使用 ndarray）
/// 
/// # 参数
/// - `embeddings`: 嵌入向量切片列表（避免内存拷贝）
/// 
/// # 返回
/// 返回所有向量的平均质心
pub fn calculate_centroid(embeddings: &[&[f32]]) -> Vec<f32> {
    if embeddings.is_empty() {
        return vec![0.0; 512];
    }
    let n = embeddings.len() as f32;
    let dim = embeddings[0].len();

    let mut sum = Array1::zeros(dim);
    for emb in embeddings {
        sum = sum + ArrayView1::from(*emb);
    }
    (sum / n).to_vec()
}

/// 加权质心计算（使用 ndarray，避免内存拷贝）
/// 
/// # 参数
/// - `embeddings`: 嵌入向量切片列表（避免内存拷贝）
/// - `weights`: 权重列表
/// 
/// # 返回
/// 返回加权质心向量
pub fn calculate_weighted_centroid(embeddings: &[&[f32]], weights: &[f32]) -> Vec<f32> {
    if embeddings.is_empty() || weights.is_empty() {
        return vec![0.0; 512];
    }
    let dim = embeddings[0].len();
    let total_weight: f32 = weights.iter().sum();
    if total_weight == 0.0 {
        return vec![0.0; dim];
    }

    let mut weighted_sum = Array1::zeros(dim);
    for (emb, w) in embeddings.iter().zip(weights.iter()) {
        let emb_arr = ArrayView1::from(*emb);
        weighted_sum = weighted_sum + emb_arr.mapv(|x| x * *w);
    }
    (weighted_sum / total_weight).to_vec()
}

/// 加权合并两个向量
/// 
/// # 参数
/// - `a`: 第一个向量
/// - `weight_a`: 第一个向量的权重
/// - `b`: 第二个向量
/// - `weight_b`: 第二个向量的权重
/// 
/// # 返回
/// 返回加权合并后的向量
pub fn weighted_merge(a: &[f32], weight_a: f32, b: &[f32], weight_b: f32) -> Vec<f32> {
    let a_arr = Array1::from_vec(a.to_vec());
    let b_arr = Array1::from_vec(b.to_vec());
    let total_weight = weight_a + weight_b;
    if total_weight == 0.0 {
        return vec![0.0; a.len()];
    }
    ((a_arr * weight_a + b_arr * weight_b) / total_weight).to_vec()
}

/// 向量减量计算（从质心中减去一个向量）
/// 
/// # 参数
/// - `centroid`: 当前质心
/// - `weight`: 当前总权重
/// - `embedding`: 要减去的向量
/// 
/// # 返回
/// 返回新的质心
pub fn decrement_centroid(centroid: &[f32], weight: f32, embedding: &[f32]) -> Vec<f32> {
    if weight <= 1.0 {
        return vec![0.0; centroid.len()];
    }
    let new_weight = weight - 1.0;
    let centroid_arr = Array1::from_vec(centroid.to_vec());
    let emb_arr = Array1::from_vec(embedding.to_vec());
    ((centroid_arr * weight - emb_arr) / new_weight).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试欧几里得距离计算的正确性
    #[test]
    fn test_euclidean_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let dist = euclidean_distance(&a, &b);
        assert!((dist - std::f64::consts::SQRT_2).abs() < 1e-5);

        let c = vec![1.0, 2.0, 3.0];
        let d = vec![4.0, 5.0, 6.0];
        let dist2 = euclidean_distance(&c, &d);
        assert!((dist2 - 5.1961524).abs() < 1e-5);
    }

    /// 测试 L2 归一化的正确性
    #[test]
    fn test_l2_normalize() {
        let vec = vec![3.0, 4.0];
        let normalized = l2_normalize(&vec);
        let norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);

        // 测试零向量
        let zero = vec![0.0, 0.0, 0.0];
        let normalized_zero = l2_normalize(&zero);
        assert_eq!(normalized_zero, zero);
    }

    /// 测试余弦相似度计算的正确性
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-5);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        let sim2 = cosine_similarity(&c, &d);
        assert!((sim2 - 0.0).abs() < 1e-5);

        // 测试零向量
        let zero = vec![0.0, 0.0, 0.0];
        let sim3 = cosine_similarity(&a, &zero);
        assert!((sim3 - 0.0).abs() < 1e-5);
    }

    /// 测试批量欧几里得距离计算的正确性
    #[test]
    fn test_euclidean_distance_batch() {
        let query = vec![1.0, 0.0, 0.0];
        let targets: Vec<&[f32]> = vec![
            &[0.0, 1.0, 0.0],
            &[0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0],
        ];
        let distances = euclidean_distance_batch(&query, &targets);
        assert_eq!(distances.len(), 3);
        assert!((distances[0] - std::f64::consts::SQRT_2).abs() < 1e-5);
        assert!((distances[1] - std::f64::consts::SQRT_2).abs() < 1e-5);
        assert!((distances[2] - 0.0).abs() < 1e-5);
    }

    /// 测试质心计算的正确性
    #[test]
    fn test_calculate_centroid() {
        let emb1 = vec![1.0, 2.0, 3.0];
        let emb2 = vec![4.0, 5.0, 6.0];
        let emb3 = vec![7.0, 8.0, 9.0];
        let embeddings: Vec<&[f32]> = vec![&emb1, &emb2, &emb3];
        
        let centroid = calculate_centroid(&embeddings);
        assert_eq!(centroid.len(), 3);
        assert!((centroid[0] - 4.0).abs() < 1e-5);
        assert!((centroid[1] - 5.0).abs() < 1e-5);
        assert!((centroid[2] - 6.0).abs() < 1e-5);

        // 测试空列表
        let empty: Vec<&[f32]> = vec![];
        let empty_centroid = calculate_centroid(&empty);
        assert_eq!(empty_centroid.len(), 512);
    }

    /// 测试加权质心计算的正确性
    #[test]
    fn test_calculate_weighted_centroid() {
        let emb1 = vec![1.0, 0.0];
        let emb2 = vec![0.0, 1.0];
        let embeddings: Vec<&[f32]> = vec![&emb1, &emb2];
        let weights = vec![1.0, 1.0];
        
        let centroid = calculate_weighted_centroid(&embeddings, &weights);
        assert!((centroid[0] - 0.5).abs() < 1e-5);
        assert!((centroid[1] - 0.5).abs() < 1e-5);

        // 测试不同权重
        let weights2 = vec![2.0, 1.0];
        let centroid2 = calculate_weighted_centroid(&embeddings, &weights2);
        assert!((centroid2[0] - 2.0 / 3.0).abs() < 1e-5);
        assert!((centroid2[1] - 1.0 / 3.0).abs() < 1e-5);
    }

    /// 测试加权合并的正确性
    #[test]
    fn test_weighted_merge() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let merged = weighted_merge(&a, 1.0, &b, 1.0);
        assert!((merged[0] - 0.5).abs() < 1e-5);
        assert!((merged[1] - 0.5).abs() < 1e-5);

        // 测试不同权重
        let merged2 = weighted_merge(&a, 2.0, &b, 1.0);
        assert!((merged2[0] - 2.0 / 3.0).abs() < 1e-5);
        assert!((merged2[1] - 1.0 / 3.0).abs() < 1e-5);
    }

    /// 测试向量减量计算的正确性
    #[test]
    fn test_decrement_centroid() {
        let centroid = vec![3.0, 3.0];
        let embedding = vec![1.0, 1.0];
        let new_centroid = decrement_centroid(&centroid, 3.0, &embedding);
        assert!((new_centroid[0] - 4.0).abs() < 1e-5);
        assert!((new_centroid[1] - 4.0).abs() < 1e-5);
    }
}
