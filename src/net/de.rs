use super::SerdeError;
use serde::de::{Error, IntoDeserializer};

pub struct BytesDeserializer<'a, B>
where
    B: bytes::Buf,
{
    buf: &'a mut B,
}

impl<'a, B> BytesDeserializer<'a, B>
where
    B: bytes::Buf,
{
    pub fn new(buf: &'a mut B) -> Self {
        Self { buf }
    }

    fn check_remaining(&self, requested: usize) -> Result<(), SerdeError> {
        let remaining = self.buf.remaining();
        if remaining < requested {
            return Err(SerdeError::custom(format!(
                "unexpected end of buffer: expected `{requested}` bytes but only `{remaining}` bytes remain"
            )));
        }
        Ok(())
    }

    fn deserialize_and_ensure_length(&mut self, ty: &'static str) -> Result<usize, SerdeError> {
        let len = usize::try_from(self.buf.try_get_u64()?).map_err(|_| {
            SerdeError::custom(format!(
                "failed to deserialize {ty} length: length exceeds usize::MAX on this machine"
            ))
        })?;
        self.check_remaining(len)?;
        Ok(len)
    }

    fn deserialize_bool_from_byte(&mut self) -> Result<bool, SerdeError> {
        let byte = self.buf.try_get_u8()?;
        match byte {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(SerdeError::custom(format!(
                "invalid byte value: expected `0` or `1` but got `{byte}`"
            ))),
        }
    }
}

impl<'de, B> serde::de::Deserializer<'de> for &'_ mut BytesDeserializer<'de, B>
where
    B: bytes::Buf,
{
    type Error = SerdeError;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Self::Error::custom("any deserialization is not supported"))
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let bool = self
            .deserialize_bool_from_byte()
            .map_err(|err| Self::Error::custom(format!("failed to deserialize bool: {err}")))?;
        visitor.visit_bool(bool)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i8(self.buf.try_get_i8()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i16(self.buf.try_get_i16()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i32(self.buf.try_get_i32()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i64(self.buf.try_get_i64()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u8(self.buf.try_get_u8()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u16(self.buf.try_get_u16()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u32(self.buf.try_get_u32()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u64(self.buf.try_get_u64()?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_f32(self.buf.try_get_f32()?)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_f64(self.buf.try_get_f64()?)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let codepoint = self.buf.try_get_u32()?;
        let char = char::from_u32(codepoint).ok_or_else(|| {
            Self::Error::custom(format!(
                "failed to deserialize char: `{codepoint:#X}` is not valid unicode scalar value"
            ))
        })?;
        visitor.visit_char(char)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.deserialize_and_ensure_length("string")?;
        let bytes = self.buf.copy_to_bytes(len);
        visitor.visit_str(std::str::from_utf8(&bytes)?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.deserialize_and_ensure_length("string")?;
        let bytes = self.buf.copy_to_bytes(len);
        visitor.visit_string(std::str::from_utf8(&bytes)?.to_owned())
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.deserialize_and_ensure_length("bytes")?;
        let bytes = self.buf.copy_to_bytes(len);
        visitor.visit_bytes(&bytes)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.deserialize_and_ensure_length("bytes")?;
        let bytes = self.buf.copy_to_bytes(len);
        visitor.visit_byte_buf(bytes.to_vec())
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let is_some = self
            .deserialize_bool_from_byte()
            .map_err(|err| Self::Error::custom(format!("failed to deserialize bool: {err}")))?;

        if is_some {
            return visitor.visit_some(self);
        }
        visitor.visit_none()
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor
            .visit_newtype_struct(self)
            .map_err(|err| Self::Error::custom(format!("failed to deserialize `{name}`: {err}")))
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.deserialize_and_ensure_length("sequence")?;
        visitor.visit_seq(SeqOrMapAccess {
            inner: self,
            remaining: len,
        })
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(SeqOrMapAccess {
            inner: self,
            remaining: len,
        })
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(NamedSeqAccess {
            name,
            inner: self,
            remaining: len,
        })
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.deserialize_and_ensure_length("map")?;
        visitor.visit_map(SeqOrMapAccess {
            inner: self,
            remaining: len,
        })
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(NamedFieldsSeqAccess {
            name,
            fields,
            inner: self,
            remaining: fields.len(),
        })
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let variant_index = self.buf.try_get_u32()?;

        let index = usize::try_from(variant_index).map_err(|_| {
            Self::Error::custom(
                "failed to deserialize variant index: index exceeds usize::MAX on this machine",
            )
        })?;
        let variant_name = variants.get(index).ok_or_else(|| {
            Self::Error::custom(format!(
                "invalid variant index `{}` for enum `{}` (expected `0`..`{}`)",
                variant_index,
                name,
                variants.len().saturating_sub(1)
            ))
        })?;

        visitor.visit_enum(EnumAccess {
            name,
            variant_name,
            variant_index,
            inner: self,
        })
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Self::Error::custom(
            "identifier deserialization is not supported",
        ))
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Self::Error::custom("any deserialization is not supported"))
    }
}

pub struct SeqOrMapAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    inner: &'a mut BytesDeserializer<'de, B>,
    remaining: usize,
}

impl<'a, 'de, B> serde::de::SeqAccess<'de> for SeqOrMapAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    type Error = SerdeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        if self.remaining == 0 {
            return Ok(None);
        }

        self.remaining -= 1;

        seed.deserialize(&mut *self.inner).map(Some)
    }
}

impl<'a, 'de, B> serde::de::MapAccess<'de> for SeqOrMapAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    type Error = SerdeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        if self.remaining == 0 {
            return Ok(None);
        }
        seed.deserialize(&mut *self.inner).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        self.remaining -= 1;
        seed.deserialize(&mut *self.inner)
    }
}

pub struct NamedSeqAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    inner: &'a mut BytesDeserializer<'de, B>,
    remaining: usize,
    name: &'static str,
}

impl<'a, 'de, B> serde::de::SeqAccess<'de> for NamedSeqAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    type Error = SerdeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        if self.remaining == 0 {
            return Ok(None);
        }

        self.remaining -= 1;

        seed.deserialize(&mut *self.inner)
            .map_err(|err| {
                Self::Error::custom(format!("failed to deserialize `{}`: {err}", self.name))
            })
            .map(Some)
    }
}

pub struct NamedFieldsSeqAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    inner: &'a mut BytesDeserializer<'de, B>,
    remaining: usize,
    name: &'static str,
    fields: &'static [&'static str],
}

impl<'a, 'de, B> serde::de::SeqAccess<'de> for NamedFieldsSeqAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    type Error = SerdeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        if self.remaining == 0 {
            return Ok(None);
        }

        let value = seed
            .deserialize(&mut *self.inner)
            .map_err(|err| {
                Self::Error::custom(format!(
                    "failed to deserialize `{}` in `{}`: {err}",
                    self.fields[self.fields.len() - self.remaining],
                    self.name
                ))
            })
            .map(Some);
        self.remaining -= 1;
        value
    }
}

pub struct EnumAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    inner: &'a mut BytesDeserializer<'de, B>,
    name: &'static str,
    variant_name: &'static str,
    variant_index: u32,
}

impl<'a, 'de, B> serde::de::EnumAccess<'de> for EnumAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    type Error = SerdeError;

    type Variant = VariantAccess<'a, 'de, B>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(IntoDeserializer::<'de, SerdeError>::into_deserializer(
            self.variant_index,
        ))?;

        Ok((
            variant,
            VariantAccess {
                inner: self.inner,
                name: self.name,
                variant_name: self.variant_name,
            },
        ))
    }
}

pub struct VariantAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    inner: &'a mut BytesDeserializer<'de, B>,
    name: &'static str,
    variant_name: &'static str,
}

impl<'a, 'de, B> serde::de::VariantAccess<'de> for VariantAccess<'a, 'de, B>
where
    B: bytes::Buf,
{
    type Error = SerdeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        seed.deserialize(self.inner).map_err(|err| {
            Self::Error::custom(format!(
                "failed to deserialize `{}::{}`: {err}",
                self.name, self.variant_name
            ))
        })
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor
            .visit_seq(SeqOrMapAccess {
                inner: self.inner,
                remaining: len,
            })
            .map_err(|err| {
                Self::Error::custom(format!(
                    "failed to deserialize `{}::{}`: {err}",
                    self.name, self.variant_name
                ))
            })
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(NamedFieldsSeqAccess {
            name: self.name,
            inner: self.inner,
            remaining: fields.len(),
            fields,
        })
    }
}
