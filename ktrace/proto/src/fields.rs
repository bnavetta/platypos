//! Serializable adapter for trace attributes

use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use phf::phf_map;
use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Serialize, Serializer};
use tracing::field::{Field, Visit};
use tracing::span::Attributes;
use tracing::Event;

#[derive(Debug)]
pub struct SerializeAttributes<'a>(&'a Attributes<'a>);

impl<'a> Serialize for SerializeAttributes<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let values = self.0.values();
        serialize_fields(serializer, values.len(), |f| values.record(f))
    }
}

impl<'a> From<&'a Attributes<'a>> for SerializeAttributes<'a> {
    #[inline(always)]
    fn from(attrs: &'a Attributes<'a>) -> Self {
        SerializeAttributes(attrs)
    }
}

#[derive(Debug)]
pub struct SerializeEvent<'a>(&'a Event<'a>);

impl<'a> Serialize for SerializeEvent<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_fields(serializer, self.0.fields().count(), |f| self.0.record(f))
    }
}

impl<'a> From<&'a Event<'a>> for SerializeEvent<'a> {
    #[inline(always)]
    fn from(event: &'a Event<'a>) -> Self {
        SerializeEvent(event)
    }
}

#[inline(always)]
fn serialize_fields<S: Serializer, F: FnOnce(&mut FieldVisitor<S::SerializeMap>)>(
    serializer: S,
    len: usize,
    f: F,
) -> Result<S::Ok, S::Error> {
    let map = serializer.serialize_map(Some(len))?;
    let mut visitor = FieldVisitor::new(map);
    f(&mut visitor);
    visitor.finish()
}

#[derive(Debug)]
pub struct DeserializedFields<'a> {
    fields: Vec<(&'a str, Value<'a>)>,
}

impl<'a> DeserializedFields<'a> {
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(&'a str, Value<'a>)> {
        self.fields.iter()
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for DeserializedFields<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FieldsVisitor;

        impl<'de> Visitor<'de> for FieldsVisitor {
            type Value = DeserializedFields<'de>;

            fn expecting(&self, formatter: &mut Formatter) -> alloc::fmt::Result {
                formatter.write_str("fields")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut fields = Vec::new();
                while let Some(field) = map.next_key()? {
                    if let Some(ty) = TYPES.get(field) {
                        let value = ty.read(&mut map)?;
                        fields.push((field, value));
                    } else {
                        return Err(A::Error::custom(format_args!("unknown field {field}")));
                    }
                }

                Ok(DeserializedFields { fields })
            }
        }

        deserializer.deserialize_map(FieldsVisitor)
    }
}

/// Dynamic field value, for use on the materialized side
#[derive(Debug, PartialEq, Eq)]
pub enum Value<'a> {
    KernelAddress(u64),
    PhysicalAddress(u64),
    VirtualAddress(u64),
    String(&'a str),
    U64(u64),
}

/// Mapping of known fields to their expected types. This forms a dynamic
/// schema, where any given data point can contain 0 or more known fields.
static TYPES: phf::Map<&'static str, FieldType> = phf_map! {
    "at" => FieldType::KernelAddress,
    "message" => FieldType::String,
    "count" => FieldType::U64,
    "size" => FieldType::U64,
    "vaddr" => FieldType::VirtualAddress,
    "paddr" => FieldType::PhysicalAddress,
    "range" => FieldType::String,
};

#[derive(Clone, Copy, Debug)]
pub enum FieldType {
    KernelAddress,
    PhysicalAddress,
    VirtualAddress,
    String,
    U64,
}

impl FieldType {
    fn write_u64<S: SerializeMap>(self, name: &str, value: u64, s: &mut S) -> Result<(), S::Error> {
        match self {
            FieldType::KernelAddress
            | FieldType::U64
            | FieldType::PhysicalAddress
            | FieldType::VirtualAddress => s.serialize_entry(name, &value),
            other => Err(S::Error::custom(format_args!(
                "{name} value must be a {other:?}, got u64"
            ))),
        }
    }

    fn write_str<S: SerializeMap>(
        self,
        name: &str,
        value: &str,
        s: &mut S,
    ) -> Result<(), S::Error> {
        match self {
            FieldType::String => s.serialize_entry(name, &value),
            other => Err(S::Error::custom(format_args!(
                "{name} value must be a {other:?}, got str"
            ))),
        }
    }

    fn write_debug<S: SerializeMap>(
        self,
        name: &str,
        value: &dyn Debug,
        s: &mut S,
    ) -> Result<(), S::Error> {
        match self {
            FieldType::String => {
                struct SerializeDebug<'a>(&'a dyn Debug);

                impl<'a> Serialize for SerializeDebug<'a> {
                    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                    where
                        S: Serializer,
                    {
                        serializer.collect_str(&format_args!("{:?}", self.0))
                    }
                }

                s.serialize_entry(name, &SerializeDebug(value))
            }
            other => Err(S::Error::custom(format_args!(
                "{name} value must be a {other:?}, got str"
            ))),
        }
    }

    fn read<'a, M: MapAccess<'a>>(self, map: &mut M) -> Result<Value<'a>, M::Error> {
        match self {
            FieldType::KernelAddress => Ok(Value::KernelAddress(map.next_value()?)),
            FieldType::U64 => Ok(Value::U64(map.next_value()?)),
            FieldType::PhysicalAddress => Ok(Value::PhysicalAddress(map.next_value()?)),
            FieldType::VirtualAddress => Ok(Value::VirtualAddress(map.next_value()?)),
            FieldType::String => Ok(Value::String(map.next_value()?)),
        }
    }
}

struct FieldVisitor<S: SerializeMap> {
    state: Result<(), S::Error>,
    serializer: S,
}

impl<S: SerializeMap> FieldVisitor<S> {
    fn new(serializer: S) -> Self {
        Self {
            state: Ok(()),
            serializer,
        }
    }

    fn finish(self) -> Result<S::Ok, S::Error> {
        self.state?;
        self.serializer.end()
    }
}

impl<S: SerializeMap> Visit for FieldVisitor<S> {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if self.state.is_ok() {
            let name = field.name();
            if let Some(ty) = TYPES.get(name) {
                self.state = ty.write_debug(field.name(), value, &mut self.serializer);
            } else {
                panic!("unknown field: {field}");
            }
        }
    }

    fn record_f64(&mut self, _field: &Field, _valuee: f64) {
        panic!("no known fields use f64");
    }

    fn record_i64(&mut self, _field: &Field, _value: i64) {
        panic!("no known fields use i64");
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if self.state.is_ok() {
            if let Some(ty) = TYPES.get(field.name()) {
                self.state = ty.write_u64(field.name(), value, &mut self.serializer);
            } else {
                panic!("unknown field: {field}")
            }
        }
    }

    fn record_i128(&mut self, _field: &Field, _value: i128) {
        panic!("no known fields use i128");
    }

    fn record_u128(&mut self, _field: &Field, _value: u128) {
        panic!("no known fields use u128");
    }

    fn record_bool(&mut self, _field: &Field, _value: bool) {
        panic!("no known fields use bool");
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if self.state.is_ok() {
            if let Some(ty) = TYPES.get(field.name()) {
                self.state = ty.write_str(field.name(), value, &mut self.serializer);
            } else {
                panic!("unknown field: {field}")
            }
        }
    }
}
