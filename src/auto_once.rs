use std::ops::Deref;
use std::sync::OnceLock;

pub struct AutoOnceLock<T>(OnceLock<T>);

impl<T> AutoOnceLock<T> {
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    pub fn set(&self, v: T) -> Result<(), T> {
        self.0.set(v)
    }
}

impl<T> Deref for AutoOnceLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self.0.get() {
            Some(v) => v,
            _ => panic!("not initialized"),
        }
    }
}
