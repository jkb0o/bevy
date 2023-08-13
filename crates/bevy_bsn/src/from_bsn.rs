use crate::BsnValue;
use bevy_math::{Quat, Vec2, Vec3};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FromBsnError {
    #[error("Type did not match expected type")]
    MismatchedType,
    #[error("Encountered unexpected field {0}")]
    UnexpectedField(String),
    #[error(transparent)]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

// TODO: move all bsn code to bevy_bsn
pub trait FromBsn: Sized {
    fn from_bsn<'a>(value: BsnValue<'a>) -> Result<Self, FromBsnError>;
}

macro_rules! impl_with_parse {
    ($ty: ident) => {
        impl FromBsn for $ty {
            fn from_bsn<'a>(value: BsnValue<'a>) -> Result<Self, FromBsnError> {
                if let BsnValue::Number(value) = value {
                    let val: Self = value
                        .parse()
                        .map_err(|e| FromBsnError::Custom(Box::new(e)))?;
                    Ok(val)
                } else {
                    Err(FromBsnError::MismatchedType)
                }
            }
        }
    };
}

impl_with_parse!(u8);
impl_with_parse!(u16);
impl_with_parse!(u32);
impl_with_parse!(u64);
impl_with_parse!(u128);
impl_with_parse!(usize);
impl_with_parse!(f32);
impl_with_parse!(f64);

impl FromBsn for String {
    fn from_bsn<'a>(value: BsnValue<'a>) -> Result<Self, FromBsnError> {
        if let BsnValue::String(value) = value {
            Ok(value.to_string())
        } else {
            Err(FromBsnError::MismatchedType)
        }
    }
}

impl FromBsn for Vec2 {
    fn from_bsn<'a>(value: BsnValue<'a>) -> Result<Self, FromBsnError> {
        if let BsnValue::Struct(bsn_struct) = value {
            let mut value = Self::default();
            match bsn_struct {
                crate::BsnStruct::Tuple(_) => {}
                crate::BsnStruct::NamedFields(fields) => {
                    for field in fields {
                        match field.name {
                            "x" => {
                                value.x = f32::from_bsn(field.value)?;
                            }
                            "y" => {
                                value.y = f32::from_bsn(field.value)?;
                            }
                            _ => return Err(FromBsnError::UnexpectedField(field.name.to_string())),
                        }
                    }
                }
            }
            Ok(value)
        } else {
            Err(FromBsnError::MismatchedType)
        }
    }
}

impl FromBsn for Vec3 {
    fn from_bsn<'a>(value: BsnValue<'a>) -> Result<Self, FromBsnError> {
        if let BsnValue::Struct(bsn_struct) = value {
            let mut value = Self::default();
            match bsn_struct {
                crate::BsnStruct::Tuple(_) => {}
                crate::BsnStruct::NamedFields(fields) => {
                    for field in fields {
                        match field.name {
                            "x" => {
                                value.x = f32::from_bsn(field.value)?;
                            }
                            "y" => {
                                value.y = f32::from_bsn(field.value)?;
                            }
                            "z" => {
                                value.z = f32::from_bsn(field.value)?;
                            }
                            _ => return Err(FromBsnError::UnexpectedField(field.name.to_string())),
                        }
                    }
                }
            }
            Ok(value)
        } else {
            Err(FromBsnError::MismatchedType)
        }
    }
}

impl FromBsn for Quat {
    fn from_bsn<'a>(value: BsnValue<'a>) -> Result<Self, FromBsnError> {
        if let BsnValue::Struct(bsn_struct) = value {
            let mut value = Self::default();
            match bsn_struct {
                crate::BsnStruct::Tuple(_) => {}
                crate::BsnStruct::NamedFields(fields) => {
                    for field in fields {
                        match field.name {
                            "w" => {
                                value.w = f32::from_bsn(field.value)?;
                            }
                            "x" => {
                                value.x = f32::from_bsn(field.value)?;
                            }
                            "y" => {
                                value.y = f32::from_bsn(field.value)?;
                            }
                            "z" => {
                                value.z = f32::from_bsn(field.value)?;
                            }
                            _ => return Err(FromBsnError::UnexpectedField(field.name.to_string())),
                        }
                    }
                }
            }
            Ok(value)
        } else {
            Err(FromBsnError::MismatchedType)
        }
    }
}

impl FromBsn for () {
    fn from_bsn<'a>(value: BsnValue<'a>) -> Result<Self, FromBsnError> {
        if let BsnValue::Struct(crate::BsnStruct::Tuple(fields)) = value {
            if fields.is_empty() {
                Ok(())
            } else {
                Err(FromBsnError::MismatchedType)
            }
        } else {
            Err(FromBsnError::MismatchedType)
        }
    }
}
