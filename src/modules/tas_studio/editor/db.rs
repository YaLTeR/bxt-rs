use std::ffi::CStr;
use std::path::Path;
use std::{collections::HashSet, fmt};

use bincode::Options;
use color_eyre::eyre::{self, ensure, eyre};
use hltas::{types::Line, HLTAS};
use itertools::{Itertools, MultiPeek};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::hooks::engine;
use crate::utils::MainThreadMarker;

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
    pub splits: Vec<SplitInfo>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitInfo {
    // this is 1 index right after the split marker
    // it is guaranteed to have a framebulk somewhere after this
    pub start_idx: usize,
    // for the sake of easier searching, the last framebulk index is stored
    pub bulk_idx: usize,
    // a split could have no name
    // this could also be duplicate names, which becomes `None`
    // TODO: could probably get away with &str
    pub name: Option<String>,
    pub split_type: SplitType,
    // ready as in, there's a save created, and lines before and including start_idx is still unchanged
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitType {
    Comment,
    Reset,
    Save,
}

impl SplitInfo {
    #[must_use]
    pub fn split_lines<'a, T: Iterator<Item = &'a Line>>(lines: T) -> Vec<Self> {
        Self::split_lines_with_stop(lines, usize::MAX)
    }

    // TODO: test stop_idx
    #[must_use]
    pub fn split_lines_with_stop<'a, T: Iterator<Item = &'a Line>>(
        lines: T,
        stop_idx: usize,
    ) -> Vec<Self> {
        let mut lines = lines.into_iter().multipeek();

        if lines.peek().is_none() {
            return Vec::new();
        }

        let mut splits = Vec::new();

        let mut line_idx = 0usize;
        let mut bulk_idx = 1usize;
        // skip till there's at least 1 framebulk
        for line in lines.by_ref() {
            if matches!(line, Line::FrameBulk(_)) {
                break;
            }

            line_idx += 1;
            if line_idx >= stop_idx {
                return splits;
            }
        }

        let mut used_save_names = HashSet::new();

        while let Some(line) = lines.next() {
            // this is correct, if FrameBulk is at index 0, we are searching from index 1
            line_idx += 1;
            if line_idx >= stop_idx {
                return splits;
            }

            const SPLIT_MARKER: &str = "bxt-rs-split";

            let name;
            let start_idx;
            let split_type;

            match line {
                // TODO: save name;load name console
                // TODO: handle setting shared rng, and what property lines do i bring over?
                // TODO: handle completely invalid back to back splits
                Line::Save(save_name) => {
                    if Self::no_framebulks_left(&mut lines) {
                        break;
                    }
                    line_idx += 1;

                    name = Some(save_name.as_str());
                    start_idx = line_idx;
                    split_type = SplitType::Save;
                }
                // this reset doesn't have a name, one with comment attached is handled below
                Line::Reset { .. } => {
                    if Self::no_framebulks_left(&mut lines) {
                        break;
                    }
                    line_idx += 1;

                    name = None;
                    start_idx = line_idx;
                    split_type = SplitType::Reset;
                }
                Line::Comment(comment) => {
                    let comment = comment.trim();

                    if !comment.starts_with(SPLIT_MARKER) {
                        continue;
                    }

                    let comment = &comment[SPLIT_MARKER.len()..];

                    if !comment.is_empty() && !comment.chars().next().unwrap().is_whitespace() {
                        continue;
                    }

                    // linked to reset?
                    split_type = if matches!(lines.peek(), Some(Line::Reset { .. })) {
                        lines.next(); // consume reset
                        if Self::no_framebulks_left(&mut lines) {
                            break;
                        }
                        line_idx += 1;

                        SplitType::Reset
                    } else {
                        if Self::no_framebulks_left(&mut lines) {
                            break;
                        }

                        SplitType::Comment
                    };
                    line_idx += 1;

                    start_idx = line_idx;
                    let comment = comment.trim_start();
                    if comment.is_empty() {
                        name = None;
                    } else {
                        name = Some(comment);
                    }
                }
                Line::FrameBulk(_) => {
                    bulk_idx += 1;
                    continue;
                }
                _ => continue,
            }

            let name = if let Some(name) = name {
                if used_save_names.contains(name) {
                    None
                } else {
                    used_save_names.insert(name.to_owned());
                    Some(name.to_owned())
                }
            } else {
                None
            };

            splits.push(SplitInfo {
                start_idx,
                name,
                split_type,
                bulk_idx,
                ready: false,
            });
        }

        splits
    }

    fn no_framebulks_left<'a, T: Iterator<Item = &'a Line>>(lines: &mut MultiPeek<T>) -> bool {
        while let Some(line) = lines.peek() {
            if matches!(line, Line::FrameBulk(_)) {
                return false;
            }
        }
        true
    }

    pub fn validate_all_by_saves(splits: &mut Vec<SplitInfo>, marker: MainThreadMarker) {
        for split in splits {
            let Some(name) = &split.name else {
                return;
            };

            let game_dir = Path::new(
                unsafe { CStr::from_ptr(engine::com_gamedir.get(marker).cast()) }
                    .to_str()
                    .unwrap(),
            );
            let save_path = game_dir.join("SAVE").join(format!("{name}.sav"));
            split.ready = save_path.is_file();
        }
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

        let mut splits = SplitInfo::split_lines(script.lines.iter());
        // TODO: is this fine? not sure how else to get MainThreadMarker for game directory
        unsafe {
            SplitInfo::validate_all_by_saves(&mut splits, MainThreadMarker::new());
        }

        Ok(Branch {
            branch_id,
            name,
            is_hidden,
            script,
            splits,
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

            let mut splits = SplitInfo::split_lines(script.lines.iter());
            // TODO: is this fine? not sure how else to get MainThreadMarker for game directory
            unsafe {
                SplitInfo::validate_all_by_saves(&mut splits, MainThreadMarker::new());
            }

            branches.push(Branch {
                branch_id,
                name,
                is_hidden,
                script,
                stop_frame,
                splits,
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

#[cfg(test)]
mod tests {
    use hltas::HLTAS;

    use crate::modules::tas_studio::editor::db::{SplitInfo, SplitType};

    #[test]
    fn split_by_markers() {
        // TODO: complete, duplicate names
        let script = HLTAS::from_str(
            "version 1\nframes\n\
                ----------|------|------|0.001|-|-|10\n\
                // bxt-rs-split name\n\
                ----------|------|------|0.002|-|-|10\n\
                ----------|------|------|0.002|-|-|10\n\
                // bxt-rs-split\n\
                ----------|------|------|0.003|-|-|10\n\
                ----------|------|------|0.003|-|-|10\n\
                ----------|------|------|0.003|-|-|10\n\
                save name2
                ----------|------|------|0.004|-|-|10\n\
                ----------|------|------|0.004|-|-|10\n\
                ----------|------|------|0.004|-|-|10\n\
                ----------|------|------|0.004|-|-|10\n\
                reset 0
                ----------|------|------|0.005|-|-|10\n\
                ----------|------|------|0.005|-|-|10\n\
                ----------|------|------|0.005|-|-|10\n\
                ----------|------|------|0.005|-|-|10\n\
                ----------|------|------|0.005|-|-|10\n\
                // bxt-rs-split name4
                reset 1
                ----------|------|------|0.006|-|-|10\n\
                ----------|------|------|0.006|-|-|10\n\
                ----------|------|------|0.006|-|-|10\n\
                ----------|------|------|0.006|-|-|10\n\
                ----------|------|------|0.006|-|-|10\n\
                ----------|------|------|0.006|-|-|10\n",
        )
        .unwrap();

        let splits = SplitInfo::split_lines(script.lines.iter());
        let expected = vec![
            SplitInfo {
                start_idx: 2,
                bulk_idx: 1,
                name: Some("name".to_string()),
                split_type: SplitType::Comment,
                ready: false,
            },
            SplitInfo {
                start_idx: 5,
                bulk_idx: 3,
                name: None,
                split_type: SplitType::Comment,
                ready: false,
            },
            SplitInfo {
                start_idx: 9,
                bulk_idx: 6,
                name: Some("name2".to_string()),
                split_type: SplitType::Save,
                ready: false,
            },
            SplitInfo {
                start_idx: 14,
                bulk_idx: 10,
                name: Some("name3".to_string()),
                split_type: SplitType::Reset,
                ready: false,
            },
            SplitInfo {
                start_idx: 21,
                bulk_idx: 15,
                name: Some("name4".to_string()),
                split_type: SplitType::Comment,
                ready: false,
            },
        ];

        assert_eq!(splits, expected);
    }
}
