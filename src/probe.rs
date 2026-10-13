//! The deserializer [`crate::shape`] reads a type's declaration with: it
//! answers each question a `Deserialize` type asks with a value of the kind
//! asked for, and writes down at which key path what was asked — a struct's
//! fields, an enum's words, the kind of a value, a field it requires.

use std::cell::RefCell;

use serde::de::value::StrDeserializer;
use serde::de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};

use crate::shape::{State, Stop};

/// The deserializer that listens: at `path`, an entry of a list when
/// `element`.
pub(crate) struct Probe<'s> {
    pub(crate) path: Vec<String>,
    pub(crate) element: bool,
    pub(crate) state: &'s RefCell<State>,
}

impl Probe<'_> {
    fn child(&self, key: &str) -> Self {
        Probe {
            path: [self.path.clone(), vec![key.to_string()]].concat(),
            element: false,
            state: self.state,
        }
    }

    /// Say the kind read here; an entry of a list says only that it is a
    /// table, which makes the list a list of tables.
    fn kind(&self, kind: &'static str) {
        let mut state = self.state.borrow_mut();
        let shape = state.shapes.entry(self.path.clone()).or_default();
        if self.element {
            if kind == "table" || kind == "free" {
                shape.kind = "list";
            }
        } else if shape.kind.is_empty() || shape.kind == "any" {
            shape.kind = kind;
        }
    }
}

macro_rules! answered {
    ($($method:ident => $kind:literal, $visit:ident($($value:expr)?);)*) => {
        $(fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Stop> {
            self.kind($kind);
            visitor.$visit($($value)?)
        })*
    };
}

impl<'de> de::Deserializer<'de> for Probe<'_> {
    type Error = Stop;

    answered! {
        deserialize_bool => "boolean", visit_bool(false);
        deserialize_i8 => "integer", visit_i64(0);
        deserialize_i16 => "integer", visit_i64(0);
        deserialize_i32 => "integer", visit_i64(0);
        deserialize_i64 => "integer", visit_i64(0);
        deserialize_u8 => "integer", visit_u64(0);
        deserialize_u16 => "integer", visit_u64(0);
        deserialize_u32 => "integer", visit_u64(0);
        deserialize_u64 => "integer", visit_u64(0);
        deserialize_f32 => "float", visit_f64(0.0);
        deserialize_f64 => "float", visit_f64(0.0);
        deserialize_char => "text", visit_char('x');
        deserialize_str => "text", visit_str("x");
        deserialize_string => "text", visit_str("x");
        deserialize_identifier => "text", visit_str("x");
        deserialize_bytes => "any", visit_bytes(b"x");
        deserialize_byte_buf => "any", visit_bytes(b"x");
        deserialize_unit => "any", visit_unit();
        deserialize_any => "any", visit_unit();
        deserialize_ignored_any => "any", visit_unit();
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Stop> {
        visitor.visit_some(self)
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Stop> {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Stop> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Stop> {
        self.kind("array");
        visitor.visit_seq(One(Some(Probe {
            element: true,
            ..self
        })))
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _: usize, visitor: V) -> Result<V::Value, Stop> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: usize,
        visitor: V,
    ) -> Result<V::Value, Stop> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Stop> {
        self.kind("free");
        visitor.visit_map(Fields {
            probe: self,
            keys: Vec::new(),
        })
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Stop> {
        self.kind("table");
        let keys = {
            let mut state = self.state.borrow_mut();
            let shape = state.shapes.entry(self.path.clone()).or_default();
            fields.clone_into(&mut shape.fields);
            state
                .given
                .get(&self.path)
                .cloned()
                .unwrap_or_else(|| fields.to_vec())
        };
        let (path, state) = (self.path.clone(), self.state);
        let read = visitor.visit_map(Fields {
            probe: self,
            keys: keys.into_iter().rev().collect(),
        });
        match read {
            Err(Stop(Some(field))) if fields.contains(&field) => {
                state.borrow_mut().missing = Some((path, field));
                Err(Stop(None))
            }
            other => other,
        }
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Stop> {
        self.kind("choice");
        variants.clone_into(
            &mut self
                .state
                .borrow_mut()
                .shapes
                .entry(self.path.clone())
                .or_default()
                .choices,
        );
        let word = variants.first().copied().unwrap_or_default();
        visitor.visit_enum(Case { probe: self, word })
    }
}

/// A list of one entry.
struct One<'s>(Option<Probe<'s>>);

impl<'de> SeqAccess<'de> for One<'_> {
    type Error = Stop;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Stop> {
        self.0
            .take()
            .map(|probe| seed.deserialize(probe))
            .transpose()
    }
}

/// A table's keys, each given a value of the kind its reader asks for.
struct Fields<'s> {
    probe: Probe<'s>,
    /// What is still to be given, the next last.
    keys: Vec<&'static str>,
}

impl<'de> MapAccess<'de> for Fields<'_> {
    type Error = Stop;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Stop> {
        let Some(key) = self.keys.last() else {
            return Ok(None);
        };
        seed.deserialize(StrDeserializer::<Stop>::new(key))
            .map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Stop> {
        let key = self.keys.pop().unwrap_or_default();
        seed.deserialize(self.probe.child(key))
    }
}

/// An enum's first case, read under its word.
struct Case<'s> {
    probe: Probe<'s>,
    word: &'static str,
}

impl<'de, 's> EnumAccess<'de> for Case<'s> {
    type Error = Stop;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self), Stop> {
        let word = seed.deserialize(StrDeserializer::<Stop>::new(self.word))?;
        Ok((word, self))
    }
}

impl<'de> VariantAccess<'de> for Case<'_> {
    type Error = Stop;

    fn unit_variant(self) -> Result<(), Stop> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Stop> {
        if let Some(shape) = self
            .probe
            .state
            .borrow_mut()
            .shapes
            .get_mut(&self.probe.path)
        {
            shape.kind = "variant";
        }
        seed.deserialize(self.probe.child(self.word))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _: usize, visitor: V) -> Result<V::Value, Stop> {
        visitor.visit_seq(One(None))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Stop> {
        let word = self.word;
        de::Deserializer::deserialize_struct(self.probe.child(word), "", fields, visitor)
    }
}
