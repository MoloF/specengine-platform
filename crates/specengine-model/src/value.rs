//! Front-matter values kept as written: a generic YAML value and a map
//! that serialises in source order.

use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A YAML value of an untyped key (`extra`, `raised_by`).
#[derive(Debug, Clone, PartialEq)]
pub enum FmValue {
    Null,
    Bool(bool),
    Int(i64),
    /// An integer above `i64::MAX`.
    UInt(u64),
    Float(f64),
    Str(String),
    Seq(Vec<FmValue>),
    /// Keys in source order; a non-string key is kept as its scalar text.
    Map(OrderedMap<FmValue>),
}

impl Serialize for FmValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Int(value) => serializer.serialize_i64(*value),
            Self::UInt(value) => serializer.serialize_u64(*value),
            Self::Float(value) => serializer.serialize_f64(*value),
            Self::Str(value) => serializer.serialize_str(value),
            Self::Seq(items) => items.serialize(serializer),
            Self::Map(map) => map.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for FmValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(FmValueVisitor)
    }
}

struct FmValueVisitor;

impl<'de> Visitor<'de> for FmValueVisitor {
    type Value = FmValue;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any YAML value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<FmValue, E> {
        Ok(FmValue::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<FmValue, E> {
        Ok(FmValue::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<FmValue, D::Error> {
        FmValue::deserialize(deserializer)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<FmValue, E> {
        Ok(FmValue::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<FmValue, E> {
        Ok(FmValue::Int(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<FmValue, E> {
        Ok(i64::try_from(value).map_or(FmValue::UInt(value), FmValue::Int))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<FmValue, E> {
        Ok(FmValue::Float(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<FmValue, E> {
        Ok(FmValue::Str(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<FmValue, E> {
        Ok(FmValue::Str(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<FmValue, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(FmValue::Seq(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<FmValue, A::Error> {
        OrderedMapVisitor(PhantomData)
            .visit_map(map)
            .map(FmValue::Map)
    }
}

/// String keys in source order; serialises as a JSON object in that order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrderedMap<V>(pub Vec<(String, V)>);

impl<V> Default for OrderedMap<V> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<V> OrderedMap<V> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// The value of the first entry with `key`.
    pub fn get(&self, key: &str) -> Option<&V> {
        self.0
            .iter()
            .find(|(entry, _)| entry == key)
            .map(|(_, value)| value)
    }

    pub fn push(&mut self, key: impl Into<String>, value: V) {
        self.0.push((key.into(), value));
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }
}

impl<V: Serialize> Serialize for OrderedMap<V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for OrderedMap<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(OrderedMapVisitor(PhantomData))
    }
}

struct OrderedMapVisitor<V>(PhantomData<V>);

impl<'de, V: Deserialize<'de>> Visitor<'de> for OrderedMapVisitor<V> {
    type Value = OrderedMap<V>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a map")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OrderedMap<V>, A::Error> {
        let mut entries = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, V>()? {
            entries.push((key, value));
        }
        Ok(OrderedMap(entries))
    }
}
