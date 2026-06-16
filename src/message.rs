use crate::{
    Timestamp,
    room::RoomId,
    user::{UserId, UserRole},
};
use serde::{Deserialize, Serialize};

#[allow(unused)]
pub type MessageId = i64;

#[derive(Debug, Serialize, Deserialize)]
pub struct NewMessage {
    room_id: RoomId,
    user_id: UserId,
    content: String,
}

impl NewMessage {
    #[allow(unused)]
    pub fn new(room_id: RoomId, user_id: UserId, content: String) -> Self {
        Self {
            room_id,
            user_id,
            content,
        }
    }

    pub fn room_id(&self) -> &RoomId {
        &self.room_id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessagePayload {
    user_name: String,
    role: UserRole,
    content: String,
    timestamp: Timestamp,
}

impl MessagePayload {
    pub fn new(user_name: String, role: UserRole, content: String, timestamp: Timestamp) -> Self {
        Self {
            user_name,
            role,
            content,
            timestamp,
        }
    }
}
