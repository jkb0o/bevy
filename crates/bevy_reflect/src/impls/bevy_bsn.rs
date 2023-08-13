use crate::{FromType, Reflect};
use bevy_bsn::{BsnValue, FromBsn, FromBsnError};

#[derive(Clone)]
pub struct ReflectFromBsn {
    pub from_bsn: for<'a> fn(BsnValue<'a>) -> Result<Box<dyn Reflect>, FromBsnError>,
}

impl ReflectFromBsn {
    pub fn from_bsn<'a>(&self, value: BsnValue<'a>) -> Result<Box<dyn Reflect>, FromBsnError> {
        (self.from_bsn)(value)
    }
}

impl<F: FromBsn + Reflect> FromType<F> for ReflectFromBsn {
    fn from_type() -> Self {
        Self {
            from_bsn: |value| Ok(Box::new(F::from_bsn(value)?)),
        }
    }
}
