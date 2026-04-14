use pgvector::Vector;
use sea_orm::sea_query::{Alias, ColumnType, Nullable, SeaRc, Value, ValueType, ValueTypeErr};
use sea_orm::{ColIdx, QueryResult, TryGetError, TryGetable};
use serde::{Deserialize, Serialize};
use sqlx::Row;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrVector(pub Vec<f32>);

impl DrVector {
    pub fn new(v: Vec<f32>) -> Self {
        DrVector(v)
    }

    pub fn to_vec(&self) -> Vec<f32> {
        self.0.clone()
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }
}

impl From<Vec<f32>> for DrVector {
    fn from(vec: Vec<f32>) -> Self {
        Self(vec)
    }
}

impl From<DrVector> for Vec<f32> {
    fn from(val: DrVector) -> Self {
        val.0
    }
}

impl From<Vector> for DrVector {
    fn from(v: Vector) -> Self {
        DrVector(v.into())
    }
}

impl From<DrVector> for Vector {
    fn from(val: DrVector) -> Self {
        Vector::from(val.0)
    }
}

impl TryGetable for DrVector {
    fn try_get_by<I: ColIdx>(res: &QueryResult, index: I) -> Result<Self, TryGetError> {
        let pg_row = res.try_as_pg_row().ok_or_else(|| {
            TryGetError::DbErr(sea_orm::DbErr::Type("Not a PostgreSQL row".into()))
        })?;
        let value: Option<Vector> = pg_row
            .try_get(index.as_sqlx_postgres_index())
            .map_err(|e| TryGetError::DbErr(sea_orm::DbErr::Type(format!("Vector decode: {e}"))))?;
        value
            .map(DrVector::from)
            .ok_or_else(|| TryGetError::Null(String::new()))
    }
}

impl ValueType for DrVector {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        match v {
            Value::String(Some(s)) => {
                let data = s
                    .trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .filter_map(|v| v.trim().parse::<f32>().ok())
                    .collect();
                Ok(DrVector(data))
            }
            _ => Err(ValueTypeErr),
        }
    }

    fn type_name() -> String {
        "DrVector".into()
    }

    fn array_type() -> sea_orm::sea_query::ArrayType {
        sea_orm::sea_query::ArrayType::Float
    }

    fn column_type() -> ColumnType {
        ColumnType::Custom(SeaRc::new(Alias::new("vector")))
    }
}

impl From<DrVector> for Value {
    fn from(val: DrVector) -> Self {
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

impl Nullable for DrVector {
    fn null() -> Value {
        Value::String(None)
    }
}
