//! DTO 声明宏
//!
//! - [`validated_newtype!`]：校验型 newtype(构造时保证非空且不超过上限)
//! - [`out_dto!`]：输出 DTO(读模型 / 结果 / 响应)
//! - [`in_dto!`]：输入 DTO(参数 / 请求)
//!
//! 注意：这些宏在**本 crate 内展开**,因此其 `#[cfg_attr(feature = "ts", ...)]`
//! 按本 crate 的 feature 解析(强类型 ID 的生成宏在 `types-core`,不在此处)。

/// 校验型 newtype:构造时保证非空且不超过上限
///
/// 统一生成:derive(`Debug, Clone, Deref, Into`)、手动 `Serialize`/`Deserialize`
/// (反序列化仅做类型转换,不校验)、TS 导出、
/// `new()` / `MAX_COUNT` / `into_inner()` / `TryFrom` /
/// `Validate`(真实校验,与构造校验保持一致)。
///
/// 校验放在 `Validate` 阶段而非反序列化阶段,避免业务校验错误被 serde
/// 当作解析错误处理(会混入 `at line X column Y` 位置信息并被误判为 JSON 解析失败)。
///
/// 用法:
/// ```ignore
/// validated_newtype!(VisualIds, Vec<VisualId>, 1024, "visual/",
///     "影像ID列表不能为空", "影像数量不能超过1024");
/// validated_newtype!(CommentContent, String, 1024, "visual/",
///     "评论内容不能为空", "评论内容不能超过1024个字符");
/// ```
#[macro_export]
macro_rules! validated_newtype {
    ($name:ident, $inner:ty, $max:expr, $ts_dir:literal, $empty_msg:literal, $too_many_msg:literal) => {
        #[derive(Debug, Clone, derive_more::Deref, derive_more::Into)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir))]
        pub struct $name($inner);

        /// 序列化输出内部值（数组 / 字符串）
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                self.0.serialize(serializer)
            }
        }

        /// 反序列化仅做类型转换,校验交给 `Validate` 实现
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                <$inner>::deserialize(d).map(Self)
            }
        }

        impl $name {
            pub const MAX_COUNT: usize = $max;

            /// 构造并校验:非空 + 不超过上限
            pub fn new(v: $inner) -> Result<Self, &'static str> {
                if v.is_empty() {
                    return Err($empty_msg);
                }
                if v.len() > Self::MAX_COUNT {
                    return Err($too_many_msg);
                }
                Ok(Self(v))
            }

            /// 取出强类型包装器中的原始值.
            pub fn into_inner(self) -> $inner {
                self.0
            }
        }

        impl TryFrom<$inner> for $name {
            type Error = &'static str;

            fn try_from(v: $inner) -> Result<Self, Self::Error> {
                Self::new(v)
            }
        }

        impl validator::Validate for $name {
            fn validate(&self) -> Result<(), validator::ValidationErrors> {
                if let Err(msg) = Self::new(self.0.clone()) {
                    let mut err = validator::ValidationError::new("validated_newtype");
                    err.message = Some(std::borrow::Cow::Borrowed(msg));
                    let mut errors = validator::ValidationErrors::new();
                    errors.add("value", err);
                    return Err(errors);
                }
                Ok(())
            }
        }
    };
}

/// 输出 DTO（读模型 / 结果 / 响应）统一声明
///
/// 统一生成：`Serialize` + `Deserialize` + `Clone` + `camelCase` + TS 导出
/// （可带 `rename`、`docs` 与额外 derive）。字段（含字段级属性）原样透传。
/// 注意：`docs` 必须紧跟 `rename` 之后（或作为第一个选项）。
///
/// 用法：
/// ```ignore
/// out_dto!(VisualView, "visual/", rename = "Visual"; { ... });
/// out_dto!(CollectionVisualAddBatchResult, "visual/", Default; { ... });
/// out_dto!(UserInfo, "user/", Debug; { ... });
/// out_dto!(FaceBBox, "visual/", rename = "FaceBBox", docs = "人脸边界框", Copy; { ... });
/// ```
#[macro_export]
macro_rules! out_dto {
    // 带 TS rename + docs（读模型 View）
    ($name:ident, $ts_dir:literal, rename = $rename:literal, docs = $docs:literal $(, $extra:ident)*; { $($fields:tt)* }) => {
        #[doc = $docs]
        #[derive(serde::Serialize, serde::Deserialize, Clone $(, $extra)*)]
        #[serde(rename_all = "camelCase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir, rename = $rename))]
        pub struct $name { $($fields)* }
    };
    // 带 TS rename（读模型 View）
    ($name:ident, $ts_dir:literal, rename = $rename:literal $(, $extra:ident)*; { $($fields:tt)* }) => {
        #[derive(serde::Serialize, serde::Deserialize, Clone $(, $extra)*)]
        #[serde(rename_all = "camelCase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir, rename = $rename))]
        pub struct $name { $($fields)* }
    };
    // 不带 rename + docs（Result / Response）
    ($name:ident, $ts_dir:literal, docs = $docs:literal $(, $extra:ident)*; { $($fields:tt)* }) => {
        #[doc = $docs]
        #[derive(serde::Serialize, serde::Deserialize, Clone $(, $extra)*)]
        #[serde(rename_all = "camelCase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir))]
        pub struct $name { $($fields)* }
    };
    // 不带 rename（Result / Response）
    ($name:ident, $ts_dir:literal $(, $extra:ident)*; { $($fields:tt)* }) => {
        #[derive(serde::Serialize, serde::Deserialize, Clone $(, $extra)*)]
        #[serde(rename_all = "camelCase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir))]
        pub struct $name { $($fields)* }
    };
}

/// 输入 DTO（参数 / 请求）统一声明
///
/// 统一生成：`Debug` + `Deserialize` + `Validate` + `camelCase` + TS 导出。
/// 可选 `serialize`（额外实现 `Serialize`）、`serde_default`（`#[serde(default)]`）、
/// `docs`（struct 级文档，写在其它选项之后）。字段（含字段级属性）原样透传。
///
/// 用法：
/// ```ignore
/// in_dto!(CollectionCreateParam, "visual/"; { ... });
/// in_dto!(VisualCursorParam, "visual/", serde_default; { ... });
/// in_dto!(ChangePasswordParam, "user/", serialize; { ... });
/// in_dto!(UploadVisualParam, "visual/", serialize, docs = "上传影像参数"; { ... });
/// ```
#[macro_export]
macro_rules! in_dto {
    // serialize + serde_default
    ($name:ident, $ts_dir:literal, serialize, serde_default $(, docs = $docs:literal)?; { $($fields:tt)* }) => {
        $(#[doc = $docs])?
        #[derive(Debug, serde::Deserialize, serde::Serialize, validator::Validate)]
        #[serde(rename_all = "camelCase", default)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir))]
        pub struct $name { $($fields)* }
    };
    // serialize
    ($name:ident, $ts_dir:literal, serialize $(, docs = $docs:literal)?; { $($fields:tt)* }) => {
        $(#[doc = $docs])?
        #[derive(Debug, serde::Deserialize, serde::Serialize, validator::Validate)]
        #[serde(rename_all = "camelCase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir))]
        pub struct $name { $($fields)* }
    };
    // serde_default
    ($name:ident, $ts_dir:literal, serde_default $(, docs = $docs:literal)?; { $($fields:tt)* }) => {
        $(#[doc = $docs])?
        #[derive(Debug, serde::Deserialize, validator::Validate)]
        #[serde(rename_all = "camelCase", default)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir))]
        pub struct $name { $($fields)* }
    };
    // 基础
    ($name:ident, $ts_dir:literal $(, docs = $docs:literal)?; { $($fields:tt)* }) => {
        $(#[doc = $docs])?
        #[derive(Debug, serde::Deserialize, validator::Validate)]
        #[serde(rename_all = "camelCase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir))]
        pub struct $name { $($fields)* }
    };
}
