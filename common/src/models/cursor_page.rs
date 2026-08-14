use serde::Serialize;

use crate::Result;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "common/"))]
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

    pub fn replace_records<U>(self, records: Vec<U>) -> CursorPage<U, C> {
        CursorPage {
            records,
            next_cursor: self.next_cursor,
            has_more: self.has_more,
        }
    }

    pub fn map_records<U, F>(self, map_records: F) -> CursorPage<U, C>
    where
        F: FnOnce(Vec<T>) -> Vec<U>,
    {
        let CursorPage {
            records,
            next_cursor,
            has_more,
        } = self;
        CursorPage {
            records: map_records(records),
            next_cursor,
            has_more,
        }
    }

    pub fn with_next_cursor<C2, F>(self, get_cursor: F) -> Result<CursorPage<T, C2>>
    where
        F: FnOnce(&T) -> Result<C2>,
    {
        let next_cursor = if self.has_more {
            self.records.last().map(get_cursor).transpose()?
        } else {
            None
        };
        Ok(CursorPage {
            records: self.records,
            next_cursor,
            has_more: self.has_more,
        })
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

#[macro_export]
macro_rules! current_fn_name {
    () => {{
        const fn f() {}
        fn type_name_of_val<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of_val(f);
        name
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_returns_empty_list_with_has_more_false() {
        let page: CursorPage<String, i32> = CursorPage::empty();
        assert!(page.records.is_empty());
        assert!(page.next_cursor.is_none());
        assert!(!page.has_more);
        print!("\n curent_fn_name: \n {} \n", current_fn_name!())
    }

    #[test]
    fn test_from_oversize_items_exactly_at_limit() {
        let items = vec![1, 2, 3];
        let page = CursorPage::from_oversize(items, 3);
        assert_eq!(page.records.len(), 3);
        assert_eq!(page.records, vec![1, 2, 3]);
        assert!(!page.has_more);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn test_from_oversize_items_over_limit() {
        let items = vec![1, 2, 3, 4, 5];
        let page = CursorPage::from_oversize(items, 3);
        assert_eq!(page.records.len(), 3);
        assert_eq!(page.records, vec![1, 2, 3]);
        assert!(page.has_more);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn test_from_oversize_empty_items() {
        let items: Vec<i32> = vec![];
        let page = CursorPage::from_oversize(items, 5);
        assert!(page.records.is_empty());
        assert!(!page.has_more);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn replace_records_preserves_pagination_metadata() {
        let page = CursorPage::from_oversize(vec![1, 2, 3], 2)
            .with_next_cursor(|last| Ok(last.to_string()))
            .unwrap();
        let page = page.replace_records(vec!["one", "two"]);

        assert_eq!(page.records, vec!["one", "two"]);
        assert_eq!(page.next_cursor.as_deref(), Some("2"));
        assert!(page.has_more);
    }

    #[test]
    fn with_next_cursor_skips_callback_without_more_records() {
        let page = CursorPage::from_oversize(vec![1, 2], 2)
            .with_next_cursor(|_| -> Result<String> { panic!("must not generate cursor") })
            .unwrap();

        assert!(page.next_cursor.is_none());
        assert!(!page.has_more);
    }

    #[test]
    fn map_records_preserves_pagination_metadata() {
        let page = CursorPage::from_oversize(vec![1, 2, 3], 2)
            .with_next_cursor(|last| Ok(last.to_string()))
            .unwrap()
            .map_records(|records| records.into_iter().map(|id| id * 10).collect());

        assert_eq!(page.records, vec![10, 20]);
        assert_eq!(page.next_cursor.as_deref(), Some("2"));
        assert!(page.has_more);
    }
}
