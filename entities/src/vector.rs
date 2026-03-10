use sea_orm::sea_query::{Value, ValueType, ValueTypeErr, ColumnType, Alias, SeaRc, Nullable};
use sea_orm::{QueryResult, TryGetable, TryGetError, ColIdx, DbErr};
use serde::{Serialize, Deserialize};
use std::convert::TryInto;

/// 向量包装类，直接对接 PostgreSQL 的 vector 扩展
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrVector(pub Vec<f32>);

impl DrVector {
    pub fn new(v: Vec<f32>) -> Self {
        DrVector(v)
    }

    /// 将 PostgreSQL 的二进制 vector 格式解析为 Vector 结构体
    /// Postgres vector 协议：前2字节维度(dim)，后2字节保留(unused)，之后每4字节一个大端序 f32
    pub fn from_sql(buf: &[u8]) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        if buf.len() < 4 {
            return Err("Data too short for vector header".into());
        }
        let dim = u16::from_be_bytes(buf[0..2].try_into()?).into();
        let mut vec = Vec::with_capacity(dim);
        for i in 0..dim {
            let start = 4 + 4 * i;
            if start + 4 <= buf.len() {
                vec.push(f32::from_be_bytes(buf[start..start + 4].try_into()?));
            }
        }
        Ok(DrVector(vec))
    }
}

// --- SQLx 适配层 ---
impl sqlx::Type<sqlx::Postgres> for DrVector {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("vector")
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for DrVector {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use sqlx::Decode;
        // 尝试作为字节流解码
        let buf = <&[u8] as Decode<sqlx::Postgres>>::decode(value)?;
        Self::from_sql(buf)
    }
}

// --- SeaORM 适配层 ---
impl TryGetable for DrVector {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        // 首先尝试直接从 QueryResult 获取，这会触发 sqlx::Decode
        let raw_res: Result<Self, DbErr> = res.try_get_by(index);
        if let Ok(v) = raw_res {
            return Ok(v);
        }

        // 降级方案：处理可能的字符串返回格式
        let val: Value = res.try_get_by(index).map_err(TryGetError::DbErr)?;
        match val {
            Value::String(Some(s)) => {
                let data = s.trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .filter_map(|v| v.trim().parse::<f32>().ok())
                    .collect();
                Ok(DrVector(data))
            },
            _ => Err(TryGetError::Null),
        }
    }
}

impl ValueType for DrVector {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        match v {
            Value::String(Some(s)) => {
                let data = s.trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .filter_map(|v| v.trim().parse::<f32>().ok())
                    .collect();
                Ok(DrVector(data))
            }
            _ => Err(ValueTypeErr),
        }
    }

    fn type_name() -> String { "DrVector".into() }

    fn array_type() -> sea_orm::sea_query::ArrayType {
        sea_orm::sea_query::ArrayType::Float
    }

    fn column_type() -> ColumnType {
        ColumnType::Custom(SeaRc::new(Alias::new("vector")))
    }
}

impl From<DrVector> for Value {
    fn from(val: DrVector) -> Self {
        // 写入时转为 "[1.0, 2.0]" 文本格式，Postgres 会自动识别
        let s = format!("[{}]", val.0.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(","));
        Value::String(Some(Box::new(s)))
    }
}

impl Nullable for DrVector {
    fn null() -> Value {
        Value::String(None)
    }
}