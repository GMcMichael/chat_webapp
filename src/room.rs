use serde::{Deserialize, Serialize};

pub type RoomId = i64;

#[derive(Debug, Serialize, Deserialize)]
pub struct Room {
    id: Option<RoomId>,
    name: String,
}

impl Room {
    pub fn new(id: Option<RoomId>, name: String) -> Self {
        Self { id, name }
    }
}
