use crate::{
    Timestamp,
    message::MessagePayload,
    room::{Room, RoomId},
    user::{User, UserId, UserRole},
};
use anyhow::Ok;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

pub async fn init_db(pool: &SqlitePool) -> anyhow::Result<()> {
    let schema = include_str!("../schema.sql");
    sqlx::raw_sql(schema).execute(pool).await?;
    sqlx::query("PRAGMA journal_mode = WAL;")
        .execute(pool)
        .await?;

    // default rooms
    sqlx::query(
        "INSERT OR IGNORE INTO chatrooms (id, name) VALUES
         (1, 'Class Discussion'),
         (2, 'Teacher-Parent Chat')",
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn clear_table(pool: &SqlitePool, table: &str) -> anyhow::Result<u64> {
    Ok(
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {table};"))) // user AssertSqlSafe() wrapper
            .bind(table)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

pub async fn create_user(pool: &SqlitePool, name: String, role: UserRole) -> anyhow::Result<User> {
    let id = sqlx::query_scalar::<_, UserId>(
        "INSERT INTO users (name, role) VALUES (?, ?) RETURNING id",
    )
    .bind(&name)
    .bind(&role)
    .fetch_one(pool)
    .await?;

    Ok(User::new(Some(id), name, role))
}

#[allow(unused)]
pub async fn get_users(pool: &SqlitePool, ids: Vec<UserId>) -> anyhow::Result<Vec<User>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut builder =
        QueryBuilder::<Sqlite>::new("SELECT u.id, u.name, u.role FROM Users u WHERE u.id in (");
    let mut separated = builder.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    Ok(builder
        .build_query_as()
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(id, name, role): (UserId, String, String)| {
            User::new(
                Some(id),
                name,
                role.try_into().unwrap_or_else(|_| UserRole::Parent),
            )
        })
        .collect())
}

pub async fn get_rooms(pool: &SqlitePool, _role: &UserRole) -> anyhow::Result<Vec<Room>> {
    Ok(
        sqlx::query_as::<_, (RoomId, String)>("SELECT id, name FROM Chatrooms ORDER BY id")
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|(id, name)| Room::new(Some(id), name))
            .collect(),
    )
}

pub async fn get_message_history(
    pool: &SqlitePool,
    room_id: RoomId,
) -> anyhow::Result<Vec<MessagePayload>> {
    let msgs = sqlx::query_as::<_, (String, UserRole, String, Timestamp)>(
        "SELECT u.name, u.role, m.content, m.timestamp
         FROM Messages m
         JOIN Users u ON u.id = m.user_id
         WHERE m.room_id = ?
         ORDER BY m.timestamp DESC
         LIMIT 100",
    )
    .bind(room_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(user_name, role, content, timestamp)| {
        MessagePayload::new(user_name, role, content, timestamp)
    })
    .collect();
    Ok(msgs)
}

pub async fn insert_message(
    pool: &SqlitePool,
    user_id: UserId,
    room_id: RoomId,
    content: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO messages (room_id, user_id, content) VALUES (?, ?, ?)")
        .bind(room_id)
        .bind(user_id)
        .bind(content)
        .execute(pool)
        .await?;
    Ok(())
}
