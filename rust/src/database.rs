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

/// Marks a task as changed locally so the next sync uploads it.
const BUMP: &str = "dirty = dirty + 1, modified_at = datetime('now')";

impl Database {
    /// Create the native app database.
    pub fn new() -> Result<Self> {
        let app_data_dir = crate::models::data_dir();

        std::fs::create_dir_all(&app_data_dir)
            .expect("Failed to create app data directory");

        let db_path = app_data_dir.join("flowstate.db");
        Self::from_connection(Connection::open(db_path)?)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        init_tables(&conn)?;
        Ok(Database {
            conn: Mutex::new(conn),
        })
    }
}

fn add_column_if_missing(conn: &Connection, column: &str, definition: &str) -> Result<()> {
    let exists = conn
        .prepare("SELECT 1 FROM pragma_table_info('tasks') WHERE name = ?1")?
        .exists([column])?;
    if !exists {
        conn.execute(&format!("ALTER TABLE tasks ADD COLUMN {column} {definition}"), [])?;
    }
    Ok(())
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

    // Columns added after the first release.
    add_column_if_missing(conn, "priority", "INTEGER NOT NULL DEFAULT 0")?;
    // Sync with a CalDAV task list: remote identity, last seen version, and the original
    // iCalendar text so fields FlowState doesn't know about survive round trips.
    add_column_if_missing(conn, "uid", "TEXT")?;
    add_column_if_missing(conn, "href", "TEXT")?;
    add_column_if_missing(conn, "etag", "TEXT")?;
    add_column_if_missing(conn, "ical", "TEXT")?;
    // Number of local edits not yet uploaded.
    add_column_if_missing(conn, "dirty", "INTEGER NOT NULL DEFAULT 0")?;
    add_column_if_missing(conn, "modified_at", "TEXT")?;

    // Synced tasks deleted locally, still to be deleted on the server.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_tombstones (
            href TEXT PRIMARY KEY,
            etag TEXT
        )",
        [],
    )?;

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
        "INSERT INTO tasks (text, completed, dirty, modified_at) VALUES (?1, 0, 1, datetime('now'))",
        params![text],
    )?;
    get_task(&conn, conn.last_insert_rowid())
}

pub fn delete_task(db: &Database, id: i64) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    // Remember synced tasks so the next sync deletes them on the server too.
    conn.execute(
        "INSERT OR REPLACE INTO sync_tombstones (href, etag)
         SELECT href, etag FROM tasks WHERE id = ?1 AND href IS NOT NULL",
        params![id],
    )?;
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
        &format!("UPDATE tasks SET completed = ?1, completed_at = ?2, {BUMP} WHERE id = ?3"),
        params![new_state, completed_at, id],
    )?;
    get_task(&conn, id)
}

pub fn set_priority(db: &Database, id: i64, priority: bool) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        &format!("UPDATE tasks SET priority = ?1, {BUMP} WHERE id = ?2"),
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
    conn.execute(
        &format!("UPDATE tasks SET text = ?1, {BUMP} WHERE id = ?2"),
        params![text, id],
    )?;
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

// ------------------------------------------------------------------------ sync support

/// A task with its sync bookkeeping.
#[derive(Debug, Clone)]
pub struct SyncRow {
    pub id: i64,
    pub text: String,
    pub completed: bool,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub priority: bool,
    pub uid: Option<String>,
    pub href: Option<String>,
    pub etag: Option<String>,
    pub ical: Option<String>,
    pub dirty: i64,
}

/// Task fields as they arrive from the server.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteFields {
    pub uid: String,
    pub text: String,
    pub completed: bool,
    pub completed_at: Option<String>,
    pub created_at: Option<String>,
    pub priority: bool,
}

pub fn sync_rows(db: &Database) -> Result<Vec<SyncRow>> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, text, completed, completed_at, created_at, priority, uid, href, etag, ical, dirty FROM tasks",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SyncRow {
                id: r.get(0)?,
                text: r.get(1)?,
                completed: r.get::<_, i32>(2)? != 0,
                completed_at: r.get(3)?,
                created_at: r.get(4)?,
                priority: r.get::<_, i32>(5)? != 0,
                uid: r.get(6)?,
                href: r.get(7)?,
                etag: r.get(8)?,
                ical: r.get(9)?,
                dirty: r.get(10)?,
            })
        })?
        .collect();
    rows
}

pub fn insert_from_remote(db: &Database, f: &RemoteFields, href: &str, etag: &str, ical: &str) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO tasks (text, completed, completed_at, created_at, priority, uid, href, etag, ical, dirty, modified_at)
         VALUES (?1, ?2, ?3, COALESCE(?4, datetime('now')), ?5, ?6, ?7, ?8, ?9, 0, datetime('now'))",
        params![f.text, f.completed as i32, f.completed_at, f.created_at, f.priority as i32, f.uid, href, etag, ical],
    )?;
    Ok(())
}

/// Apply a server-side change, unless the task was edited locally meanwhile.
pub fn update_from_remote(db: &Database, id: i64, f: &RemoteFields, href: &str, etag: &str, ical: &str) -> Result<bool> {
    let conn = db.conn.lock().unwrap();
    let n = conn.execute(
        "UPDATE tasks SET text = ?2, completed = ?3, completed_at = ?4, priority = ?5,
                href = ?6, etag = ?7, ical = ?8
         WHERE id = ?1 AND dirty = 0",
        params![id, f.text, f.completed as i32, f.completed_at, f.priority as i32, href, etag, ical],
    )?;
    Ok(n > 0)
}

/// Record a successful upload. Edits made while uploading stay marked for the next sync.
pub fn mark_pushed(
    db: &Database,
    id: i64,
    seen_dirty: i64,
    uid: &str,
    href: &str,
    etag: Option<&str>,
    ical: &str,
) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "UPDATE tasks SET uid = ?3, href = ?4, etag = ?5, ical = ?6,
                dirty = CASE WHEN dirty = ?2 THEN 0 ELSE dirty END
         WHERE id = ?1",
        params![id, seen_dirty, uid, href, etag, ical],
    )?;
    Ok(())
}

/// Remove a task that was deleted on the server (kept if it was edited locally meanwhile).
pub fn delete_from_remote(db: &Database, id: i64) -> Result<bool> {
    let conn = db.conn.lock().unwrap();
    Ok(conn.execute("DELETE FROM tasks WHERE id = ?1 AND dirty = 0", params![id])? > 0)
}

pub fn tombstones(db: &Database) -> Result<Vec<(String, Option<String>)>> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT href, etag FROM sync_tombstones")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect();
    rows
}

pub fn remove_tombstone(db: &Database, href: &str) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute("DELETE FROM sync_tombstones WHERE href = ?1", params![href])?;
    Ok(())
}

/// Forget all links to the server (used when switching to a different account or list).
/// Open tasks will be uploaded again as new items.
pub fn reset_sync_state(db: &Database) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "UPDATE tasks SET uid = NULL, href = NULL, etag = NULL, ical = NULL, dirty = 1",
        [],
    )?;
    conn.execute("DELETE FROM sync_tombstones", [])?;
    Ok(())
}

#[cfg(test)]
pub fn find_by_text(db: &Database, text: &str) -> Option<SyncRow> {
    use rusqlite::OptionalExtension;
    let id: Option<i64> = {
        let conn = db.conn.lock().unwrap();
        conn.query_row("SELECT id FROM tasks WHERE text = ?1", params![text], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    };
    let id = id?;
    sync_rows(db).ok()?.into_iter().find(|r| r.id == id)
}
