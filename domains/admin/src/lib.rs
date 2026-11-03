//! 管理端路由聚合；具体业务操作仍由对应业务域实现。

mod controller;
mod state;
pub mod tasks;

pub use controller::Controller;
pub use state::AdminState;
pub use tasks::{AdminJobConfig, AdminJobService, start as start_jobs};
