use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPage<T, C> {
    pub records: Vec<T>,
    pub next_cursor: Option<C>,
    pub has_more: bool,
}

impl<T, C> CursorPage<T, C> {
    pub fn empty() -> Self {
        Self {
            records: vec![],
            next_cursor: None,
            has_more: false,
        }
    }
}

impl<T> CursorPage<T, ()> {
    pub fn from_oversize(mut records: Vec<T>, size: u64) -> Self {
        let size = size as usize;
        let has_more = records.len() > size;
        if has_more {
            records.truncate(size);
        }
        Self {
            records,
            next_cursor: None,
            has_more,
        }
    }
}
