//! 本地 JSON 文件存储:平台数据目录解析 + 原子读写 + 进程内缓存。
//!
//! 供桌面端等场景持久化数据与配置:一个 [`JsonFile<T>`] 对应磁盘上一个 JSON
//! 文件,`T` 为任意可序列化类型。构造时一次性读入内存,写入采用「临时文件 +
//! 原子替换」,进程内由 [`Mutex`](std::sync::Mutex) 串行化访问(跨进程文件锁
//! 不在支持范围内)。
//!
//! # 示例
//!
//! ```no_run
//! use local_store::JsonFile;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Default, Serialize, Deserialize)]
//! struct AppConfig {
//!     theme: String,
//!     window_size: (u32, u32),
//! }
//!
//! // 落到各平台用户数据目录的 <app>/config.json;文件缺失时用 Default
//! let config = JsonFile::<AppConfig>::for_app("memory-seek", "config.json")?;
//! config.update(|cfg| cfg.theme = "dark".to_owned())?;
//! # Ok::<(), local_store::StoreError>(())
//! ```

mod error;
mod json_file;

pub use error::StoreError;
pub use json_file::JsonFile;
