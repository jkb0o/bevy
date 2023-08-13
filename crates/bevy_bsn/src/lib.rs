mod from_bsn;
pub use from_bsn::*;

use std::{iter::Peekable, str::Chars};
use thiserror::Error;

#[derive(Debug, Eq, PartialEq)]
pub struct BsnEntity<'a> {
    pub name: Option<&'a str>,
    pub configs: Vec<BsnEntityConfig<'a>>,
    pub children: Vec<BsnEntity<'a>>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum BsnEntityConfig<'a> {
    Schematic {
        type_path: &'a str,
        schematic_type: SchematicType<'a>,
    },
    Scene {
        path: &'a str,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum SchematicType<'a> {
    Struct(BsnStruct<'a>),
    Enum(BsnEnum<'a>),
}

impl<'a> Into<BsnValue<'a>> for SchematicType<'a> {
    fn into(self) -> BsnValue<'a> {
        match self {
            SchematicType::Struct(value) => BsnValue::Struct(value),
            SchematicType::Enum(value) => BsnValue::Enum(value),
        }
    }
}

impl<'a> Parse<'a> for BsnEntityConfig<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError> {
        Ok(match cursor.peek()? {
            '@' => {
                cursor.next()?;
                let path = <&str>::parse(cursor)?;
                BsnEntityConfig::Scene { path }
            }
            _ => {
                let type_path = BsnTypePath::parse(cursor)?;
                cursor.skip_whitespace();
                let schematic_type = if type_path.is_enum_variant_next {
                    SchematicType::Enum(BsnEnum::parse(cursor)?)
                } else {
                    match cursor.peek() {
                        Ok('{') | Ok('(') => SchematicType::Struct(BsnStruct::parse(cursor)?),
                        Ok(_) | Err(NoCharError) => {
                            SchematicType::Struct(BsnStruct::Tuple(Vec::new()))
                        }
                    }
                };
                BsnEntityConfig::Schematic {
                    type_path: type_path.type_path,
                    schematic_type,
                }
            }
        })
    }
}

impl<'a> Parse<'a> for BsnEnum<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError> {
        let ident = BsnIdent::parse(cursor)?;
        cursor.skip_whitespace();
        let struct_type = match cursor.peek() {
            Ok('{') | Ok('(') => BsnStruct::parse(cursor)?,
            Ok(_) | Err(NoCharError) => BsnStruct::Tuple(Vec::new()),
        };
        Ok(BsnEnum {
            variant: ident.0,
            struct_type,
        })
    }
}
impl<'a> Parse<'a> for BsnStruct<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError> {
        Ok(match cursor.peek()? {
            '{' => {
                cursor.next()?;
                let mut fields = Vec::new();
                loop {
                    cursor.skip_whitespace();
                    if let Ok('}') = cursor.peek() {
                        cursor.next()?;
                        break BsnStruct::NamedFields(fields);
                    }
                    fields.push(BsnField::parse(cursor)?);
                }
            }
            '(' => {
                cursor.next()?;
                let mut values = Vec::new();
                let mut first = true;
                loop {
                    cursor.skip_whitespace();
                    if let Ok(')') = cursor.peek() {
                        cursor.next()?;
                        break BsnStruct::Tuple(values);
                    }

                    if !first {
                        let char = cursor.next()?;
                        if char != ',' {
                            return Err(ParseSceneError::TupleStructMissingComma);
                        }
                        cursor.skip_whitespace();
                    }

                    values.push(BsnValue::parse(cursor)?);
                    first = false;
                }
            }
            char => return Err(ParseSceneError::InvalidStructCharacter(char)),
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct BsnTypePath<'a> {
    type_path: &'a str,
    is_enum_variant_next: bool,
}

impl<'a> Parse<'a> for BsnTypePath<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError> {
        let type_path_start = cursor.index;
        BsnIdent::parse(cursor)?;
        let mut is_enum_variant_next = false;
        loop {
            match cursor.peek() {
                Ok(':') => {
                    cursor.next()?;
                    let char = cursor.peek()?;
                    if char == ':' {
                        cursor.next()?;
                        BsnIdent::parse(cursor)?;
                    } else {
                        is_enum_variant_next = true;
                        break;
                    }
                }
                Ok('<') => {
                    cursor.next()?;
                    let type_path = BsnTypePath::parse(cursor)?;
                    if type_path.is_enum_variant_next {
                        return Err(ParseSceneError::GenericInstancesCannotBeEnumVariants);
                    }
                    let char = cursor.next()?;
                    if char != '>' {
                        return Err(ParseSceneError::ExpectedClosingChar {
                            closing: '>',
                            found: char,
                        });
                    }

                    if let Ok(':') = cursor.peek() {
                        cursor.next()?;
                        is_enum_variant_next = true;
                    }
                    break;
                }
                Ok(_) => break,
                Err(NoCharError) => break,
            }
        }
        let end = if is_enum_variant_next {
            cursor.index - 1
        } else {
            cursor.index
        };
        let type_path = &cursor.str[type_path_start..end];
        Ok(BsnTypePath {
            type_path,
            is_enum_variant_next,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct BsnIdent<'a>(&'a str);

impl<'a> Parse<'a> for BsnIdent<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError> {
        let start = cursor.index;
        let char = cursor.next()?;
        if !char.is_alphabetic() {
            return Err(ParseSceneError::FirstCharacterInTypeNameMustBeAlphabetic(
                char,
            ));
        }

        loop {
            let char = cursor.peek()?;
            if char.is_alphanumeric() {
                cursor.next()?;
            } else {
                break;
            }
        }
        Ok(BsnIdent(&cursor.str[start..cursor.index]))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum BsnValue<'a> {
    Struct(BsnStruct<'a>),
    Enum(BsnEnum<'a>),
    Number(&'a str),
    String(&'a str),
}

impl<'a> Parse<'a> for BsnValue<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError> {
        let char = cursor.peek()?;
        Ok(if char.is_numeric() {
            let number_start = cursor.index;
            cursor.next()?;
            let mut number_end = cursor.index;
            loop {
                match cursor.peek() {
                    Ok(char) => {
                        if char.is_numeric() {
                            cursor.next()?;
                        } else if char.is_whitespace() || char == ',' || char == ')' || char == '}'
                        {
                            number_end = cursor.index;
                            break;
                        } else {
                            return Err(ParseSceneError::InvalidIntCharacter(char));
                        }
                    }
                    Err(NoCharError) => break,
                }
            }
            BsnValue::Number(&cursor.str[number_start..number_end])
        } else if char == '\"' {
            cursor.next()?;
            let start = cursor.index;
            let end;
            loop {
                match cursor.peek() {
                    Ok(char) => {
                        if char == '"' {
                            end = cursor.index;
                            cursor.next()?;
                            break;
                        }
                        cursor.next()?;
                    }
                    Err(NoCharError) => {
                        return Err(ParseSceneError::ExpectedClosingChar {
                            closing: '"',
                            found: ' ',
                        })
                    }
                }
            }
            BsnValue::String(&cursor.str[start..end])
        } else if char == '{' || char == '(' {
            BsnValue::Struct(BsnStruct::parse(cursor)?)
        } else if char.is_alphabetic() {
            BsnValue::Enum(BsnEnum::parse(cursor)?)
        } else {
            return Err(ParseSceneError::InvalidValueCharacter(char));
        })
    }
}

impl<'a> Parse<'a> for &'a str {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError> {
        let char = cursor.next()?;
        if char != '"' {
            return Err(ParseSceneError::ExpectedOpeningChar {
                opening: '"',
                found: char,
            });
        }
        let start = cursor.index;
        let end;
        loop {
            match cursor.peek() {
                Ok(char) => {
                    if char == '"' {
                        end = cursor.index;
                        cursor.next()?;
                        break;
                    }
                    cursor.next()?;
                }
                Err(NoCharError) => {
                    return Err(ParseSceneError::ExpectedClosingChar {
                        closing: '"',
                        found: ' ',
                    })
                }
            }
        }
        Ok(&cursor.str[start..end])
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum BsnStruct<'a> {
    Tuple(Vec<BsnValue<'a>>),
    NamedFields(Vec<BsnField<'a>>),
}

#[derive(Debug, Eq, PartialEq)]
pub struct BsnField<'a> {
    pub name: &'a str,
    pub value: BsnValue<'a>,
}

impl<'a> Parse<'a> for BsnField<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError> {
        let name_start = cursor.index;
        let char = cursor.peek()?;
        if !char.is_alphanumeric() || char.is_uppercase() {
            return Err(ParseSceneError::InvalidFieldCharacter(char));
        }
        cursor.next()?;

        let (name_end, value) = loop {
            let char = cursor.peek()?;
            if char == ':' {
                let name_end = cursor.index;
                cursor.next()?;
                cursor.skip_whitespace();
                break (name_end, BsnValue::parse(cursor)?);
            } else if char == '_' {
                cursor.next()?;
            } else if !char.is_alphanumeric() || char.is_uppercase() {
                // this is inverted to ensure we allow "non lowercase alphanumeric characters"
                return Err(ParseSceneError::InvalidFieldCharacter(char));
            } else {
                // this is an alphanumeric character that is either lowercase or has no case
                cursor.next()?;
            }
        };

        Ok(BsnField {
            name: &cursor.str[name_start..name_end],
            value,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct BsnEnum<'a> {
    pub variant: &'a str,
    pub struct_type: BsnStruct<'a>,
}

pub struct Cursor<'a> {
    str: &'a str,
    chars: Peekable<Chars<'a>>,
    index: usize,
}

impl<'a> Cursor<'a> {
    fn new(str: &'a str) -> Self {
        Self {
            str,
            chars: str.chars().peekable(),
            index: 0,
        }
    }

    fn next(&mut self) -> Result<char, NoCharError> {
        self.index += 1;
        self.chars.next().ok_or(NoCharError)
    }

    fn peek(&mut self) -> Result<char, NoCharError> {
        self.chars.peek().copied().ok_or(NoCharError)
    }

    fn skip_whitespace(&mut self) {
        while let Some(char) = self.chars.peek().copied() {
            if char.is_whitespace() {
                self.next().unwrap();
            } else {
                break;
            }
        }
    }
}

impl<'a> Parse<'a> for BsnEntity<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError> {
        let mut configs = Vec::new();
        let mut name = None;
        if let Ok('#') = cursor.peek() {
            cursor.next()?;
            let ident = BsnIdent::parse(cursor)?;
            name = Some(ident.0);
        }
        if let Ok('(') = cursor.peek() {
            cursor.next()?;
            loop {
                cursor.skip_whitespace();
                let char = cursor.peek()?;
                if char == ')' {
                    cursor.next()?;
                    break;
                }

                configs.push(BsnEntityConfig::parse(cursor)?);
            }
        } else {
            configs.push(BsnEntityConfig::parse(cursor)?);
        }

        cursor.skip_whitespace();
        let mut children = Vec::new();
        if let Ok('[') = cursor.peek() {
            cursor.next()?;
            loop {
                cursor.skip_whitespace();
                if let Ok(']') = cursor.peek() {
                    cursor.next()?;
                    break;
                }
                children.push(BsnEntity::parse(cursor)?);
            }
        }

        Ok(BsnEntity {
            name,
            configs,
            children,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct BsnScene<'a> {
    pub root: BsnEntity<'a>,
}

impl<'a> BsnScene<'a> {
    pub fn parse_str(str: &'a str) -> Result<Self, ParseSceneError> {
        Self::parse(&mut Cursor::new(str))
    }
}

impl<'a> Parse<'a> for BsnScene<'a> {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError> {
        cursor.skip_whitespace();
        let root = BsnEntity::parse(cursor)?;
        cursor.skip_whitespace();
        if let Ok(char) = cursor.peek() {
            return Err(ParseSceneError::UnexpectedChar(char));
        }
        Ok(BsnScene { root })
    }
}

pub trait Parse<'a>: Sized {
    fn parse(cursor: &mut Cursor<'a>) -> Result<Self, ParseSceneError>;
}

#[derive(Error, Debug)]
pub enum ParseSceneError {
    #[error(transparent)]
    NoCharError(#[from] NoCharError),
    #[error("Encountered unexpected character {0}")]
    UnexpectedChar(char),
    #[error("The first character in type names must be alphabetic, but it was {0}")]
    FirstCharacterInTypeNameMustBeAlphabetic(char),
    #[error("Expected a closing '{closing}' but found {found}")]
    ExpectedClosingChar { closing: char, found: char },
    #[error("Expected an opening '{opening}' but found {found}")]
    ExpectedOpeningChar { opening: char, found: char },
    #[error("Field names must consist of alphanumeric lowercase characters and '_', but encountered '{0}'")]
    InvalidFieldCharacter(char),
    #[error("Encountered an invalid integer character {0}")]
    InvalidIntCharacter(char),
    #[error("Encountered an invalid value character {0}")]
    InvalidValueCharacter(char),
    #[error("Encountered an invalid struct character {0}")]
    InvalidStructCharacter(char),
    #[error("Generic instances cannot be enum variants")]
    GenericInstancesCannotBeEnumVariants,
    #[error("Tuple struct values must be separated by commas")]
    TupleStructMissingComma,
}

#[derive(Error, Debug)]
#[error("Expected a character to exist, but it did not")]
pub struct NoCharError;

#[cfg(test)]
mod tests {
    use crate::{
        BsnEntity, BsnEntityConfig, BsnEnum, BsnField, BsnScene, BsnStruct, BsnValue, Cursor,
        Parse, SchematicType,
    };

    const SCENE: &str = r#"
Div:X { hello: 123 world: { key: 49 } } [
    Thing(1, 2)
    #SomeName(Thing(3) Marker @"scene.bsn")
    Some:Value
    Div { value: 7 }
]
"#;
    #[test]
    fn parse() {
        let bsn = BsnScene::parse(&mut Cursor::new(SCENE)).unwrap();
        let expected = BsnScene {
            root: BsnEntity {
                name: None,
                configs: vec![BsnEntityConfig::Schematic {
                    type_path: "Div",
                    schematic_type: SchematicType::Enum(BsnEnum {
                        variant: "X",
                        struct_type: BsnStruct::NamedFields(vec![
                            BsnField {
                                name: "hello",
                                value: BsnValue::Number("123"),
                            },
                            BsnField {
                                name: "world",
                                value: BsnValue::Struct(BsnStruct::NamedFields(vec![BsnField {
                                    name: "key",
                                    value: BsnValue::Number("49"),
                                }])),
                            },
                        ]),
                    }),
                }],
                children: vec![
                    BsnEntity {
                        name: None,
                        configs: vec![BsnEntityConfig::Schematic {
                            type_path: "Thing",
                            schematic_type: SchematicType::Struct(BsnStruct::Tuple(vec![
                                BsnValue::Number("1"),
                                BsnValue::Number("2"),
                            ])),
                        }],
                        children: Vec::new(),
                    },
                    BsnEntity {
                        name: Some("SomeName"),
                        configs: vec![
                            BsnEntityConfig::Schematic {
                                type_path: "Thing",
                                schematic_type: SchematicType::Struct(BsnStruct::Tuple(vec![
                                    BsnValue::Number("3"),
                                ])),
                            },
                            BsnEntityConfig::Schematic {
                                type_path: "Marker",
                                schematic_type: SchematicType::Struct(BsnStruct::Tuple(Vec::new())),
                            },
                            BsnEntityConfig::Scene { path: "scene.bsn" },
                        ],
                        children: Vec::new(),
                    },
                    BsnEntity {
                        name: None,
                        configs: vec![BsnEntityConfig::Schematic {
                            type_path: "Some",
                            schematic_type: SchematicType::Enum(BsnEnum {
                                variant: "Value",
                                struct_type: BsnStruct::Tuple(Vec::new()),
                            }),
                        }],
                        children: Vec::new(),
                    },
                    BsnEntity {
                        name: None,
                        configs: vec![BsnEntityConfig::Schematic {
                            type_path: "Div",
                            schematic_type: SchematicType::Struct(BsnStruct::NamedFields(vec![
                                BsnField {
                                    name: "value",
                                    value: BsnValue::Number("7"),
                                },
                            ])),
                        }],
                        children: Vec::new(),
                    },
                ],
            },
        };
        assert_eq!(bsn, expected);
    }
}
