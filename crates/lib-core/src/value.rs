use std::ops::Index;
use std::str::FromStr;

use hashbrown::HashMap;
use itertools::Itertools;

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum Value {
    Int(i32),
    Bool(bool),
    Float(f64),
    String(Box<str>),
    Map(HashMap<String, Value>),
    Array(Vec<Value>),
    #[default]
    None,
}

impl Value {
    pub fn is_none(&self) -> bool {
        matches!(self, Value::None)
    }

    pub fn as_array(&self) -> Option<Vec<Value>> {
        match self {
            Self::Array(v) => Some(v.clone()),
            Self::String(q) => {
                let xs = q
                    .split(',')
                    .map(|it| Value::String(it.into()))
                    .collect_vec();
                Some(xs)
            }
            Self::Bool(b) => Some(vec![Value::String(b.to_string().into())]),
            _ => None,
        }
    }
}

impl Index<&str> for Value {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        match self {
            Value::Map(map) => map.get(index).unwrap_or(&Value::None),
            _ => unreachable!(),
        }
    }
}

impl Value {
    pub fn to_bool(&self) -> bool {
        match *self {
            Value::Int(v) => v != 0,
            Value::Bool(v) => v,
            Value::Float(v) => v != 0.0,
            Value::String(ref v) => !v.is_empty(),
            Value::Map(ref v) => !v.is_empty(),
            Value::None => false,
            Value::Array(ref v) => !v.is_empty(),
        }
    }

    pub fn map<T>(&self, f: impl Fn(&Self) -> T) -> Option<T> {
        if self == &Value::None {
            return None;
        }

        Some(f(self))
    }

    pub fn as_map(&self) -> Option<&HashMap<String, Value>> {
        if let Self::Map(map) = self {
            Some(map)
        } else {
            None
        }
    }

    pub fn as_map_mut(&mut self) -> Option<&mut HashMap<String, Value>> {
        if let Self::Map(map) = self {
            Some(map)
        } else {
            None
        }
    }

    pub fn as_int(&self) -> Option<i32> {
        if let Self::Int(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        if let Self::String(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self {
            Some(*v)
        } else {
            None
        }
    }
}

impl FromStr for Value {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(value) = s.parse() {
            return Ok(Value::Int(value));
        }

        if let Ok(value) = s.parse() {
            return Ok(Value::Float(value));
        }

        let value = match () {
            _ if s.eq_ignore_ascii_case("true") => Value::Bool(true),
            _ if s.eq_ignore_ascii_case("false") => Value::Bool(false),
            _ if s.eq_ignore_ascii_case("none") => Value::None,
            _ => Value::String(Box::from(s)),
        };

        Ok(value)
    }
}
