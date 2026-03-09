use pgvector::Vector;
use sea_orm::sea_query::{Alias, ColumnType, Value, ValueType, ValueTypeErr};
use sea_orm::{ColIdx, QueryResult, TryGetError, TryGetable};
use sea_orm::sea_query::SeaRc;
use serde::{Deserialize, Deserializer, Serialize, Serializer};


#[derive(Debug, Clone, PartialEq)]
pub struct Embedding(pub Vector);

impl Embedding {
    pub fn new(vec: Vec<f32>) -> Self {
        Self(Vector::from(vec))
    }

    pub fn to_vec(&self) -> Vec<f32> {
        self.0.clone().into()
    }

    pub fn as_slice(&self) -> &[f32] {
        self.0.as_slice()
    }
}

impl From<Vector> for Embedding {
    fn from(v: Vector) -> Self {
        Self(v)
    }
}

impl From<Embedding> for Vector {
    fn from(val: Embedding) -> Self {
        val.0
    }
}
impl From<Vec<f32>> for Embedding {
    fn from(vec: Vec<f32>) -> Self {
        Self(Vector::from(vec))
    }
}
impl From<Embedding> for Vec<f32> {
    fn from(val: Embedding) -> Self {
        val.0.into()
    }
}

impl Serialize for Embedding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // JS 不识别特殊向量格式，所以转回普通的数组
        let vec: Vec<f32> = self.0.clone().into();
        vec.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Embedding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec: Vec<f32> = Vec::deserialize(deserializer)?;
        Ok(Embedding::from(vec))
    }
}

impl TryGetable for Embedding {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let vec_data: Vec<f32> = res.try_get_by(index).map_err(|e| {
            TryGetError::DbErr(sea_orm::DbErr::Type(format!(
                "无法通过 SeaORM 转换向量。请确保数据库字段是 vector 类型: {}", e
            )))
        })?;

        Ok(Embedding(pgvector::Vector::from(vec_data)))
    }
}

impl ValueType for Embedding {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        match v {
            Value::String(Some(s)) => {
                let vec: Vec<f32> = s.trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .filter_map(|v| v.trim().parse::<f32>().ok())
                    .collect();
                Ok(Embedding::from(vec))
            }
            _ => Err(ValueTypeErr),
        }
    }

    fn type_name() -> String { "Embedding".to_string() }

    fn array_type() -> sea_orm::sea_query::ArrayType {
        sea_orm::sea_query::ArrayType::Float
    }

    fn column_type() -> ColumnType {
        ColumnType::Custom(SeaRc::new(Alias::new("vector")))
    }
}

impl From<Embedding> for Value {
    fn from(val: Embedding) -> Self {
        let s = format!("[{}]", val.0.as_slice().iter().map(|f| f.to_string()).collect::<Vec<_>>().join(","));
        Value::String(Some(Box::new(s)))
    }
}