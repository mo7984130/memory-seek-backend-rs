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
    /// 创建新的 `PostgreVector` 实例
    ///
    /// # 参数
    /// - `v`: 底层浮点数向量
    pub fn new(v: Vec<f32>) -> Self {
        PostgreVector(v)
    }

    /// 克隆并返回底层 `Vec<f32>`
    ///
    /// # 返回
    /// 向量数据的深拷贝
    pub fn to_vec(&self) -> Vec<f32> {
        self.0.clone()
    }

    /// 返回底层数据的只读切片引用
    ///
    /// # 返回
    /// 向量数据的 `&[f32]` 切片
    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }

    /// 返回向量的实际维度（元素数量）
    ///
    /// # 返回
    /// 向量中浮点数的个数
    pub fn dim(&self) -> usize {
        self.0.len()
    }
}

impl<const N: usize> From<Vec<f32>> for PostgreVector<N> {
    /// 从 `Vec<f32>` 构建 `PostgreVector`
    ///
    /// # 参数
    /// - `vec`: 浮点数向量
    fn from(vec: Vec<f32>) -> Self {
        Self(vec)
    }
}

impl<const N: usize> From<PostgreVector<N>> for Vec<f32> {
    /// 将 `PostgreVector` 转换为 `Vec<f32>`
    ///
    /// # 参数
    /// - `val`: `PostgreVector` 实例
    fn from(val: PostgreVector<N>) -> Self {
        val.0
    }
}

impl<const N: usize> From<Vector> for PostgreVector<N> {
    /// 从 pgvector 的 `Vector` 类型转换
    ///
    /// # 参数
    /// - `v`: pgvector 原生向量
    fn from(v: Vector) -> Self {
        PostgreVector(v.into())
    }
}

impl<const N: usize> From<PostgreVector<N>> for Vector {
    /// 将 `PostgreVector` 转换为 pgvector 的 `Vector` 类型
    ///
    /// # 参数
    /// - `val`: `PostgreVector` 实例
    fn from(val: PostgreVector<N>) -> Self {
        Vector::from(val.0)
    }
}

/// 从 pgvector 二进制格式解析向量数据
///
/// 二进制格式：
/// - 2 字节：维度（big-endian u16）
/// - 2 字节：保留字段（unused）
/// - 后续每 4 字节为一个 f32 元素（big-endian）
fn parse_vector_bytes(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if bytes.len() < 4 {
        return Err("vector data too short".into());
    }
    let dim = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
    let expected = 4 + dim * 4;
    if bytes.len() < expected {
        return Err(format!(
            "vector data truncated: expected {expected} bytes, got {}",
            bytes.len()
        ));
    }
    let data = bytes[4..expected]
        .chunks_exact(4)
        .map(|chunk| f32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    Ok(data)
}

impl<const N: usize> TryGetable for PostgreVector<N> {
    /// 从 PostgreSQL 查询结果中提取向量列
    ///
    /// 手动解析 pgvector 二进制格式，避免 pgvector crate 的 sqlx 版本
    /// 与项目使用的 sqlx 版本不一致导致的 `Decode` trait 冲突。
    ///
    /// # 参数
    /// - `res`: Sea-ORM 查询结果
    /// - `index`: 列索引
    ///
    /// # 返回
    /// 解码后的 `PostgreVector` 实例
    ///
    /// # 错误
    /// - `TryGetError`: 行不是 PostgreSQL 类型或向量解码失败
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let pg_row = res.try_as_pg_row().ok_or_else(|| {
            TryGetError::DbErr(sea_orm::DbErr::Type("Not a PostgreSQL row".into()))
        })?;
        let value: Option<Vec<u8>> = pg_row
            .try_get(index.as_sqlx_postgres_index())
            .map_err(|e| TryGetError::DbErr(sea_orm::DbErr::Type(format!("Vector decode: {e}"))))?;
        match value {
            Some(bytes) => {
                let vec = parse_vector_bytes(&bytes)
                    .map_err(|e| TryGetError::DbErr(sea_orm::DbErr::Type(format!("Vector parse: {e}"))))?;
                Ok(PostgreVector::new(vec))
            }
            None => Err(TryGetError::Null(String::new())),
        }
    }
}

impl<const N: usize> ValueType for PostgreVector<N> {
    /// 从 Sea-ORM `Value` 尝试转换为 `PostgreVector`
    ///
    /// 支持解析格式为 `[1.0,2.0,3.0]` 的字符串值。
    ///
    /// # 参数
    /// - `v`: Sea-ORM 值
    ///
    /// # 返回
    /// 解析成功的 `PostgreVector` 实例
    ///
    /// # 错误
    /// - `ValueTypeErr`: 输入不是字符串类型或格式不合法
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

    /// 返回类型名称，格式为 `PostgreVector<N>`
    ///
    /// # 返回
    /// 包含泛型维度参数的类型名字符串
    fn type_name() -> String {
        format!("PostgreVector<{}>", N)
    }

    /// 返回数组元素类型为 `Float`
    ///
    /// # 返回
    /// `ArrayType::Float`
    fn array_type() -> sea_orm::sea_query::ArrayType {
        sea_orm::sea_query::ArrayType::Float
    }

    /// 返回 PostgreSQL `vector` 自定义列类型
    ///
    /// # 返回
    /// `ColumnType::Custom("vector")`
    fn column_type() -> ColumnType {
        ColumnType::Custom(SeaRc::new(Alias::new("vector")))
    }
}

impl<const N: usize> From<PostgreVector<N>> for Value {
    /// 将 `PostgreVector` 序列化为 Sea-ORM `Value::String`
    ///
    /// 输出格式为 `[1.0,2.0,3.0]`，整数元素保留一位小数。
    ///
    /// # 参数
    /// - `val`: `PostgreVector` 实例
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
    /// 返回表示 NULL 的 `Value::String(None)`
    ///
    /// # 返回
    /// 空值
    fn null() -> Value {
        Value::String(None)
    }
}

impl<const N: usize> Deref for PostgreVector<N> {
    type Target = [f32];

    /// 解引用为底层 `&[f32]` 切片
    ///
    /// # 返回
    /// 向量数据的只读切片引用
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> DerefMut for PostgreVector<N> {
    /// 可变解引用为底层 `&mut [f32]` 切片
    ///
    /// # 返回
    /// 向量数据的可变切片引用
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
