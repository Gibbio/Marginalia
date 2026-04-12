use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_baseline",
        include_str!("../migrations/001_baseline.sql"),
    ),
    (
        "002_active_session_flag",
        include_str!("../migrations/002_active_session_flag.sql"),
    ),
];

#[derive(Debug, Clone)]
pub struct SQLiteDatabase {
    connection: Arc<Mutex<Connection>>,
}

impl SQLiteDatabase {
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let connection = Connection::open_in_memory()?;
        let database = Self {
            connection: Arc::new(Mutex::new(connection)),
        };
        database.initialize()?;
        Ok(database)
    }

    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let connection = Connection::open(path)?;
        let database = Self {
            connection: Arc::new(Mutex::new(connection)),
        };
        database.initialize()?;
        Ok(database)
    }

    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.connection)
    }

    fn initialize(&self) -> rusqlite::Result<()> {
        let connection = self
            .connection
            .lock()
            .expect("sqlite connection lock poisoned");

        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "busy_timeout", 5000)?;
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_migrations (
                migration_id TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL
            );
            ",
        )?;

        let applied = {
            let mut statement = connection
                .prepare("SELECT migration_id FROM schema_migrations ORDER BY migration_id")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            let mut applied = Vec::new();
            for row in rows {
                applied.push(row?);
            }
            applied
        };

        for (migration_id, sql) in MIGRATIONS {
            if applied.iter().any(|existing| existing == migration_id) {
                continue;
            }

            connection.execute_batch(sql)?;
            connection.execute(
                "INSERT INTO schema_migrations(migration_id, applied_at) VALUES(?, ?)",
                rusqlite::params![migration_id, chrono::Utc::now().to_rfc3339()],
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SQLiteDatabase;

    #[test]
    fn open_in_memory_applies_migrations() {
        let database = SQLiteDatabase::open_in_memory().unwrap();
        let connection = database.connection();
        let connection = connection.lock().unwrap();

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();

        assert_eq!(count, 2);
    }
}
