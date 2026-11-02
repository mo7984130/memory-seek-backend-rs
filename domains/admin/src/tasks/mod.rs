//! 后台任务编排：持久化队列、多实例领取、心跳续约与取消传导。
//!
//! 模块边界：
//! - `repo`：唯一允许写 SQL 的地方；
//! - `service`：HTTP 用例、事务与审计；
//! - `dispatcher`：任务类型到业务域执行器的映射；
//! - `runner`：单个任务的执行生命周期；
//! - `supervisor`：每实例常驻的领取与回收循环。

mod config;
mod dispatcher;
mod repo;
mod runner;
mod service;
mod supervisor;

pub use config::AdminJobConfig;
pub use service::AdminJobService;
pub use supervisor::start;
