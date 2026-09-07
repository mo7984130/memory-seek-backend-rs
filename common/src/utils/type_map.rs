use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

pub struct TypeMap {
    values: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl TypeMap {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn insert<T: Any + Send + Sync>(&mut self, value: T) {
        self.values.insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
    }
}
