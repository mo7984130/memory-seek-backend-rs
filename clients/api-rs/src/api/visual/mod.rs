//! 影像模块:影像、收藏夹、评论、时间线、点赞、人脸、人物。
//!
//! 与 TS 客户端一致,文件自身的函数(影像)在模块根再导出,子分组以子模块访问:
//! `visual::get_visuals(...)` / `visual::collection::get_collection_list(...)`。
#![allow(clippy::module_inception)]

pub mod collection;
pub mod comment;
pub mod face;
pub mod like;
pub mod person;
pub mod timeline;
pub mod visual;

pub use visual::*;
