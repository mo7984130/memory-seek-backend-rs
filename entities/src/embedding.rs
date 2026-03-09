use sea_orm::sea_query::{Alias, ColumnType, SeaRc, Value, ValueType, ValueTypeErr};
use sea_orm::{ColIdx, QueryResult, TryGetError, TryGetable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedding(pub Vec<f32>);

impl Embedding {
    pub fn new(vec: Vec<f32>) -> Self {
        Self(vec)
    }

    pub fn to_vec(&self) -> Vec<f32> {
        self.0.clone()
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }
}

impl From<Vec<f32>> for Embedding {
    fn from(vec: Vec<f32>) -> Self {
        Self(vec)
    }
}

impl From<Embedding> for Vec<f32> {
    fn from(val: Embedding) -> Self {
        val.0
    }
}

impl TryGetable for Embedding {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let value: Option<String> = res.try_get_by(index).map_err(|e| {
            TryGetError::DbErr(sea_orm::DbErr::Type(format!("Failed to get embedding: {}", e)))
        })?;

        match value {
            Some(s) => {
                let vec: Vec<f32> = s
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .filter_map(|v| v.trim().parse::<f32>().ok())
                    .collect();
                Ok(Embedding::from(vec))
            }
            None => Err(TryGetError::Null("embedding".to_string())),
        }
    }
}

impl ValueType for Embedding {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        match v {
            Value::String(Some(s)) => {
                let vec: Vec<f32> = s
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .filter_map(|v| v.trim().parse::<f32>().ok())
                    .collect();
                Ok(Embedding::from(vec))
            }
            _ => Err(ValueTypeErr),
        }
    }

    fn type_name() -> String {
        "Embedding".to_string()
    }

    fn array_type() -> sea_orm::sea_query::ArrayType {
        sea_orm::sea_query::ArrayType::Float
    }

    fn column_type() -> ColumnType {
        ColumnType::Custom(SeaRc::new(Alias::new("vector")))
    }
}

impl From<Embedding> for Value {
    fn from(val: Embedding) -> Self {
        let s = format!(
            "[{}]",
            val.0
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        Value::String(Some(Box::new(s)))
    }
}
