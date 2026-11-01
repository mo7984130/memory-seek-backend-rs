use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPage<T, C> {
    pub records: Vec<T>,
    pub next_cursor: Option<C>,
    pub has_more: bool,
    /// 匹配完整查询范围的记录总数，不受分页游标和页大小影响。
    pub total_count: u64,
}

impl<T, C> CursorPage<T, C> {
    /// 创建空分页结果；`has_more` 为 false，`next_cursor` 为 `None`。
    /// `total_count` 是完整查询范围的总记录数，不受游标影响。
    #[inline]
    pub fn empty(total_count: u64) -> Self {
        Self {
            records: vec![],
            next_cursor: None,
            has_more: false,
            total_count,
        }
    }

    /// 替换分页记录，保留游标与分页状态
    #[inline]
    pub fn replace_records<U>(self, records: Vec<U>) -> CursorPage<U, C> {
        CursorPage {
            records,
            next_cursor: self.next_cursor,
            has_more: self.has_more,
            total_count: self.total_count,
        }
    }

    /// 映射分页记录，保留游标与分页状态
    #[inline]
    pub fn map_records<U, F>(self, map_records: F) -> CursorPage<U, C>
    where
        F: FnOnce(Vec<T>) -> Vec<U>,
    {
        let CursorPage {
            records,
            next_cursor,
            has_more,
            total_count,
        } = self;
        CursorPage {
            records: map_records(records),
            next_cursor,
            has_more,
            total_count,
        }
    }

    /// 仅在存在下一页时用最后一条记录生成下一页游标
    #[inline]
    pub fn with_next_cursor<C2, F>(self, get_cursor: F) -> CursorPage<T, C2>
    where
        F: FnOnce(&T) -> C2,
    {
        let next_cursor = if self.has_more {
            self.records.last().map(get_cursor)
        } else {
            None
        };
        CursorPage {
            records: self.records,
            next_cursor,
            has_more: self.has_more,
            total_count: self.total_count,
        }
    }
}

impl<T> CursorPage<T, ()> {
    /// 从多取一条的查询结果构造分页并截断超出页大小的记录
    #[inline]
    pub fn from_oversize(mut records: Vec<T>, size: u64, total_count: u64) -> Self {
        let size = size as usize;
        let has_more = records.len() > size;
        if has_more {
            records.truncate(size);
        }
        Self {
            records,
            next_cursor: None,
            has_more,
            total_count,
        }
    }

    /// 从记录构造分页；`has_more` 为 false，`next_cursor` 为 `None`
    #[inline]
    pub fn from_records(records: Vec<T>) -> Self {
        let total_count = records.len() as u64;
        Self {
            records,
            next_cursor: None,
            has_more: false,
            total_count,
        }
    }

    /// 从记录与是否还有下一页标志构造分页；`next_cursor` 为 `None`
    #[inline]
    pub fn from_has_more(records: Vec<T>, has_more: bool, total_count: u64) -> Self {
        Self {
            records,
            next_cursor: None,
            has_more,
            total_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_returns_empty_list_with_has_more_false() {
        let page: CursorPage<String, i32> = CursorPage::empty(0);
        assert!(page.records.is_empty());
        assert!(page.next_cursor.is_none());
        assert!(!page.has_more);
        assert_eq!(page.total_count, 0);
    }

    #[test]
    fn serializes_total_count_in_camel_case() {
        let page = CursorPage::from_has_more(vec![1, 2], true, 8);
        let value = serde_json::to_value(page).unwrap();

        assert_eq!(value["totalCount"], 8);
    }

    #[test]
    fn test_from_oversize_items_exactly_at_limit() {
        let items = vec![1, 2, 3];
        let page = CursorPage::from_oversize(items, 3, 3);
        assert_eq!(page.records.len(), 3);
        assert_eq!(page.records, vec![1, 2, 3]);
        assert!(!page.has_more);
        assert_eq!(page.total_count, 3);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn test_from_oversize_items_over_limit() {
        let items = vec![1, 2, 3, 4, 5];
        let page = CursorPage::from_oversize(items, 3, 5);
        assert_eq!(page.records.len(), 3);
        assert_eq!(page.records, vec![1, 2, 3]);
        assert!(page.has_more);
        assert_eq!(page.total_count, 5);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn test_from_oversize_empty_items() {
        let items: Vec<i32> = vec![];
        let page = CursorPage::from_oversize(items, 5, 0);
        assert!(page.records.is_empty());
        assert!(!page.has_more);
        assert_eq!(page.total_count, 0);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn replace_records_preserves_pagination_metadata() {
        let page = CursorPage::from_oversize(vec![1, 2, 3], 2, 3)
            .with_next_cursor(|last| last.to_string());
        let page = page.replace_records(vec!["one", "two"]);

        assert_eq!(page.records, vec!["one", "two"]);
        assert_eq!(page.next_cursor.as_deref(), Some("2"));
        assert!(page.has_more);
        assert_eq!(page.total_count, 3);
    }

    #[test]
    fn with_next_cursor_skips_callback_without_more_records() {
        let page = CursorPage::from_oversize(vec![1, 2], 2, 2)
            .with_next_cursor(|_| -> String { panic!("must not generate cursor") });

        assert!(page.next_cursor.is_none());
        assert!(!page.has_more);
    }

    #[test]
    fn map_records_preserves_pagination_metadata() {
        let page = CursorPage::from_oversize(vec![1, 2, 3], 2, 3)
            .with_next_cursor(|last| last.to_string())
            .map_records(|records| records.into_iter().map(|id| id * 10).collect());

        assert_eq!(page.records, vec![10, 20]);
        assert_eq!(page.next_cursor.as_deref(), Some("2"));
        assert!(page.has_more);
        assert_eq!(page.total_count, 3);
    }
}
