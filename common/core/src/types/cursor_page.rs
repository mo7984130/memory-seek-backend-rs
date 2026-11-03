use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPage<T, C> {
    pub records: Vec<T>,
    pub next_cursor: Option<C>,
    pub has_more: bool,
}

impl<T, C> CursorPage<T, C> {
    /// 创建空分页结果；`has_more` 为 false，`next_cursor` 为 `None`。
    #[inline]
    pub fn empty() -> Self {
        Self {
            records: vec![],
            next_cursor: None,
            has_more: false,
        }
    }

    /// 替换分页记录，保留游标与分页状态
    #[inline]
    pub fn replace_records<U>(self, records: Vec<U>) -> CursorPage<U, C> {
        CursorPage {
            records,
            next_cursor: self.next_cursor,
            has_more: self.has_more,
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
        } = self;
        CursorPage {
            records: map_records(records),
            next_cursor,
            has_more,
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
        }
    }

    /// 附加记录总数，供确实需要总数的接口使用。
    #[inline]
    pub fn with_total(self, total_count: u64) -> CursorPageWithTotal<T, C> {
        CursorPageWithTotal {
            page: self,
            total_count: Some(total_count),
        }
    }

    /// 包装为带总数的分页；`total_count` 为 `None` 表示本页不携带总数。
    #[inline]
    pub fn with_optional_total(self, total_count: Option<u64>) -> CursorPageWithTotal<T, C> {
        CursorPageWithTotal {
            page: self,
            total_count,
        }
    }
}

impl<T> CursorPage<T, ()> {
    /// 从多取一条的查询结果构造分页并截断超出页大小的记录
    #[inline]
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

    /// 从记录构造分页；`has_more` 为 false，`next_cursor` 为 `None`
    #[inline]
    pub fn from_records(records: Vec<T>) -> Self {
        Self {
            records,
            next_cursor: None,
            has_more: false,
        }
    }

    /// 从记录与是否还有下一页标志构造分页；`next_cursor` 为 `None`
    #[inline]
    pub fn from_has_more(records: Vec<T>, has_more: bool) -> Self {
        Self {
            records,
            next_cursor: None,
            has_more,
        }
    }
}

/// 带记录总数的游标分页。
///
/// `total_count` 只在首屏（无游标）时计算；后续页为 `None` 并在序列化时省略。
#[derive(Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPageWithTotal<T, C> {
    pub page: CursorPage<T, C>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_count: Option<u64>,
}

impl<T, C> CursorPageWithTotal<T, C> {
    /// 从分页与总数构造结果
    #[inline]
    pub fn new(page: CursorPage<T, C>, total_count: Option<u64>) -> Self {
        Self { page, total_count }
    }

    /// 创建空分页结果，保留本页是否携带总数
    #[inline]
    pub fn empty(total_count: Option<u64>) -> Self {
        Self {
            page: CursorPage::empty(),
            total_count,
        }
    }

    /// 替换分页记录，保留游标、分页状态与总数
    #[inline]
    pub fn replace_records<U>(self, records: Vec<U>) -> CursorPageWithTotal<U, C> {
        CursorPageWithTotal {
            page: self.page.replace_records(records),
            total_count: self.total_count,
        }
    }

    /// 映射分页记录，保留游标、分页状态与总数
    #[inline]
    pub fn map_records<U, F>(self, map_records: F) -> CursorPageWithTotal<U, C>
    where
        F: FnOnce(Vec<T>) -> Vec<U>,
    {
        CursorPageWithTotal {
            page: self.page.map_records(map_records),
            total_count: self.total_count,
        }
    }

    /// 仅在存在下一页时用最后一条记录生成下一页游标
    #[inline]
    pub fn with_next_cursor<C2, F>(self, get_cursor: F) -> CursorPageWithTotal<T, C2>
    where
        F: FnOnce(&T) -> C2,
    {
        CursorPageWithTotal {
            page: self.page.with_next_cursor(get_cursor),
            total_count: self.total_count,
        }
    }
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
    }

    #[test]
    fn serializes_page_with_total_in_camel_case() {
        let page = CursorPage::from_has_more(vec![1, 2], true).with_total(8);
        let value = serde_json::to_value(page).unwrap();

        assert_eq!(value["page"]["hasMore"], true);
        assert_eq!(value["totalCount"], 8);
        assert!(value["page"].get("totalCount").is_none());
    }

    #[test]
    fn omits_total_count_when_absent() {
        let page = CursorPage::from_has_more(vec![1], true).with_optional_total(None);
        let value = serde_json::to_value(page).unwrap();

        assert!(value.get("totalCount").is_none());
        assert_eq!(value["page"]["hasMore"], true);
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
            .with_next_cursor(|last| last.to_string())
            .with_total(3);
        let page = page.replace_records(vec!["one", "two"]);

        assert_eq!(page.page.records, vec!["one", "two"]);
        assert_eq!(page.page.next_cursor.as_deref(), Some("2"));
        assert!(page.page.has_more);
        assert_eq!(page.total_count, Some(3));
    }

    #[test]
    fn with_next_cursor_skips_callback_without_more_records() {
        let page = CursorPage::from_oversize(vec![1, 2], 2)
            .with_next_cursor(|_| -> String { panic!("must not generate cursor") });

        assert!(page.next_cursor.is_none());
        assert!(!page.has_more);
    }

    #[test]
    fn map_records_preserves_pagination_metadata() {
        let page = CursorPage::from_oversize(vec![1, 2, 3], 2)
            .with_next_cursor(|last| last.to_string())
            .with_total(3)
            .map_records(|records| records.into_iter().map(|id| id * 10).collect());

        assert_eq!(page.page.records, vec![10, 20]);
        assert_eq!(page.page.next_cursor.as_deref(), Some("2"));
        assert!(page.page.has_more);
        assert_eq!(page.total_count, Some(3));
    }
}
