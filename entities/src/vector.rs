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
