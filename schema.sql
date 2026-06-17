CREATE TABLE
IF NOT EXISTS Users
(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    role TEXT NOT NULL CHECK
(role IN
('parent','teacher'))
);

CREATE TABLE
IF NOT EXISTS Chatrooms
(
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE
IF NOT EXISTS Messages
(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id INTEGER NOT NULL,
    user_id INTEGER,
    content TEXT NOT NULL,
    timestamp INTEGER DEFAULT
(unixepoch
('subsec') * 1000), -- SELECT datetime(timestamp / 1000.0, 'unixepoch') AS readable_time
    FOREIGN KEY
(user_id) REFERENCES users
(id) ON
DELETE
SET NULL
,
    FOREIGN KEY
(room_id) REFERENCES chatrooms
(id) ON
DELETE CASCADE
);