use std::fmt::Debug;

pub mod scope;

pub type Array<T> = Box<[T]>;

#[repr(transparent)]
#[derive(PartialEq, Clone)]
pub struct Str(Box<str>);

impl Debug for Str {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::ops::Deref for Str {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl From<&str> for Str {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl PartialEq<str> for Str {
    fn eq(&self, other: &str) -> bool {
        return &*self.0 == other;
    }
}
