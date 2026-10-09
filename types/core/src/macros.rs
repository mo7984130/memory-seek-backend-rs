//! 共享内核的声明宏
//!
//! - [`id_type!`]：强类型主键 ID
//! - [`validated_newtype!`]：校验型 newtype(非空 + 上限)
//! - [`dto!`]：DTO(请求 / 响应)统一声明
//!
//! ⚠️ 这些宏体内的 `#[cfg_attr(feature = "orm" / "ts", ...)]` 按**展开处所在
//! crate** 的 feature 解析,展开处还需能解析宏内引用的路径:
//! - `serde` / `validator` / `ts-rs`(DTO 宏)
//! - `derive_more`(`validated_newtype!`)
//!
//! 因此:
//!
//! - `id_type!` 只在**本 crate** 内使用,ID 定义集中在 [`crate::ids`]
//!   (避免实体侧与 DTO 侧派生不一致);
//! - DTO 宏由各域契约 crate 展开,那些 crate 需自行声明 `ts` feature
//!   并依赖 `serde` / `validator` / `ts-rs`。
//!
//! ## id_type!
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
            type Err = $crate::ids::ParseIdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                s.parse::<i64>()
                    .map($name)
                    .map_err(|_| $crate::ids::ParseIdError(concat!("无效 ", stringify!($name))))
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
            type Err = $crate::ids::ParseIdError;

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

// ============================================================
// DTO 声明宏
// ============================================================

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

/// DTO 声明宏:请求 / 响应统一声明
///
/// 统一生成完整能力集:`Debug` + `Clone` + `Serialize` + `Deserialize` +
/// `Validate`,外加 `camelCase` 与 TS 导出。数据在线上双向流动(服务端反序列化
/// 请求 / 序列化响应,客户端序列化请求 / 反序列化响应),故不再区分
/// 「输入 / 输出」方向:一律派生双向能力,杜绝某一方向遗漏。
///
/// 字段(含字段级属性)原样透传。
///
/// 选项(逗号分隔、顺序任意、均可省略):
/// - `rename = "X"`:TS 导出重命名(仅 `ts` feature 生效)
/// - `serde_default`:追加容器级 `#[serde(default)]`
/// - `docs = "..."`:结构体级文档注释
/// - 额外 derive 标识符(如 `Default`):追加到 derive 列表末尾
///
/// 用法:
/// ```ignore
/// dto!(VisualView, "visual/", rename = "Visual"; { pub id: VisualId });
/// dto!(CollectionVisualAddBatchResult, "visual/", Default; { pub new_visual_count: u64 });
/// dto!(ChangePasswordParam, "user/", docs = "修改密码参数"; { pub old: String });
/// dto!(VisualCursorParam, "visual/", serde_default; { pub size: u64 });
/// ```
#[macro_export]
macro_rules! dto {
    // ===== 入口:含选项 =====
    ($name:ident, $ts_dir:literal, $($rest:tt)+) => {
        $crate::dto! { @acc $name, $ts_dir, [rename_all = "camelCase"], [], [], [] ; $($rest)+ }
    };
    // ===== 入口:无选项 =====
    ($name:ident, $ts_dir:literal; { $($fields:tt)* }) => {
        $crate::dto! { @acc $name, $ts_dir, [rename_all = "camelCase"], [], [], [] ; ; { $($fields)* } }
    };

    // ===== 累积:rename =====
    (@acc $name:ident, $dir:literal, [$($s:tt)*], [$($t:tt)*], [$($d:tt)*], [$($e:tt)*] ; rename = $rename:literal, $($rest:tt)*) => {
        $crate::dto! { @acc $name, $dir, [$($s)*], [$($t)*, rename = $rename], [$($d)*], [$($e)*] ; $($rest)* }
    };
    (@acc $name:ident, $dir:literal, [$($s:tt)*], [$($t:tt)*], [$($d:tt)*], [$($e:tt)*] ; rename = $rename:literal ; $($rest:tt)*) => {
        $crate::dto! { @acc $name, $dir, [$($s)*], [$($t)*, rename = $rename], [$($d)*], [$($e)*] ; ; $($rest)* }
    };

    // ===== 累积:serde_default =====
    (@acc $name:ident, $dir:literal, [$($s:tt)*], [$($t:tt)*], [$($d:tt)*], [$($e:tt)*] ; serde_default, $($rest:tt)*) => {
        $crate::dto! { @acc $name, $dir, [$($s)*, default], [$($t)*], [$($d)*], [$($e)*] ; $($rest)* }
    };
    (@acc $name:ident, $dir:literal, [$($s:tt)*], [$($t:tt)*], [$($d:tt)*], [$($e:tt)*] ; serde_default ; $($rest:tt)*) => {
        $crate::dto! { @acc $name, $dir, [$($s)*, default], [$($t)*], [$($d)*], [$($e)*] ; ; $($rest)* }
    };

    // ===== 累积:docs =====
    (@acc $name:ident, $dir:literal, [$($s:tt)*], [$($t:tt)*], [$($d:tt)*], [$($e:tt)*] ; docs = $doc:literal, $($rest:tt)*) => {
        $crate::dto! { @acc $name, $dir, [$($s)*], [$($t)*], [#[doc = $doc]], [$($e)*] ; $($rest)* }
    };
    (@acc $name:ident, $dir:literal, [$($s:tt)*], [$($t:tt)*], [$($d:tt)*], [$($e:tt)*] ; docs = $doc:literal ; $($rest:tt)*) => {
        $crate::dto! { @acc $name, $dir, [$($s)*], [$($t)*], [#[doc = $doc]], [$($e)*] ; ; $($rest)* }
    };

    // ===== 累积:额外 derive =====
    (@acc $name:ident, $dir:literal, [$($s:tt)*], [$($t:tt)*], [$($d:tt)*], [$($e:tt)*] ; $extra:ident, $($rest:tt)*) => {
        $crate::dto! { @acc $name, $dir, [$($s)*], [$($t)*], [$($d)*], [$($e)*, $extra] ; $($rest)* }
    };
    (@acc $name:ident, $dir:literal, [$($s:tt)*], [$($t:tt)*], [$($d:tt)*], [$($e:tt)*] ; $extra:ident ; $($rest:tt)*) => {
        $crate::dto! { @acc $name, $dir, [$($s)*], [$($t)*], [$($d)*], [$($e)*, $extra] ; ; $($rest)* }
    };

    // ===== 终结:选项耗尽,剩余即结构体 =====
    (@acc $name:ident, $dir:literal, [$($s:tt)*], [$($t:tt)*], [$($d:tt)*], [$($e:tt)*] ; ; { $($fields:tt)* }) => {
        $($d)*
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, validator::Validate $($e)*)]
        #[serde($($s)*)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[cfg_attr(feature = "ts", ts(export, export_to = $dir $($t)*))]
        pub struct $name { $($fields)* }
    };
}
