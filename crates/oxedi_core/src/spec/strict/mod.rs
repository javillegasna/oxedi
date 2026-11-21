//! A deserializer over a parsed JSON value that reads it as `serde_json`
//! does, except that a struct is read only from an object: serde_json's own
//! deserializer also reads a struct from an array. Every value under an
//! array, an object or an enum is read through it too, so the rule holds at
//! every depth.
//!
//! Map keys are read as strings only (every model key is a `String`); another key type is refused.
//!
//! The methods that hold no nested value forward to the value itself, which
//! keeps serde_json's reading and its error text for them.

use serde::Deserialize;
use serde::de::value::BorrowedStrDeserializer;
use serde::de::{
    self, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess, Unexpected,
    VariantAccess, Visitor,
};
use serde_json::{Error, Map, Value};

#[cfg(test)]
mod tests;

/// A JSON value read strictly.
#[derive(Clone, Copy)]
pub(in crate::spec) struct StrictValue<'de>(pub(in crate::spec) &'de Value);

macro_rules! forward_to_value {
    ($($method:ident)*) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            self.0.$method(visitor)
        }
    )*};
}

// Methods that read as another one of the same deserializer.
macro_rules! read_as {
    ($($method:ident($($arg:ident: $ty:ty),*) => $target:ident)*) => {$(
        fn $method<V: Visitor<'de>>(self, $($arg: $ty,)* visitor: V) -> Result<V::Value, Error> {
            self.$target(visitor)
        }
    )*};
}

// Visits the items, then refuses when the visitor left some, as serde_json does.
fn visit_array<'de, V: Visitor<'de>>(items: &'de [Value], visitor: V) -> Result<V::Value, Error> {
    let mut seq = StrictSeq {
        items: items.iter(),
    };
    let read = visitor.visit_seq(&mut seq)?;
    if seq.items.len() > 0 {
        return Err(de::Error::invalid_length(
            items.len(),
            &"fewer elements in array",
        ));
    }
    Ok(read)
}

// Visits the entries, then refuses when the visitor left some.
fn visit_object<'de, V: Visitor<'de>>(
    map: &'de Map<String, Value>,
    visitor: V,
) -> Result<V::Value, Error> {
    let mut access = StrictMap {
        entries: map.iter(),
        value: None,
    };
    let read = visitor.visit_map(&mut access)?;
    if access.entries.len() > 0 {
        return Err(de::Error::invalid_length(
            map.len(),
            &"fewer elements in map",
        ));
    }
    Ok(read)
}

impl<'de> Deserializer<'de> for StrictValue<'de> {
    type Error = Error;

    forward_to_value! {
        deserialize_bool deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64
        deserialize_i128 deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64
        deserialize_u128 deserialize_f32 deserialize_f64 deserialize_char deserialize_str
        deserialize_string deserialize_bytes deserialize_byte_buf deserialize_unit
        deserialize_identifier deserialize_ignored_any
    }

    read_as! {
        deserialize_unit_struct(_name: &'static str) => deserialize_unit
        deserialize_tuple(_len: usize) => deserialize_seq
        deserialize_tuple_struct(_name: &'static str, _len: usize) => deserialize_seq
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            Value::Array(items) => visit_array(items, visitor),
            Value::Object(map) => visit_object(map, visitor),
            other => other.deserialize_any(visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            Value::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            Value::Array(items) => visit_array(items, visitor),
            other => other.deserialize_seq(visitor),
        }
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            Value::Object(map) => visit_object(map, visitor),
            other => other.deserialize_map(visitor),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.0 {
            Value::Object(map) => visit_object(map, visitor),
            Value::Array(_) => Err(de::Error::invalid_type(Unexpected::Seq, &visitor)),
            other => other.deserialize_struct(name, fields, visitor),
        }
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.0 {
            Value::String(variant) => visitor.visit_enum(StrictEnum {
                variant,
                value: None,
            }),
            Value::Object(map) => match (map.len(), map.iter().next()) {
                (1, Some((variant, value))) => visitor.visit_enum(StrictEnum {
                    variant,
                    value: Some(value),
                }),
                _ => map.deserialize_enum(name, variants, visitor),
            },
            other => other.deserialize_enum(name, variants, visitor),
        }
    }
}

struct StrictSeq<'de> {
    items: std::slice::Iter<'de, Value>,
}

impl<'de> SeqAccess<'de> for &mut StrictSeq<'de> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        self.items
            .next()
            .map(|item| seed.deserialize(StrictValue(item)))
            .transpose()
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.items.len())
    }
}

struct StrictMap<'de> {
    entries: serde_json::map::Iter<'de>,
    value: Option<&'de Value>,
}

impl<'de> MapAccess<'de> for &mut StrictMap<'de> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.entries.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(BorrowedStrDeserializer::<Error>::new(key))
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<T::Value, Error> {
        match self.value.take() {
            Some(value) => seed.deserialize(StrictValue(value)),
            None => Err(de::Error::custom("value is missing")),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len())
    }
}

struct StrictEnum<'de> {
    variant: &'de str,
    value: Option<&'de Value>,
}

impl<'de> EnumAccess<'de> for StrictEnum<'de> {
    type Error = Error;
    type Variant = StrictVariant<'de>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, StrictVariant<'de>), Error> {
        let variant = seed.deserialize(BorrowedStrDeserializer::<Error>::new(self.variant))?;
        Ok((variant, StrictVariant { value: self.value }))
    }
}

struct StrictVariant<'de> {
    value: Option<&'de Value>,
}

impl<'de> StrictVariant<'de> {
    fn payload(self, expected: &'static str) -> Result<StrictValue<'de>, Error> {
        match self.value {
            Some(value) => Ok(StrictValue(value)),
            None => Err(de::Error::invalid_type(Unexpected::UnitVariant, &expected)),
        }
    }
}

impl<'de> VariantAccess<'de> for StrictVariant<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        match self.value {
            Some(value) => <()>::deserialize(StrictValue(value)),
            None => Ok(()),
        }
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(self.payload("newtype variant")?)
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        self.payload("tuple variant")?.deserialize_seq(visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.payload("struct variant")?
            .deserialize_struct("", fields, visitor)
    }
}
