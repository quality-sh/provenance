//! Deserialization of a struct with no fields and of an enum with no variants.
//!
//! The results and the error texts are equal to those of the serde derive for
//! the same types: a struct with no fields and `deny_unknown_fields`, and an
//! enum with no variants. The derive makes an enum with no variants for the
//! names of the fields or variants, and these functions make none.
use serde::de::{
    DeserializeSeed, Deserializer, EnumAccess, Error, MapAccess, SeqAccess, Unexpected, Visitor,
};
use std::{convert::Infallible, fmt};

/// Reads the struct `name`, which has no fields. It accepts a map with no
/// entries or a sequence, and it refuses the first field name in a map.
pub fn deserialize_struct<'de, D: Deserializer<'de>>(
    deserializer: D,
    name: &'static str,
) -> Result<(), D::Error> {
    deserializer.deserialize_struct(name, &[], Struct { name })
}

/// Reads the enum `name`, which has no variants, so each input is an error.
pub fn deserialize_enum<'de, D: Deserializer<'de>>(
    deserializer: D,
    name: &'static str,
) -> Result<Infallible, D::Error> {
    deserializer.deserialize_enum(name, &[], Enum { name })
}

struct Struct {
    name: &'static str,
}

impl<'de> Visitor<'de> for Struct {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "struct {}", self.name)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, _: A) -> Result<(), A::Error> {
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        if let Some(never) = map.next_key_seed(Name::Field)? {
            match never {}
        }
        Ok(())
    }
}

struct Enum {
    name: &'static str,
}

impl<'de> Visitor<'de> for Enum {
    type Value = Infallible;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "enum {}", self.name)
    }

    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Infallible, A::Error> {
        let (never, _) = data.variant_seed(Name::Variant)?;
        match never {}
    }
}

/// The name of a field or of a variant. The type has none, so each name is
/// an error.
#[derive(Clone, Copy)]
enum Name {
    Field,
    Variant,
}

impl Name {
    const fn word(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Variant => "variant",
        }
    }

    fn unknown<E: Error>(self, name: &str) -> E {
        match self {
            Self::Field => E::unknown_field(name, &[]),
            Self::Variant => E::unknown_variant(name, &[]),
        }
    }
}

impl<'de> DeserializeSeed<'de> for Name {
    type Value = Infallible;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Infallible, D::Error> {
        deserializer.deserialize_identifier(self)
    }
}

impl Visitor<'_> for Name {
    type Value = Infallible;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} identifier", self.word())
    }

    fn visit_u64<E: Error>(self, value: u64) -> Result<Infallible, E> {
        let expected = format!("{} index 0 <= i < 0", self.word());
        Err(E::invalid_value(
            Unexpected::Unsigned(value),
            &expected.as_str(),
        ))
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Infallible, E> {
        Err(self.unknown(value))
    }

    fn visit_bytes<E: Error>(self, value: &[u8]) -> Result<Infallible, E> {
        Err(self.unknown(&String::from_utf8_lossy(value)))
    }
}
