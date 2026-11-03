//! 装箱 future 的公共类型别名。

use std::future::Future;
use std::pin::Pin;

/// 装箱且 `Send` 的 future。
///
/// 用于在跨 crate 的异步边界切断 coroutine 布局递归: 若调用方直接内联 `await`
/// 一条很深的 async 调用链, rustc 计算外层状态机布局时会沿链递归展开子 future,
/// 深度随调用层级累加, 最终可能超过编译器的查询深度上限(报
/// `queries overflow the depth limit`)。装箱后调用方只持有一个指针, 布局深度
/// 不再随被调用方内部层级增长。
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
