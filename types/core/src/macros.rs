//! 强类型 ID 统一生成宏
//!
//! 所有主键 ID 类型通过 [`id_type!`] 生成,统一行为:
//! - JSON 序列化为字符串(如 `"42"`)
//! - 反序列化:i64 版接受字符串/数字双向,String 版只接受字符串
//! - 实现 `FromStr` / `Display` / `From` / `Into`
//! - `ts` feature 下导出为 TS `string` 类型
//! - `orm` feature 下 derive `sea_orm::DeriveValueType`,自动实现
//!   `TryGetable` / `sea_query::ValueType` / `Nullable` / `From<XxxId> for sea_orm::Value`,
//!   使 sea-orm 可直接查询/写入强类型 ID(`Option<XxxId>` 亦自动支持)
//!
//! 用法:
//! ```ignore
//! id_type!(VisualId, "visual/");        // i64 主键
//! id_type!(TimelineStatId, String, "visual/"); // String 主键
//! ```
//!
//! ⚠️ 展开处所在 crate 的 `orm` / `ts` feature 决定派生行为,因此该宏只在
//! 本 crate(共享内核)内使用,ID 定义集中在 [`crate::ids`]。

/// i64 主键 ID:序列化为字符串,反序列化接受字符串/数字
#[macro_export]
macro_rules! id_type {
    ($name:ident, $ts_dir:literal) => {
        #[derive(
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
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
        #[cfg_attr(feature = "orm", derive(sea_orm::DeriveValueType))]
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
        #[cfg_attr(feature = "orm", derive(sea_orm::DeriveValueType))]
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
        impl sea_orm::TryFromU64 for TimelineStatId {
            fn try_from_u64(value: u64) -> Result<Self, sea_orm::DbErr> {
                Ok(Self(value.to_string()))
            }
        }
    };
}
