use super::SerdeError;
use serde::ser::Error;

pub struct BytesSerializer<'a, B>
where
    B: bytes::BufMut,
{
    buf: &'a mut B,
}

impl<'a, B> BytesSerializer<'a, B>
where
    B: bytes::BufMut,
{
    pub fn new(buf: &'a mut B) -> Self {
        Self { buf }
    }
}

impl<'a, 'b, B> serde::Serializer for &'b mut BytesSerializer<'a, B>
where
    B: bytes::BufMut,
{
    type Ok = ();

    type Error = SerdeError;

    type SerializeSeq = Self;

    type SerializeTuple = Self;

    type SerializeTupleStruct = NamedByteSerializer<'a, 'b, B>;

    type SerializeTupleVariant = VariantByteSerializer<'a, 'b, B>;

    type SerializeMap = Self;

    type SerializeStruct = NamedByteSerializer<'a, 'b, B>;

    type SerializeStructVariant = VariantByteSerializer<'a, 'b, B>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.buf.put_u8(if v { 1 } else { 0 });
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.buf.put_i8(v);
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.buf.put_i16(v);
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.buf.put_i32(v);
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.buf.put_i64(v);
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.buf.put_u8(v);
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.buf.put_u16(v);
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.buf.put_u32(v);
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.buf.put_u64(v);
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.buf.put_f32(v);
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.buf.put_f64(v);
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.buf.put_u32(u32::from(v));
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.buf.put_u64(u64::try_from(v.len()).map_err(|_| {
            Self::Error::custom("string length exceeds `18446744073709551615` bytes")
        })?);
        self.buf.put_slice(v.as_bytes());
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.buf.put_u64(
            u64::try_from(v.len()).map_err(|_| {
                Self::Error::custom("bytes len exceeds `18446744073709551615` bytes")
            })?,
        );
        self.buf.put_slice(v);
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.buf.put_u8(0);
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.buf.put_u8(1);
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.buf.put_u32(variant_index);
        Ok(())
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value
            .serialize(self)
            .map_err(|err| Self::Error::custom(format!("failed to serialize `{name}`: {err}")))
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.buf.put_u32(variant_index);
        value.serialize(self).map_err(|err| {
            Self::Error::custom(format!("failed to serialize `{name}::{variant}`: {err}"))
        })
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let len = len.ok_or_else(|| Self::Error::custom("sequence length must be known"))?;
        self.buf.put_u64(u64::try_from(len).map_err(|_| {
            Self::Error::custom("sequence length exceeds `18446744073709551615` bytes")
        })?);
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(self)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(NamedByteSerializer { name, inner: self })
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.buf.put_u32(variant_index);
        Ok(VariantByteSerializer {
            name,
            variant,
            inner: self,
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        let len = len.ok_or_else(|| Self::Error::custom("map length must be known"))?;
        self.buf.put_u64(
            u64::try_from(len).map_err(|_| {
                Self::Error::custom("map length exceeds `18446744073709551615` bytes")
            })?,
        );
        Ok(self)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(NamedByteSerializer { name, inner: self })
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.buf.put_u32(variant_index);
        Ok(VariantByteSerializer {
            name,
            variant,
            inner: self,
        })
    }
}

impl<'a, 'b, B> serde::ser::SerializeSeq for &'b mut BytesSerializer<'a, B>
where
    B: bytes::BufMut,
{
    type Ok = ();

    type Error = SerdeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a, 'b, B> serde::ser::SerializeTuple for &'b mut BytesSerializer<'a, B>
where
    B: bytes::BufMut,
{
    type Ok = ();

    type Error = SerdeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a, 'b, B> serde::ser::SerializeMap for &'b mut BytesSerializer<'a, B>
where
    B: bytes::BufMut,
{
    type Ok = ();

    type Error = SerdeError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        key.serialize(&mut **self)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

pub struct NamedByteSerializer<'a, 'b, B>
where
    B: bytes::BufMut,
{
    name: &'static str,
    inner: &'b mut BytesSerializer<'a, B>,
}

impl<'a, 'b, B> serde::ser::SerializeTupleStruct for NamedByteSerializer<'a, 'b, B>
where
    B: bytes::BufMut,
{
    type Ok = ();

    type Error = SerdeError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(&mut *self.inner).map_err(|err| {
            Self::Error::custom(format!("failed to serialize `{}`: {err}", self.name))
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a, 'b, B> serde::ser::SerializeStruct for NamedByteSerializer<'a, 'b, B>
where
    B: bytes::BufMut,
{
    type Ok = ();

    type Error = SerdeError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(&mut *self.inner).map_err(|err| {
            Self::Error::custom(format!(
                "failed to serialize `{key}` in `{}`: {err}",
                self.name,
            ))
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

pub struct VariantByteSerializer<'a, 'b, B>
where
    B: bytes::BufMut,
{
    name: &'static str,
    variant: &'static str,
    inner: &'b mut BytesSerializer<'a, B>,
}

impl<'a, 'b, B> serde::ser::SerializeTupleVariant for VariantByteSerializer<'a, 'b, B>
where
    B: bytes::BufMut,
{
    type Ok = ();

    type Error = SerdeError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(&mut *self.inner).map_err(|err| {
            Self::Error::custom(format!(
                "failed serialize `{}::{}`: {err}",
                self.name, self.variant,
            ))
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a, 'b, B> serde::ser::SerializeStructVariant for VariantByteSerializer<'a, 'b, B>
where
    B: bytes::BufMut,
{
    type Ok = ();

    type Error = SerdeError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(&mut *self.inner).map_err(|err| {
            Self::Error::custom(format!(
                "failed to serialize `{key}` in `{}::{}`: {err}",
                self.name, self.variant
            ))
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}
