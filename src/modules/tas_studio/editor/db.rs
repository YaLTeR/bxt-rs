use std::fmt;
use std::path::Path;

use bincode::Options;
use color_eyre::eyre::{self, ensure, eyre};
use hltas::HLTAS;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::operation::Operation;

#[derive(Debug)]
pub struct Db {
    conn: Connection,
}

#[derive(Clone)]
pub struct Branch {
    pub branch_id: i64,
    pub name: String,
    pub is_hidden: bool,

    pub script: HLTAS,
    pub stop_frame: u32,
}

impl fmt::Debug for Branch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Branch")
            .field("branch_id", &self.branch_id)
            .field("name", &self.name)
            .field("is_hidden", &self.is_hidden)
            .field("stop_frame", &self.stop_frame)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct GlobalSettings {
    pub current_branch_id: i64,
}

/// An action that applies to a branch and can be undone and redone.
#[derive(Debug, Clone)]
pub struct Action {
    /// Id of the branch this action applies to.
    pub branch_id: i64,
    /// Kind and data of the action.
    pub kind: ActionKind,
}

// This enum is stored in a SQLite DB as bincode bytes. All changes MUST BE BACKWARDS COMPATIBLE to
// be able to load old projects.
/// A kind of an action.
///
/// These kinds are stored in the database in the undo and redo logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionKind {
    /// Apply operation to a script.
    ApplyOperation(Operation),
    /// Hide a visible branch.
    Hide,
    /// Show a hidden branch.
    Show,
}

impl Db {
    /// Creates a new database at `path`, filling it with the `script`.
    #[instrument(skip(script))]
    pub fn create(path: &Path, script: &HLTAS) -> eyre::Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        Self::create_from_connection(conn, script)
    }

    /// Creates a new in-memory database, filling it with the `script`.
    #[instrument(skip(script))]
    pub fn create_in_memory(script: &HLTAS) -> eyre::Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::create_from_connection(conn, script)
    }

    /// Creates a new database from existing connection, filling it with the `script`.
    #[instrument(skip(script))]
    pub fn create_from_connection(mut conn: Connection, script: &HLTAS) -> eyre::Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;

        // Create the default tables.
        conn.execute(
            "CREATE TABLE branches (
                branch_id INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL DEFAULT \"Default Branch\",
                is_hidden INTEGER NOT NULL DEFAULT 0, 
                script TEXT NOT NULL,
                stop_frame INTEGER NOT NULL DEFAULT 0
            ) STRICT",
            (),
        )?;

        conn.execute(
            "CREATE TABLE undo_log (
                branch_id INTEGER NOT NULL,
                action BLOB NOT NULL,
                FOREIGN KEY(branch_id) REFERENCES branches(branch_id)
            ) STRICT",
            (),
        )?;

        conn.execute(
            "CREATE TABLE redo_log (
                branch_id INTEGER NOT NULL,
                action BLOB NOT NULL,
                FOREIGN KEY(branch_id) REFERENCES branches(branch_id)
            ) STRICT",
            (),
        )?;

        conn.execute(
            "CREATE TABLE global_settings (
                current_branch_id INTEGER NOT NULL,
                FOREIGN KEY(current_branch_id) REFERENCES branches(branch_id)
            ) STRICT",
            (),
        )?;

        // Add the default rows.
        let mut buffer = Vec::new();
        script
            .to_writer(&mut buffer)
            .expect("writing to an in-memory buffer should never fail");
        let buffer = String::from_utf8(buffer)
            .expect("HLTAS serialization should never produce invalid UTF-8");

        let tx = conn.transaction()?;
        tx.execute("INSERT INTO branches (script) VALUES (?1)", params![buffer])?;
        let branch_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO global_settings (current_branch_id) VALUES (?1)",
            params![branch_id],
        )?;
        tx.commit()?;

        Ok(Self { conn })
    }

    /// Opens an existing database.
    #[instrument]
    pub fn open(path: &Path) -> eyre::Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        Ok(Self { conn })
    }

    #[instrument]
    pub fn global_settings(&self) -> eyre::Result<GlobalSettings> {
        let rv =
            self.conn
                .query_row("SELECT current_branch_id FROM global_settings", [], |row| {
                    Ok(GlobalSettings {
                        current_branch_id: row.get(0)?,
                    })
                })?;

        Ok(rv)
    }

    #[instrument]
    pub fn branch(&self, branch_id: i64) -> eyre::Result<Branch> {
        let (buffer, name, is_hidden, stop_frame) = self.conn.query_row(
            "SELECT script, name, is_hidden, stop_frame FROM branches WHERE branch_id = ?1",
            [branch_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                ))
            },
        )?;

        let script = HLTAS::from_str(&buffer)
            .map_err(|err| eyre!("invalid script value, cannot parse: {err:?}"))?;

        Ok(Branch {
            branch_id,
            name,
            is_hidden,
            script,
            stop_frame,
        })
    }

    #[instrument]
    pub fn branches(&self) -> eyre::Result<Vec<Branch>> {
        let mut branches = vec![];

        let mut stmt = self
            .conn
            .prepare("SELECT branch_id, script, name, is_hidden, stop_frame FROM branches")?;
        for value in stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, String>(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })? {
            let (branch_id, buffer, name, is_hidden, stop_frame) = value?;

            let script = HLTAS::from_str(&buffer)
                .map_err(|err| eyre!("invalid script value, cannot parse: {err:?}"))?;

            branches.push(Branch {
                branch_id,
                name,
                is_hidden,
                script,
                stop_frame,
            })
        }
        stmt.finalize()?;

        Ok(branches)
    }

    #[instrument]
    pub fn last_undo_entry(&self) -> eyre::Result<Option<Action>> {
        let value = self
            .conn
            .query_row(
                "SELECT branch_id, action FROM undo_log ORDER BY _rowid_ DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()?;

        match value {
            Some((branch_id, buffer)) => {
                let kind = bincode::options()
                    .deserialize(&buffer)
                    .map_err(|err| eyre!("invalid action, cannot deserialize: {err:?}"))?;

                Ok(Some(Action { branch_id, kind }))
            }
            None => Ok(None),
        }
    }

    #[instrument]
    pub fn last_redo_entry(&self) -> eyre::Result<Option<Action>> {
        let value = self
            .conn
            .query_row(
                "SELECT branch_id, action FROM redo_log ORDER BY _rowid_ DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()?;

        match value {
            Some((branch_id, buffer)) => {
                let kind = bincode::options()
                    .deserialize(&buffer)
                    .map_err(|err| eyre!("invalid action, cannot deserialize: {err:?}"))?;

                Ok(Some(Action { branch_id, kind }))
            }
            None => Ok(None),
        }
    }

    #[instrument]
    pub fn undo_redo(&mut self) -> eyre::Result<(Vec<Action>, Vec<Action>)> {
        let tx = self.conn.transaction()?;

        let mut undo_log = Vec::new();
        let mut stmt = tx.prepare("SELECT branch_id, action FROM undo_log")?;
        for value in stmt.query_map([], |row| Ok((row.get(0)?, row.get::<_, Vec<u8>>(1)?)))? {
            let (branch_id, buffer) = value?;
            let kind = bincode::options()
                .deserialize(&buffer)
                .map_err(|err| eyre!("invalid action, cannot deserialize: {err:?}"))?;
            undo_log.push(Action { branch_id, kind });
        }
        stmt.finalize()?;

        let mut redo_log = Vec::new();
        let mut stmt = tx.prepare("SELECT branch_id, action FROM redo_log")?;
        for value in stmt.query_map([], |row| Ok((row.get(0)?, row.get::<_, Vec<u8>>(1)?)))? {
            let (branch_id, buffer) = value?;
            let kind = bincode::options()
                .deserialize(&buffer)
                .map_err(|err| eyre!("invalid action, cannot deserialize: {err:?}"))?;
            redo_log.push(Action { branch_id, kind });
        }
        stmt.finalize()?;

        tx.commit()?;

        Ok((undo_log, redo_log))
    }

    #[instrument]
    pub fn update_with_action(&mut self, branch: &Branch, kind: &ActionKind) -> eyre::Result<()> {
        let tx = self.conn.transaction()?;

        update_branch(&tx, branch)?;

        let buffer = bincode::options()
            .serialize(kind)
            .expect("serializing action should never fail");
        tx.execute(
            "INSERT INTO undo_log (branch_id, action) VALUES (?1, ?2)",
            params![branch.branch_id, buffer],
        )?;

        tx.execute("DELETE FROM redo_log", [])?;

        tx.commit()?;
        Ok(())
    }

    #[instrument]
    pub fn update_after_undo(&mut self, branch: &Branch, kind: &ActionKind) -> eyre::Result<()> {
        let tx = self.conn.transaction()?;

        update_branch(&tx, branch)?;

        let deleted = tx.execute(
            "DELETE FROM undo_log WHERE _rowid_ = (
                SELECT max(_rowid_) FROM undo_log WHERE branch_id = ?1
            )",
            [branch.branch_id],
        )?;
        ensure!(deleted == 1, "undo log should have had an entry");

        let buffer = bincode::options()
            .serialize(kind)
            .expect("serializing action should never fail");
        tx.execute(
            "INSERT INTO redo_log (branch_id, action) VALUES (?1, ?2)",
            params![branch.branch_id, buffer],
        )?;

        tx.commit()?;
        Ok(())
    }

    #[instrument]
    pub fn update_after_redo(&mut self, branch: &Branch, kind: &ActionKind) -> eyre::Result<()> {
        let tx = self.conn.transaction()?;

        update_branch(&tx, branch)?;

        let deleted = tx.execute(
            "DELETE FROM redo_log WHERE _rowid_ = (
                SELECT max(_rowid_) FROM redo_log WHERE branch_id = ?1
            )",
            [branch.branch_id],
        )?;
        ensure!(deleted == 1, "redo log should have had an entry");

        let buffer = bincode::options()
            .serialize(kind)
            .expect("serializing action should never fail");
        tx.execute(
            "INSERT INTO undo_log (branch_id, action) VALUES (?1, ?2)",
            params![branch.branch_id, buffer],
        )?;

        tx.commit()?;
        Ok(())
    }

    #[instrument]
    pub fn update_branch(&self, branch: &Branch) -> eyre::Result<()> {
        update_branch(&self.conn, branch)
    }

    #[instrument]
    pub fn insert_branch(&mut self, branch: &mut Branch) -> eyre::Result<()> {
        let tx = self.conn.transaction()?;

        let mut buffer = Vec::new();
        branch
            .script
            .to_writer(&mut buffer)
            .expect("writing to an in-memory buffer should never fail");
        let buffer = String::from_utf8(buffer)
            .expect("HLTAS serialization should never produce invalid UTF-8");

        tx.execute(
            "INSERT INTO branches (name, is_hidden, script, stop_frame) VALUES (?1, ?2, ?3, ?4)",
            params![&branch.name, branch.is_hidden, buffer, branch.stop_frame],
        )?;
        branch.branch_id = tx.last_insert_rowid();

        let kind = if branch.is_hidden {
            ActionKind::Hide
        } else {
            ActionKind::Show
        };
        let buffer = bincode::options()
            .serialize(&kind)
            .expect("serializing action should never fail");
        tx.execute(
            "INSERT INTO undo_log (branch_id, action) VALUES (?1, ?2)",
            params![branch.branch_id, buffer],
        )?;

        tx.execute("DELETE FROM redo_log", [])?;

        tx.commit()?;
        Ok(())
    }

    #[instrument]
    pub fn switch_to_branch(&mut self, branch: &Branch) -> eyre::Result<()> {
        self.conn.execute(
            "UPDATE global_settings SET current_branch_id = ?1",
            params![branch.branch_id],
        )?;

        Ok(())
    }

    #[instrument]
    pub fn hide_branch(&mut self, branch: &Branch) -> eyre::Result<()> {
        assert!(branch.is_hidden);

        let tx = self.conn.transaction()?;

        update_branch(&tx, branch)?;

        let buffer = bincode::options()
            .serialize(&ActionKind::Hide)
            .expect("serializing action should never fail");
        tx.execute(
            "INSERT INTO undo_log (branch_id, action) VALUES (?1, ?2)",
            params![branch.branch_id, buffer],
        )?;

        tx.execute("DELETE FROM redo_log", [])?;

        tx.commit()?;
        Ok(())
    }

    #[instrument]
    pub fn show_branch(&mut self, branch: &Branch) -> eyre::Result<()> {
        assert!(!branch.is_hidden);

        let tx = self.conn.transaction()?;

        update_branch(&tx, branch)?;

        let buffer = bincode::options()
            .serialize(&ActionKind::Show)
            .expect("serializing action should never fail");
        tx.execute(
            "INSERT INTO undo_log (branch_id, action) VALUES (?1, ?2)",
            params![branch.branch_id, buffer],
        )?;

        tx.execute("DELETE FROM redo_log", [])?;

        tx.commit()?;
        Ok(())
    }
}

fn update_branch(conn: &Connection, branch: &Branch) -> eyre::Result<()> {
    let mut buffer = Vec::new();
    branch
        .script
        .to_writer(&mut buffer)
        .expect("writing to an in-memory buffer should never fail");
    let buffer =
        String::from_utf8(buffer).expect("HLTAS serialization should never produce invalid UTF-8");

    conn.execute(
        "UPDATE branches SET
            name = ?1,
            is_hidden = ?2,
            script = ?3,
            stop_frame = ?4
        WHERE branch_id = ?5",
        params![
            &branch.name,
            branch.is_hidden,
            buffer,
            branch.stop_frame,
            branch.branch_id
        ],
    )?;

    Ok(())
}
