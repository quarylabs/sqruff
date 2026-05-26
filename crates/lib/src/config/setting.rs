use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting<T> {
    Unset,
    Set(T),
}

impl<T> Default for Setting<T> {
    fn default() -> Self {
        Self::Unset
    }
}

impl<T> Setting<T> {
    pub fn is_set(&self) -> bool {
        matches!(self, Self::Set(_))
    }

    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Unset => None,
            Self::Set(value) => Some(value),
        }
    }
}

impl<'de, T> Deserialize<'de> for Setting<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self::Set)
    }
}

pub type NullableSetting<T> = Setting<Option<T>>;

pub trait Merge {
    fn merge(&mut self, other: Self);
}

impl<T> Merge for Setting<T> {
    fn merge(&mut self, other: Self) {
        if other.is_set() {
            *self = other;
        }
    }
}
