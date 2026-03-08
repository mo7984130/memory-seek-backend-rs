use std::collections::HashMap;

use crate::models::face::{FeatureNode, PersonCluster};

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

    pub fn find(&mut self, i: usize) -> usize {
        if self.parent[i] != i {
            self.parent[i] = self.find(self.parent[i]);
        }
        self.parent[i]
    }

    pub fn union(&mut self, i: usize, j: usize) {
        let (root_i, root_j) = (self.find(i), self.find(j));
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
        let mut uf = Self::new(n);

        for i in 0..n {
            for j in (i + 1)..n {
                if features[i].photo_id == features[j].photo_id {
                    continue;
                }
                let dist = super::vector_utils::euclidean_distance(
                    &features[i].embedding,
                    &features[j].embedding,
                );
                if dist < radius {
                    uf.union(i, j);
                }
            }
        }

        let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            let root = uf.find(i);
            clusters.entry(root).or_default().push(i);
        }
        clusters
    }
}

pub fn calculate_weighted_centroid(nodes: &[&FeatureNode]) -> Vec<f32> {
    const DIM: usize = 512;
    let total_weight: f32 = nodes.iter().map(|n| n.score).sum();
    let mut centroid = vec![0.0f32; DIM];

    for node in nodes {
        for i in 0..DIM {
            centroid[i] += node.embedding[i] * node.score;
        }
    }

    for i in 0..DIM {
        centroid[i] /= total_weight;
    }
    centroid
}

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

        let has_duplicate_photo = nodes
            .iter()
            .map(|n| n.photo_id)
            .collect::<std::collections::HashSet<_>>()
            .len()
            < nodes.len();

        if has_duplicate_photo {
            continue;
        }

        let centroid = calculate_weighted_centroid(&nodes);
        let total_weight: f32 = nodes.iter().map(|n| n.score).sum();
        let member_nodes: Vec<FeatureNode> = nodes.into_iter().cloned().collect();

        seeds.push(PersonCluster {
            id: 0,
            vector: centroid,
            member_nodes,
            total_weight,
        });
    }

    seeds
}

pub fn grow_stage(
    seeds: &mut [PersonCluster],
    features: &[FeatureNode],
    grow_radius: f64,
    update_centroid: bool,
) {
    const CENTROID_EFFECT_DISTANCE: f64 = 0.7;

    let unassigned: Vec<&FeatureNode> = features
        .iter()
        .filter(|f| f.person_id.is_none())
        .collect();

    for node in unassigned {
        let mut nearest: Option<(usize, f64)> = None;

        for (idx, seed) in seeds.iter().enumerate() {
            let dist = super::vector_utils::euclidean_distance(&node.embedding, &seed.vector);
            if dist < grow_radius {
                if nearest.is_none() || dist < nearest.unwrap().1 {
                    nearest = Some((idx, dist));
                }
            }
        }

        if let Some((seed_idx, dist)) = nearest {
            let seed = &mut seeds[seed_idx];

            let has_same_photo = seed.member_nodes.iter().any(|n| n.photo_id == node.photo_id);
            if has_same_photo {
                continue;
            }

            let node_owned = node.clone();
            seed.member_nodes.push(node_owned);

            if update_centroid && dist < CENTROID_EFFECT_DISTANCE {
                seed.vector = calculate_weighted_centroid(
                    &seed.member_nodes.iter().collect::<Vec<_>>(),
                );
                seed.total_weight = seed.member_nodes.iter().map(|n| n.score).sum();
            }
        }
    }
}
