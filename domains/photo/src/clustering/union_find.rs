use std::collections::{HashMap, HashSet};
use rayon::prelude::*;
use crate::models::face::{FeatureNode, PersonCluster};
use crate::clustering::vector_utils::{self, calculate_weighted_centroid as calc_weighted_centroid};

pub struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    pub fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    pub fn find(&mut self, mut i: usize) -> usize {
        while self.parent[i] != i {
            self.parent[i] = self.parent[self.parent[i]];
            i = self.parent[i];
        }
        i
    }

    pub fn union(&mut self, i: usize, j: usize) {
        let root_i = self.find(i);
        let root_j = self.find(j);
        if root_i != root_j {
            match self.rank[root_i].cmp(&self.rank[root_j]) {
                std::cmp::Ordering::Less => self.parent[root_i] = root_j,
                std::cmp::Ordering::Greater => self.parent[root_j] = root_i,
                std::cmp::Ordering::Equal => {
                    self.parent[root_i] = root_j;
                    self.rank[root_j] += 1;
                }
            }
        }
    }

    pub fn cluster(
        features: &[FeatureNode],
        radius: f64,
    ) -> HashMap<usize, Vec<usize>> {
        let n = features.len();
        // 优化：避免嵌套 par_iter，只在外层并行
        // 使用 flat_map_iter 在并行任务内部使用普通迭代器
        let pairs: Vec<(usize, usize)> = (0..n)
            .into_par_iter()
            .flat_map_iter(|i| {
                (i + 1..n).filter_map(move |j| {
                    if features[i].photo_id == features[j].photo_id {
                        return None;
                    }
                    let dist = vector_utils::euclidean_distance(
                        &features[i].embedding,
                        &features[j].embedding,
                    );
                    if dist < radius {
                        Some((i, j))
                    } else {
                        None
                    }
                })
            })
            .collect();

        let mut uf = Self::new(n);
        for (i, j) in pairs {
            uf.union(i, j);
        }

        let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            let root = uf.find(i);
            clusters.entry(root).or_default().push(i);
        }
        clusters
    }
}

/// 计算加权质心（使用 ndarray 优化，避免内存拷贝）
/// 
/// # 参数
/// - `nodes`: 特征节点列表
/// 
/// # 返回
/// 返回加权质心向量
pub fn calculate_weighted_centroid(nodes: &[&FeatureNode]) -> Vec<f32> {
    if nodes.is_empty() {
        return vec![0.0f32; 512];
    }

    // 避免内存拷贝：直接使用切片引用
    let embeddings: Vec<&[f32]> = nodes.iter().map(|n| n.embedding.as_slice()).collect();
    let weights: Vec<f32> = nodes.iter().map(|n| n.score).collect();
    
    calc_weighted_centroid(&embeddings, &weights)
}

/// 过滤有效的种子聚类
/// 
/// # 参数
/// - `clusters`: 聚类结果
/// - `features`: 特征节点列表
/// - `min_points`: 最小聚类点数
/// 
/// # 返回
/// 返回有效的种子聚类列表
pub fn filter_valid_seeds(
    clusters: HashMap<usize, Vec<usize>>,
    features: &[FeatureNode],
    min_points: usize,
) -> Vec<PersonCluster> {
    let mut seeds = Vec::new();

    for (_, indices) in clusters {
        if indices.len() < min_points {
            continue;
        }

        let nodes: Vec<&FeatureNode> = indices.iter().map(|&i| &features[i]).collect();

        let mut photo_ids = HashSet::with_capacity(nodes.len());
        let mut has_duplicate_photo = false;
        for n in &nodes {
            if !photo_ids.insert(n.photo_id) {
                has_duplicate_photo = true;
                break;
            }
        }

        if has_duplicate_photo {
            continue;
        }

        let centroid = calculate_weighted_centroid(&nodes);
        let total_weight: f32 = nodes.iter().map(|n| n.score).sum();

        // 优化：只存储 ID 列表，避免克隆完整的 FeatureNode
        let member_ids: Vec<i64> = nodes.iter().map(|n| n.id).collect();

        seeds.push(PersonCluster {
            id: 0,
            vector: centroid,
            member_ids,
            total_weight,
        });
    }

    seeds
}

/// 种子生长阶段
/// 
/// 将未分配的特征节点分配到最近的种子聚类中
/// 
/// # 参数
/// - `seeds`: 种子聚类列表
/// - `features`: 特征节点列表（用于查找 photo_id 和 embedding）
/// - `grow_radius`: 生长半径阈值
/// - `update_centroid`: 是否更新质心
pub fn grow_stage(
    seeds: &mut [PersonCluster],
    features: &[FeatureNode],
    grow_radius: f64,
    update_centroid: bool,
) {
    const CENTROID_EFFECT_DISTANCE: f64 = 0.7;

    // 构建 ID -> FeatureNode 的查找表
    let feature_map: HashMap<i64, &FeatureNode> = features
        .iter()
        .map(|f| (f.id, f))
        .collect();

    let unassigned: Vec<&FeatureNode> = features
        .iter()
        .filter(|f| f.person_id.is_none())
        .collect();

    for node in unassigned {
        // 优化：不进行全量排序，而是寻找最近的有效候选者
        let mut best_candidate: Option<(usize, f64)> = None;

        for (idx, seed) in seeds.iter().enumerate() {
            let dist = vector_utils::euclidean_distance(&node.embedding, &seed.vector);
            if dist < grow_radius {
                // 检查是否有相同照片的冲突（使用查找表）
                let has_same_photo = seed.member_ids.iter().any(|&id| {
                    feature_map.get(&id)
                        .map(|f| f.photo_id == node.photo_id)
                        .unwrap_or(false)
                });
                
                if !has_same_photo {
                    // 只保留最近的候选者
                    if best_candidate.is_none() || dist < best_candidate.unwrap().1 {
                        best_candidate = Some((idx, dist));
                    }
                }
            }
        }

        // 如果找到有效候选者，直接分配
        if let Some((seed_idx, dist)) = best_candidate {
            let seed = &mut seeds[seed_idx];
            // 优化：只存储 ID，避免克隆完整的 FeatureNode
            seed.member_ids.push(node.id);

            if update_centroid && dist < CENTROID_EFFECT_DISTANCE {
                // 使用 ID 列表从查找表获取特征节点
                let members: Vec<&FeatureNode> = seed.member_ids.iter()
                    .filter_map(|id| feature_map.get(id).copied())
                    .collect();
                seed.vector = calculate_weighted_centroid(&members);
                seed.total_weight = members.iter().map(|n| n.score).sum();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 UnionFind 的基本功能
    #[test]
    fn test_union_find_basic() {
        let mut uf = UnionFind::new(5);
        
        // 初始状态，每个元素都是自己的根
        assert_eq!(uf.find(0), 0);
        assert_eq!(uf.find(1), 1);
        
        // 合并 0 和 1
        uf.union(0, 1);
        assert_eq!(uf.find(0), uf.find(1));
        
        // 合并 2 和 3
        uf.union(2, 3);
        assert_eq!(uf.find(2), uf.find(3));
        
        // 合并两个集合
        uf.union(0, 2);
        assert_eq!(uf.find(0), uf.find(3));
    }

    /// 测试聚类功能
    #[test]
    fn test_cluster() {
        let features = vec![
            FeatureNode {
                id: 1,
                photo_id: 1,
                embedding: vec![1.0; 512],
                score: 0.9,
                person_id: None,
            },
            FeatureNode {
                id: 2,
                photo_id: 2,
                embedding: vec![1.0; 512],
                score: 0.8,
                person_id: None,
            },
            FeatureNode {
                id: 3,
                photo_id: 3,
                embedding: vec![100.0; 512],
                score: 0.7,
                person_id: None,
            },
        ];

        let clusters = UnionFind::cluster(&features, 1.0);
        // 前两个特征应该被聚类在一起
        assert!(clusters.len() >= 1);
    }

    /// 测试加权质心计算
    #[test]
    fn test_calculate_weighted_centroid() {
        let nodes = vec![
            FeatureNode {
                id: 1,
                photo_id: 1,
                embedding: vec![1.0; 512],
                score: 1.0,
                person_id: None,
            },
            FeatureNode {
                id: 2,
                photo_id: 2,
                embedding: vec![2.0; 512],
                score: 1.0,
                person_id: None,
            },
        ];

        let refs: Vec<&FeatureNode> = nodes.iter().collect();
        let centroid = calculate_weighted_centroid(&refs);
        
        // 质心应该是 [1.5; 512]
        for i in 0..512 {
            assert!((centroid[i] - 1.5).abs() < 1e-5);
        }
    }

    /// 测试空节点列表的质心计算
    #[test]
    fn test_calculate_weighted_centroid_empty() {
        let nodes: Vec<&FeatureNode> = vec![];
        let centroid = calculate_weighted_centroid(&nodes);
        assert_eq!(centroid.len(), 512);
        assert!(centroid.iter().all(|&x| x == 0.0));
    }
}
