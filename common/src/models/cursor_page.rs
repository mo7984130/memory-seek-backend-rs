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
}
