//! 强类型 ID 统一生成宏
//!
//! 所有主键 ID 类型通过 [`id_type!`] 生成,统一行为:
//! - JSON 序列化为字符串(如 `"42"`)
//! - 反序列化:i64 版接受字符串/数字双向,String 版只接受字符串
//! - 实现 `FromStr` / `Display` / `From` / `Into`
//! - `ts` feature 下导出为 TS `string` 类型
//! - `orm` feature 下实现 `From<XxxId> for sea_orm::Value`
//!
//! 用法:
//! ```ignore
//! id_type!(PhotoId, "photo/");        // i64 主键
//! id_type!(TimelineStatId, String, "photo/"); // String 主键
//! ```

/// i64 主键 ID:序列化为字符串,反序列化接受字符串/数字
#[macro_export]
macro_rules! id_type {
    ($name:ident, $ts_dir:literal) => {
        #[derive(
            PartialEq,
            Eq,
            Hash,
            Copy,
            Clone,
            Debug,
            derive_more::Display,
            derive_more::From,
            derive_more::Into,
        )]
        #[display("{}", _0)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir))]
        pub struct $name(pub i64);

        /// 序列化为字符串（如 "42"），而非数字
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0.to_string())
            }
        }

        /// 反序列化时同时接受字符串 ("42") 和数字 (42)
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct Visitor;

                impl<'de> serde::de::Visitor<'de> for Visitor {
                    type Value = $name;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str(concat!("a ", stringify!($name), " as a number or string"))
                    }

                    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<$name, E> {
                        Ok($name(v))
                    }

                    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<$name, E> {
                        Ok($name(v as i64))
                    }

                    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<$name, E> {
                        v.parse::<i64>()
                            .map($name)
                            .map_err(|_| E::custom(concat!("invalid ", stringify!($name))))
                    }
                }

                d.deserialize_any(Visitor)
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::error::ParseIdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                s.parse::<i64>()
                    .map($name)
                    .map_err(|_| $crate::error::ParseIdError(concat!("无效 ", stringify!($name))))
            }
        }

        #[cfg(feature = "orm")]
        impl From<$name> for sea_orm::Value {
            fn from(val: $name) -> Self {
                sea_orm::Value::BigInt(Some(val.0))
            }
        }
    };
    // String 主键 ID:序列化为字符串,反序列化只接受字符串
    ($name:ident, String, $ts_dir:literal) => {
        #[derive(
            PartialEq,
            Eq,
            Hash,
            Clone,
            Debug,
            derive_more::Display,
            derive_more::From,
            derive_more::Into,
        )]
        #[display("{}", _0)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $ts_dir))]
        pub struct $name(pub String);

        /// 序列化为字符串
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        /// 反序列化只接受字符串
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct Visitor;

                impl<'de> serde::de::Visitor<'de> for Visitor {
                    type Value = $name;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str(concat!("a ", stringify!($name), " as a string"))
                    }

                    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<$name, E> {
                        Ok($name(v.to_string()))
                    }

                    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<$name, E> {
                        Ok($name(v))
                    }
                }

                d.deserialize_any(Visitor)
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::error::ParseIdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok($name(s.to_string()))
            }
        }

        #[cfg(feature = "orm")]
        impl From<$name> for sea_orm::Value {
            fn from(val: $name) -> Self {
                sea_orm::Value::String(Some(Box::new(val.0)))
            }
        }
    };
}

/// 校验型 newtype:构造时保证非空且不超过上限
///
/// 统一生成:derive(`Debug, Clone, Deref, Into`)、手动 `Serialize`/`Deserialize`
/// (反序列化走构造校验,等价于 `#[serde(try_from)]`)、TS 导出、
/// `new()` / `MAX_COUNT` / `into_inner()` / `TryFrom` / `Validate`(no-op)。
///
/// 用法:
/// ```ignore
/// validated_newtype!(PhotoIds, Vec<PhotoId>, 1024, "photo/",
///     "照片ID列表不能为空", "照片数量不能超过1024");
/// validated_newtype!(CommentContent, String, 1024, "photo/",
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

        /// 反序列化内部值并走构造校验
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = <$inner>::deserialize(d)?;
                Self::new(v).map_err(serde::de::Error::custom)
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
                // 构造时（反序列化时）已校验，此处为 no-op
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
/// out_dto!(PhotoView, "photo/", rename = "Photo"; { ... });
/// out_dto!(CollectionPhotoAddBatchResult, "photo/", Default; { ... });
/// out_dto!(UserInfo, "user/", Debug; { ... });
/// out_dto!(FaceBBox, "photo/", rename = "FaceBBox", docs = "人脸边界框", Copy; { ... });
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
/// in_dto!(CollectionCreateParam, "photo/"; { ... });
/// in_dto!(PhotoCursorParam, "photo/", serde_default; { ... });
/// in_dto!(ChangePasswordParam, "user/", serialize; { ... });
/// in_dto!(UploadPhotoParam, "photo/", serialize, docs = "上传照片参数"; { ... });
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
