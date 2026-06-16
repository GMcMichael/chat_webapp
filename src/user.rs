use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::{Database, Sqlite, Type};

pub type UserId = i64;

#[derive(Debug, PartialEq, Eq)]
pub struct ParseUserRoleError;

impl std::fmt::Display for ParseUserRoleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid user role")
    }
}

impl std::error::Error for ParseUserRoleError {}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum UserRole {
    Teacher,
    Parent,
}

impl UserRole {
    pub fn from_str(value: &str) -> Result<Self, ParseUserRoleError> {
        match value.to_lowercase().as_str() {
            "teacher" => Ok(UserRole::Teacher),
            "parent" => Ok(UserRole::Parent),
            _ => Err(ParseUserRoleError),
        }
    }
}

impl Type<Sqlite> for UserRole {
    fn type_info() -> <Sqlite as Database>::TypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
}

impl sqlx::Encode<'_, Sqlite> for UserRole {
    fn encode_by_ref(
        &self,
        buf: &mut <Sqlite as Database>::ArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <String as sqlx::Encode<'_, Sqlite>>::encode(self.to_string(), buf)
    }
}

impl sqlx::Decode<'_, Sqlite> for UserRole {
    fn decode(value: <Sqlite as Database>::ValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        UserRole::from_str(<&str as sqlx::Decode<'_, Sqlite>>::decode(value)?)
            .map_err(|e| Box::new(e) as sqlx::error::BoxDynError)
    }
}

impl TryFrom<String> for UserRole {
    type Error = ParseUserRoleError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        UserRole::from_str(&value)
    }
}

impl TryFrom<&str> for UserRole {
    type Error = ParseUserRoleError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        UserRole::from_str(value)
    }
}

impl FromStr for UserRole {
    type Err = ParseUserRoleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        UserRole::from_str(s)
    }
}

impl AsRef<str> for UserRole {
    fn as_ref(&self) -> &str {
        match self {
            UserRole::Teacher => "teacher",
            UserRole::Parent => "parent",
        }
    }
}

impl ToString for UserRole {
    fn to_string(&self) -> String {
        self.as_ref().to_string()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    id: Option<UserId>,
    name: String,
    role: UserRole,
}

impl User {
    pub fn new(id: Option<UserId>, name: String, role: UserRole) -> Self {
        Self { id, name, role }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn role(&self) -> &UserRole {
        &self.role
    }
}
