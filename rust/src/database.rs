use rusqlite::{Connection, Result, Row, params};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

pub struct Database {
    pub conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub text: String,
    pub completed: bool,
    pub created_at: String,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub priority: bool,
}

const TASK_COLUMNS: &str = "id, text, completed, created_at, completed_at, priority";

fn row_to_task(row: &Row) -> Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        text: row.get(1)?,
        completed: row.get::<_, i32>(2)? != 0,
        created_at: row.get(3)?,
        completed_at: row.get(4)?,
        priority: row.get::<_, i32>(5)? != 0,
    })
}

impl Database {
    /// Create the native app database.
    pub fn new() -> Result<Self> {
        let app_data_dir = crate::models::data_dir();

        std::fs::create_dir_all(&app_data_dir)
            .expect("Failed to create app data directory");

        let db_path = app_data_dir.join("flowstate.db");
        let conn = Connection::open(db_path)?;

        init_tables(&conn)?;

        Ok(Database {
            conn: Mutex::new(conn),
        })
    }
}

fn init_tables(conn: &Connection) -> Result<()> {
    // Create tasks table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            completed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            completed_at TEXT,
            priority INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;

    // Databases created before priorities existed: add the column.
    let has_priority = conn
        .prepare("SELECT 1 FROM pragma_table_info('tasks') WHERE name = 'priority'")?
        .exists([])?;
    if !has_priority {
        conn.execute("ALTER TABLE tasks ADD COLUMN priority INTEGER NOT NULL DEFAULT 0", [])?;
    }

    // Create index for faster queries
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tasks_completed ON tasks(completed)",
        [],
    )?;

    // Create settings table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;

    // Set default Ollama enabled to false (fast mode by default)
    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('ollama_enabled', 'false')",
        [],
    )?;

    Ok(())
}

fn get_task(conn: &Connection, id: i64) -> Result<Task> {
    conn.query_row(
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1"),
        params![id],
        row_to_task,
    )
}

/// Open tasks (high priority first, then newest) followed by tasks completed in the last 7 days.
pub fn get_all_tasks(db: &Database) -> Result<Vec<Task>> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare(&format!(
        "SELECT {TASK_COLUMNS}
         FROM tasks
         WHERE completed = 0 OR completed_at > datetime('now', '-7 days')
         ORDER BY completed ASC, priority DESC, created_at DESC"
    ))?;
    let tasks = stmt.query_map([], row_to_task)?.collect();
    tasks
}

pub fn add_task(db: &Database, text: &str) -> Result<Task> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO tasks (text, completed) VALUES (?1, 0)",
        params![text],
    )?;
    get_task(&conn, conn.last_insert_rowid())
}

pub fn delete_task(db: &Database, id: i64) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn toggle_task(db: &Database, id: i64) -> Result<Task> {
    let conn = db.conn.lock().unwrap();

    let current: i32 = conn.query_row("SELECT completed FROM tasks WHERE id = ?1", params![id], |row| row.get(0))?;
    let new_state = if current == 0 { 1 } else { 0 };
    let completed_at: Option<String> = if new_state == 1 {
        Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
    } else {
        None
    };

    conn.execute(
        "UPDATE tasks SET completed = ?1, completed_at = ?2 WHERE id = ?3",
        params![new_state, completed_at, id],
    )?;
    get_task(&conn, id)
}

pub fn set_priority(db: &Database, id: i64, priority: bool) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "UPDATE tasks SET priority = ?1 WHERE id = ?2",
        params![priority as i32, id],
    )?;
    Ok(())
}

pub fn find_and_complete_task(db: &Database, text: &str) -> Result<Task> {
    // Fuzzy match an open task; release the lock before toggling to avoid self-deadlock.
    let found: Option<i64> = {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT id FROM tasks WHERE completed = 0 AND (text LIKE ?1 OR ?2 LIKE '%' || text || '%')
             ORDER BY created_at DESC LIMIT 1",
            params![format!("%{}%", text), text],
            |row| row.get(0),
        )
        .ok()
    };

    let id = match found {
        Some(id) => id,
        None => add_task(db, text)?.id,
    };
    toggle_task(db, id)
}

pub fn update_task_text(db: &Database, id: i64, text: &str) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute("UPDATE tasks SET text = ?1 WHERE id = ?2", params![text, id])?;
    Ok(())
}

pub fn get_setting(db: &Database, key: &str) -> Option<String> {
    let conn = db.conn.lock().unwrap();
    conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0))
        .ok()
}

pub fn set_setting(db: &Database, key: &str, value: &str) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}
