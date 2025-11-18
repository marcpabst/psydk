use std::{any::Any, fmt::Debug};

pub use super::scenes::Scene;

#[derive(Debug)]
/// A dynamic bitmap type that can hold backend-specific bitmap implementations.
/// A bitmap is usually immutable once created.
pub struct DynamicBitmap(pub Box<dyn Bitmap>);

impl DynamicBitmap {
    pub fn try_as<T>(&self) -> Option<&T>
    where
        T: Any,
    {
        self.0.as_any().downcast_ref::<T>()
    }
}

pub trait Bitmap: Any + Debug {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
