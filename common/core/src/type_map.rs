use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

pub struct TypeMap {
    values: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Default for TypeMap {
    /// 创建空的 `TypeMap`
    fn default() -> Self {
        Self::new()
    }
}

impl TypeMap {
    /// 创建空的 `TypeMap`
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// 以值的类型为键插入或覆盖条目
    pub fn insert<T: Any + Send + Sync>(&mut self, value: T) {
        self.values.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// 按类型取出值的引用，不存在或类型不符时返回 `None`
    pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
    }
}
