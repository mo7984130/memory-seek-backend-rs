# Vector 类型重命名与 Embedding 定义完善 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `DrVector` 重命名为 `PostgreVector`，并引入类型安全的 `Embedding512` 类型替代裸的 512 维向量

**Architecture:** 在 `entities/src/vector.rs` 中定义泛型的 `PostgreVector<N>` 作为 pgvector 的 Sea-ORM 适配层，然后在 `face_feature.rs` 中定义 `Embedding512 = PostgreVector<512>` 作为人脸特征向量的语义类型。所有使用 `DrVector` 的地方统一改为 `PostgreVector`（无维度约束），用于人脸 embedding 的地方改为 `Embedding512`。

**Tech Stack:** Rust, Sea-ORM, pgvector

---

## 文件结构

| 文件 | 操作 | 说明 |
|------|------|------|
| `entities/src/vector.rs` | 重写 | `DrVector` → `PostgreVector`，泛型化维度 |
| `entities/src/photo_entities/face_feature.rs` | 修改 | 定义 `Embedding512` 类型别名，更新字段类型 |
| `entities/src/photo_entities/face_person.rs` | 修改 | `DrVector` → `Embedding512` |
| `entities/src/lib.rs` | 修改 | 导出 `PostgreVector` 和 `Embedding512` |
| `domains/photo/src/mappers/face_feature_mapper.rs` | 修改 | `DrVector` → `Embedding512` |
| `domains/photo/src/mappers/face_person_mapper.rs` | 修改 | `DrVector` → `Embedding512` |
| `domains/photo/src/services/face_service.rs` | 修改 | `DrVector` → `Embedding512` |
| `domains/photo/src/services/feature_service.rs` | 修改 | `DrVector` → `Embedding512` |
| `domains/photo/src/services/photo_service.rs` | 修改 | `DrVector` → `Embedding512` |

---

### Task 1: 重写 `entities/src/vector.rs` — PostgreVector

**Files:**
- Modify: `entities/src/vector.rs`

- [ ] **Step 1: 重写 vector.rs，将 DrVector 替换为 PostgreVector**

```rust
use std::ops::{Deref, DerefMut};

use pgvector::Vector;
use sea_orm::sea_query::{Alias, ColumnType, Nullable, SeaRc, Value, ValueType, ValueTypeErr};
use sea_orm::{ColIdx, QueryResult, TryGetError, TryGetable};
use serde::{Deserialize, Serialize};
use sqlx::Row;

/// PostgreSQL vector 类型的 Sea-ORM 适配层
///
/// 封装 `Vec<f32>`，提供与 pgvector 和 Sea-ORM 的互操作。
/// 泛型参数 `N` 为向量维度（编译时常量），用于类型级别的维度约束。
/// 实际存储和序列化时不校验维度，维度检查由上层语义类型保证。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PostgreVector<const N: usize>(pub Vec<f32>);

impl<const N: usize> PostgreVector<N> {
    pub fn new(v: Vec<f32>) -> Self {
        PostgreVector(v)
    }

    pub fn to_vec(&self) -> Vec<f32> {
        self.0.clone()
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }

    pub fn dim(&self) -> usize {
        self.0.len()
    }
}

impl<const N: usize> From<Vec<f32>> for PostgreVector<N> {
    fn from(vec: Vec<f32>) -> Self {
        Self(vec)
    }
}

impl<const N: usize> From<PostgreVector<N>> for Vec<f32> {
    fn from(val: PostgreVector<N>) -> Self {
        val.0
    }
}

impl<const N: usize> From<Vector> for PostgreVector<N> {
    fn from(v: Vector) -> Self {
        PostgreVector(v.into())
    }
}

impl<const N: usize> From<PostgreVector<N>> for Vector {
    fn from(val: PostgreVector<N>) -> Self {
        Vector::from(val.0)
    }
}

impl<const N: usize> TryGetable for PostgreVector<N> {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let pg_row = res.try_as_pg_row().ok_or_else(|| {
            TryGetError::DbErr(sea_orm::DbErr::Type("Not a PostgreSQL row".into()))
        })?;
        let value: Option<Vector> = pg_row
            .try_get(index.as_sqlx_postgres_index())
            .map_err(|e| TryGetError::DbErr(sea_orm::DbErr::Type(format!("Vector decode: {e}"))))?;
        value
            .map(PostgreVector::from)
            .ok_or_else(|| TryGetError::Null(String::new()))
    }
}

impl<const N: usize> ValueType for PostgreVector<N> {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        match v {
            Value::String(Some(s)) => {
                let data = s
                    .trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .filter_map(|v| v.trim().parse::<f32>().ok())
                    .collect();
                Ok(PostgreVector(data))
            }
            _ => Err(ValueTypeErr),
        }
    }

    fn type_name() -> String {
        format!("PostgreVector<{}>", N)
    }

    fn array_type() -> sea_orm::sea_query::ArrayType {
        sea_orm::sea_query::ArrayType::Float
    }

    fn column_type() -> ColumnType {
        ColumnType::Custom(SeaRc::new(Alias::new("vector")))
    }
}

impl<const N: usize> From<PostgreVector<N>> for Value {
    fn from(val: PostgreVector<N>) -> Self {
        let s = format!(
            "[{}]",
            val.0
                .iter()
                .map(|f| {
                    if f.fract() == 0.0 {
                        format!("{:.1}", f)
                    } else {
                        f.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        );
        Value::String(Some(Box::new(s)))
    }
}

impl<const N: usize> Nullable for PostgreVector<N> {
    fn null() -> Value {
        Value::String(None)
    }
}

impl<const N: usize> Deref for PostgreVector<N> {
    type Target = [f32];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> DerefMut for PostgreVector<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
```

- [ ] **Step 2: 更新 `entities/src/lib.rs` 导出**

```rust
pub mod vector;
pub mod photo_entities;
pub mod user_entities;

pub use vector::PostgreVector;

pub use photo_entities::collection;
pub use photo_entities::collection_photo;
pub use photo_entities::comment;
pub use photo_entities::comment_like;
pub use photo_entities::face_feature;
pub use photo_entities::face_feature::Embedding512;
pub use photo_entities::face_person;
pub use photo_entities::photo;
pub use photo_entities::timeline_stat;

pub use user_entities::user;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p entities`
Expected: 编译失败（下游引用未更新），确认错误来自类型不匹配而非语法错误

- [ ] **Step 4: Commit**

```bash
git add entities/src/vector.rs entities/src/lib.rs
git commit -m "refactor(entities): rename DrVector to PostgreVector with const generic dim"
```

---

### Task 2: 更新 Entity 层 — face_feature 和 face_person

**Files:**
- Modify: `entities/src/photo_entities/face_feature.rs`
- Modify: `entities/src/photo_entities/face_person.rs`

- [ ] **Step 1: 更新 face_feature.rs，定义 Embedding512 并替换字段类型**

```rust
use crate::vector::PostgreVector;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub const FEATURE_DIM: usize = 512;

/// 512 维人脸特征嵌入向量
pub type Embedding512 = PostgreVector<FEATURE_DIM>;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_face_feature")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub photo_id: i64,
    pub person_id: Option<i64>,
    pub embedding: Embedding512,
    #[sea_orm(column_type = "Json")]
    pub bbox: Json,
    pub score: f32,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::photo::Entity",
        from = "Column::PhotoId",
        to = "super::photo::Column::Id"
    )]
    Photo,
    #[sea_orm(
        belongs_to = "super::face_person::Entity",
        from = "Column::PersonId",
        to = "super::face_person::Column::Id"
    )]
    Person,
}

impl Related<super::photo::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Photo.def()
    }
}

impl Related<super::face_person::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Person.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceBBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
```

- [ ] **Step 2: 更新 face_person.rs**

```rust
use crate::vector::PostgreVector;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::face_feature::Embedding512;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "photo_face_person")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub name_initials: Option<String>,
    pub max_score_feature_id: i64,
    pub max_score: f32,
    pub total_photo_count: i64,
    pub centroid_embedding: Embedding512,
    pub total_weight_count: f32,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::face_feature::Entity")]
    FaceFeatures,
}

impl Related<super::face_feature::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FaceFeatures.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p entities`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add entities/src/photo_entities/face_feature.rs entities/src/photo_entities/face_person.rs
git commit -m "refactor(entities): define Embedding512 type alias, update face entity fields"
```

---

### Task 3: 更新 Mapper 层 — face_feature_mapper

**Files:**
- Modify: `domains/photo/src/mappers/face_feature_mapper.rs`

- [ ] **Step 1: 更新导入和参数类型**

将第 5 行：
```rust
use entities::{DrVector, face_feature::*};
```
改为：
```rust
use entities::{face_feature::*, Embedding512};
```

将第 153 行 `insert` 方法的参数类型：
```rust
        embedding: DrVector,
```
改为：
```rust
        embedding: Embedding512,
```

- [ ] **Step 2: 验证编译**

Run: `cargo check --features photo`
Expected: 可能仍有其他文件报错，确认此文件无误

- [ ] **Step 3: Commit**

```bash
git add domains/photo/src/mappers/face_feature_mapper.rs
git commit -m "refactor(photo/mapper): update face_feature_mapper to use Embedding512"
```

---

### Task 4: 更新 Mapper 层 — face_person_mapper

**Files:**
- Modify: `domains/photo/src/mappers/face_person_mapper.rs`

- [ ] **Step 1: 更新导入**

将第 5-9 行：
```rust
use entities::{
    DrVector,
    face_feature::FEATURE_DIM,
    face_person::{self, Column, Entity},
};
```
改为：
```rust
use entities::{
    Embedding512,
    face_feature::FEATURE_DIM,
    face_person::{self, Column, Entity},
};
```

- [ ] **Step 2: 更新 insert 方法参数**

将第 156 行：
```rust
        centroid_embedding: DrVector,
```
改为：
```rust
        centroid_embedding: Embedding512,
```

- [ ] **Step 3: 更新 update 方法参数**

将第 202 行：
```rust
        centroid_embedding: Option<DrVector>,
```
改为：
```rust
        centroid_embedding: Option<Embedding512,
```

- [ ] **Step 4: 更新 decr_by_features 中的构造**

将第 402 行：
```rust
                new_centroid: DrVector::from(new_centroid),
```
改为：
```rust
                new_centroid: Embedding512::from(new_centroid),
```

- [ ] **Step 5: 更新 UpdateInfo 结构体**

将第 536 行：
```rust
    new_centroid: DrVector,
```
改为：
```rust
    new_centroid: Embedding512,
```

- [ ] **Step 6: 验证编译**

Run: `cargo check --features photo`
Expected: 可能仍有其他文件报错，确认此文件无误

- [ ] **Step 7: Commit**

```bash
git add domains/photo/src/mappers/face_person_mapper.rs
git commit -m "refactor(photo/mapper): update face_person_mapper to use Embedding512"
```

---

### Task 5: 更新 Service 层 — face_service

**Files:**
- Modify: `domains/photo/src/services/face_service.rs`

- [ ] **Step 1: 更新导入**

将第 8 行：
```rust
use entities::{face_feature, face_person, DrVector};
```
改为：
```rust
use entities::{face_feature, face_person, Embedding512};
```

- [ ] **Step 2: 更新 detect_and_recognize 中的构造**

将第 89 行：
```rust
            let dr_vector = DrVector::new(norm_embedding.to_vec());
```
改为：
```rust
            let embedding = Embedding512::new(norm_embedding.to_vec());
```

将第 101 行：
```rust
                embedding: Set(dr_vector),
```
改为：
```rust
                embedding: Set(embedding),
```

- [ ] **Step 3: 更新 sync_to_db 中的构造**

将第 207 行：
```rust
                    let centroid = DrVector::new(seed.vector.clone());
```
改为：
```rust
                    let centroid = Embedding512::new(seed.vector.clone());
```

- [ ] **Step 4: 更新 merge_person 中的构造**

将第 411 行：
```rust
        let merged_embedding = DrVector::new(merged.to_vec());
```
改为：
```rust
        let merged_embedding = Embedding512::new(merged.to_vec());
```

- [ ] **Step 5: 验证编译**

Run: `cargo check --features photo`
Expected: 可能仍有其他文件报错，确认此文件无误

- [ ] **Step 6: Commit**

```bash
git add domains/photo/src/services/face_service.rs
git commit -m "refactor(photo/service): update face_service to use Embedding512"
```

---

### Task 6: 更新 Service 层 — feature_service 和 photo_service

**Files:**
- Modify: `domains/photo/src/services/feature_service.rs`
- Modify: `domains/photo/src/services/photo_service.rs`

- [ ] **Step 1: 更新 feature_service.rs 导入**

将第 8 行：
```rust
use entities::{face_feature, DrVector};
```
改为：
```rust
use entities::{face_feature, Embedding512};
```

- [ ] **Step 2: 更新 feature_service.rs 中的构造**

将第 97 行：
```rust
            Some(DrVector::new(new_centroid.to_vec())),
```
改为：
```rust
            Some(Embedding512::new(new_centroid.to_vec())),
```

将第 161 行：
```rust
        let centroid_embedding = DrVector::new(centroid.to_vec());
```
改为：
```rust
        let centroid_embedding = Embedding512::new(centroid.to_vec());
```

- [ ] **Step 3: 更新 photo_service.rs 中的类型标注**

搜索 `DrVector` 出现的位置（约第 468 行），将：
```rust
                .into_tuple::<(i64, Option<i64>, DrVector, f32)>()
```
改为：
```rust
                .into_tuple::<(i64, Option<i64>, Embedding512, f32)>()
```

- [ ] **Step 4: 验证编译**

Run: `cargo check --features photo`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add domains/photo/src/services/feature_service.rs domains/photo/src/services/photo_service.rs
git commit -m "refactor(photo/service): update feature_service and photo_service to use Embedding512"
```

---

### Task 7: 全量验证与清理

**Files:**
- 全项目扫描

- [ ] **Step 1: 确认无残留引用**

Run: `grep -rn "DrVector" --include="*.rs" .`
Expected: 无输出

- [ ] **Step 2: 全量编译检查**

Run: `cargo check`
Expected: PASS

- [ ] **Step 3: 运行测试**

Run: `cargo test`
Expected: PASS

- [ ] **Step 4: 最终 Commit（如有遗漏修复）**

```bash
git add -A
git commit -m "refactor: complete DrVector to PostgreVector/Embedding512 rename"
```

---

## 类型设计说明

### PostgreVector\<N\>

- 泛型参数 `N` 为编译时常量，表示向量维度
- 实际序列化/反序列化不校验维度（由 pgvector 和上层逻辑保证）
- 提供 `new()`, `to_vec()`, `as_slice()`, `dim()` 方法
- 实现 `Deref<[f32]>` 和 `DerefMut`，可直接当切片使用
- 实现 Sea-ORM 的 `ValueType`, `TryGetable`, `Nullable`
- 实现与 `pgvector::Vector` 的双向转换

### Embedding512

- 类型别名：`pub type Embedding512 = PostgreVector<512>`
- 语义明确：表示人脸特征嵌入向量
- 定义在 `face_feature.rs` 中，与 `FEATURE_DIM` 常量同处一处
- 未来可扩展其他维度的 Embedding 类型（如 `Embedding128`）
