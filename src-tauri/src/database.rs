use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

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
}

pub fn init_database(app: &AppHandle) -> Result<Database> {
    let app_data_dir = app.path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    std::fs::create_dir_all(&app_data_dir)
        .expect("Failed to create app data directory");

    let db_path = app_data_dir.join("flowstate.db");
    let conn = Connection::open(db_path)?;
    
    // Create tasks table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            completed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            completed_at TEXT
        )",
        [],
    )?;
    
    // Create index for faster queries
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tasks_completed ON tasks(completed)",
        [],
    )?;
    
    Ok(Database {
        conn: Mutex::new(conn),
    })
}

pub fn get_all_tasks(db: &Database) -> Result<Vec<Task>> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, text, completed, created_at, completed_at 
         FROM tasks 
         WHERE completed = 0 OR completed_at > datetime('now', '-7 days')
         ORDER BY completed ASC, created_at DESC"
    )?;
    
    let task_iter = stmt.query_map([], |row| {
        Ok(Task {
            id: row.get(0)?,
            text: row.get(1)?,
            completed: row.get::<_, i32>(2)? != 0,
            created_at: row.get(3)?,
            completed_at: row.get(4)?,
        })
    })?;
    
    let mut tasks = Vec::new();
    for task in task_iter {
        tasks.push(task?);
    }
    Ok(tasks)
}

pub fn add_task(db: &Database, text: &str) -> Result<Task> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO tasks (text, completed) VALUES (?1, 0)",
        params![text],
    )?;
    
    let id = conn.last_insert_rowid();
    let mut stmt = conn.prepare(
        "SELECT id, text, completed, created_at, completed_at FROM tasks WHERE id = ?1"
    )?;
    stmt.query_row(params![id], |row| {
        Ok(Task {
            id: row.get(0)?,
            text: row.get(1)?,
            completed: row.get::<_, i32>(2)? != 0,
            created_at: row.get(3)?,
            completed_at: row.get(4)?,
        })
    })
}

pub fn update_task(db: &Database, id: i64, text: &str) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "UPDATE tasks SET text = ?1 WHERE id = ?2",
        params![text, id],
    )?;
    Ok(())
}

pub fn delete_task(db: &Database, id: i64) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn toggle_task(db: &Database, id: i64) -> Result<Task> {
    let conn = db.conn.lock().unwrap();
    
    // Get current state
    let mut stmt = conn.prepare("SELECT completed FROM tasks WHERE id = ?1")?;
    let current: i32 = stmt.query_row(params![id], |row| row.get(0))?;
    let new_state = if current == 0 { 1 } else { 0 };
    
    // Update
    let completed_at: Option<String> = if new_state == 1 {
        Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
    } else {
        None
    };
    
    conn.execute(
        "UPDATE tasks SET completed = ?1, completed_at = ?2 WHERE id = ?3",
        params![new_state, completed_at, id],
    )?;
    
    // Return updated task
    let mut stmt = conn.prepare(
        "SELECT id, text, completed, created_at, completed_at FROM tasks WHERE id = ?1"
    )?;
    stmt.query_row(params![id], |row| {
        Ok(Task {
            id: row.get(0)?,
            text: row.get(1)?,
            completed: row.get::<_, i32>(2)? != 0,
            created_at: row.get(3)?,
            completed_at: row.get(4)?,
        })
    })
}

pub fn find_and_complete_task(db: &Database, text: &str) -> Result<Task> {
    let conn = db.conn.lock().unwrap();
    
    // Try to find matching task (fuzzy match)
    let search_pattern = format!("%{}%", text);
    let mut stmt = conn.prepare(
        "SELECT id, text, completed, created_at, completed_at 
         FROM tasks 
         WHERE text LIKE ?1 AND completed = 0 
         LIMIT 1"
    )?;
    
    if let Ok(task) = stmt.query_row(params![search_pattern], |row| {
        Ok(Task {
            id: row.get(0)?,
            text: row.get(1)?,
            completed: row.get::<_, i32>(2)? != 0,
            created_at: row.get(3)?,
            completed_at: row.get(4)?,
        })
    }) {
        // Mark as completed
        toggle_task(db, task.id)?;
        get_task_by_id(db, task.id)
    } else {
        // Create new completed task
        add_task(db, text)?;
        let new_task = get_all_tasks(db)?.first().unwrap().clone();
        toggle_task(db, new_task.id)?;
        get_task_by_id(db, new_task.id)
    }
}

fn get_task_by_id(db: &Database, id: i64) -> Result<Task> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, text, completed, created_at, completed_at FROM tasks WHERE id = ?1"
    )?;
    stmt.query_row(params![id], |row| {
        Ok(Task {
            id: row.get(0)?,
            text: row.get(1)?,
            completed: row.get::<_, i32>(2)? != 0,
            created_at: row.get(3)?,
            completed_at: row.get(4)?,
        })
    })
}

// Find and delete a task by fuzzy text matching
pub fn find_and_delete_task(db: &Database, search_text: &str) -> Result<Option<Task>> {
    let conn = db.conn.lock().unwrap();

    // Try to find matching task (fuzzy match using LIKE)
    let search_pattern = format!("%{}%", search_text.to_lowercase());
    let mut stmt = conn.prepare(
        "SELECT id, text, completed, created_at, completed_at
         FROM tasks
         WHERE LOWER(text) LIKE ?1
         ORDER BY
            CASE WHEN LOWER(text) = ?2 THEN 0 ELSE 1 END,
            completed ASC,
            created_at DESC
         LIMIT 1"
    )?;

    let search_exact = search_text.to_lowercase();
    if let Ok(task) = stmt.query_row(params![search_pattern, search_exact], |row| {
        Ok(Task {
            id: row.get(0)?,
            text: row.get(1)?,
            completed: row.get::<_, i32>(2)? != 0,
            created_at: row.get(3)?,
            completed_at: row.get(4)?,
        })
    }) {
        // Delete the task
        drop(stmt);
        conn.execute("DELETE FROM tasks WHERE id = ?1", params![task.id])?;
        Ok(Some(task))
    } else {
        Ok(None)
    }
}
