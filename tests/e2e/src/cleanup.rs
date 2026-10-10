//! 收尾(teardown)复用的删除动作。
//!
//! 语义:
//! - 幂等: 资源已不存在(404/410)视为收尾完成, 不算失败(本轮可能根本没创建成功);
//! - 其余非 200 视为收尾失败并上报(框架单列 `teardown_failures`, 不影响通过率);
//! - 只删本次轮次/任务自己创建的产物, 绝不触碰种子数据。

use memory_seek_api::ApiError;

use crate::error::E2eError;

/// 幂等收尾: 把类型化删除动作的结果归一——资源已不存在(404/410)视为收尾完成,
/// 其余错误上报(框架单列 `teardown_failures`, 不影响通过率)。
pub fn idempotent<T>(result: Result<T, ApiError>) -> Result<(), E2eError> {
    match result {
        Ok(_) => Ok(()),
        Err(ApiError::HttpStatus {
            code: 404 | 410, ..
        }) => Ok(()),
        Err(error) => Err(E2eError::Api(error)),
    }
}
