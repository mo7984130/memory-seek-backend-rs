use common_core::DbConn;
use common_core::Result;

pub struct DbUtils;

impl DbUtils {
    /// 对两个 id 升序调用 `f` 加锁并返回结果，返回值顺序与传入的 id 对应
    pub async fn lock_two_ordered<'a, DB, Id, T, F, Fut>(
        db: &'a DB,
        id1: Id,
        id2: Id,
        f: F,
    ) -> Result<(Option<T>, Option<T>)>
    where
        DB: DbConn + ?Sized,
        Id: Ord + Copy,
        F: Fn(&'a DB, Id) -> Fut,
        Fut: Future<Output = Result<Option<T>>>,
    {
        let (first_id, second_id, reverse) = if id1 <= id2 {
            (id1, id2, false)
        } else {
            (id2, id1, true)
        };

        let first = f(db, first_id).await?;
        let second = f(db, second_id).await?;

        Ok(if reverse {
            (second, first)
        } else {
            (first, second)
        })
    }

    /// 对两个 id 升序调用 `f` 加锁并返回两值，返回值顺序与传入的 id 对应
    pub async fn ensure_lock_two_ordered<'a, DB, Id, T, F, Fut>(
        db: &'a DB,
        id1: Id,
        id2: Id,
        f: F,
    ) -> Result<(T, T)>
    where
        DB: DbConn + ?Sized,
        Id: Ord + Copy,
        F: Fn(&'a DB, Id) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let (first_id, second_id, reverse) = if id1 <= id2 {
            (id1, id2, false)
        } else {
            (id2, id1, true)
        };

        let first = f(db, first_id).await?;
        let second = f(db, second_id).await?;

        Ok(if reverse {
            (second, first)
        } else {
            (first, second)
        })
    }

    /// 对两个 id 升序调用 `f` 加锁，第二个 id 可选；未命中时用 `miss_f` 处理
    pub async fn lock_two_optional_ordered<'a, DB, Id, T, F, MF, Fut>(
        db: &'a DB,
        id1: Id,
        id2: Option<Id>,
        f: F,
        miss_f: MF,
    ) -> Result<(T, Option<T>)>
    where
        DB: DbConn + ?Sized,
        Id: Ord + Copy,
        F: Fn(&'a DB, Id) -> Fut,
        MF: Fn(Option<T>) -> Result<T>,
        Fut: Future<Output = Result<Option<T>>>,
    {
        match id2 {
            Some(id2) => {
                let (first_id, second_id, reverse) = if id1 <= id2 {
                    (id1, id2, false)
                } else {
                    (id2, id1, true)
                };
                let first = miss_f(f(db, first_id).await?)?;
                let second = miss_f(f(db, second_id).await?)?;
                Ok(if reverse {
                    (second, Some(first))
                } else {
                    (first, Some(second))
                })
            }
            None => Ok((miss_f(f(db, id1).await?)?, None)),
        }
    }

    /// 对两个 id 升序调用 `f` 加锁并返回两值，第二个 id 可选；返回值顺序与传入的 id 对应
    pub async fn ensure_lock_two_optional_ordered<'a, DB, Id, T, F, Fut>(
        db: &'a DB,
        id1: Id,
        id2: Option<Id>,
        f: F,
    ) -> Result<(T, Option<T>)>
    where
        DB: DbConn + ?Sized,
        Id: Ord + Copy,
        F: Fn(&'a DB, Id) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        match id2 {
            Some(id2) => {
                let (first_id, second_id, reverse) = if id1 <= id2 {
                    (id1, id2, false)
                } else {
                    (id2, id1, true)
                };
                let first = f(db, first_id).await?;
                let second = f(db, second_id).await?;
                Ok(if reverse {
                    (second, Some(first))
                } else {
                    (first, Some(second))
                })
            }
            None => Ok((f(db, id1).await?, None)),
        }
    }
}
