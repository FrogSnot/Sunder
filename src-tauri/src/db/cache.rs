use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::models::Track;

pub struct SearchCache {
    conn: Mutex<Connection>,
}

impl SearchCache {
    pub fn new(data_dir: &Path) -> Result<Self, AppError> {
        std::fs::create_dir_all(data_dir)?;
        let db_path = data_dir.join("sunder.db");
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;

             CREATE TABLE IF NOT EXISTS tracks (
                 id       TEXT PRIMARY KEY,
                 title    TEXT NOT NULL,
                 artist   TEXT NOT NULL,
                 thumbnail TEXT NOT NULL DEFAULT '',
                 duration REAL NOT NULL DEFAULT 0
             );

             CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
                 title, artist,
                 content='tracks',
                 content_rowid='rowid'
             );

             -- Keep FTS in sync via triggers
             CREATE TRIGGER IF NOT EXISTS tracks_ai AFTER INSERT ON tracks BEGIN
                 INSERT INTO tracks_fts(rowid, title, artist)
                 VALUES (new.rowid, new.title, new.artist);
             END;
             CREATE TRIGGER IF NOT EXISTS tracks_ad AFTER DELETE ON tracks BEGIN
                 INSERT INTO tracks_fts(tracks_fts, rowid, title, artist)
                 VALUES ('delete', old.rowid, old.title, old.artist);
             END;
             CREATE TRIGGER IF NOT EXISTS tracks_au AFTER UPDATE ON tracks BEGIN
                 INSERT INTO tracks_fts(tracks_fts, rowid, title, artist)
                 VALUES ('delete', old.rowid, old.title, old.artist);
                 INSERT INTO tracks_fts(rowid, title, artist)
                 VALUES (new.rowid, new.title, new.artist);
             END;",
        )?;

        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn upsert_tracks(&self, tracks: &[Track]) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "INSERT OR REPLACE INTO tracks (id, title, artist, thumbnail, duration)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for t in tracks {
            stmt.execute(params![t.id, t.title, t.artist, t.thumbnail, t.duration_secs])?;
        }
        Ok(())
    }

    pub fn search_local(&self, query: &str) -> Result<Vec<Track>, AppError> {
        if query.trim().is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn.lock().unwrap();
        let fts_query = query
            .split_whitespace()
            .map(|w| format!("{w}*"))
            .collect::<Vec<_>>()
            .join(" ");

        let mut stmt = conn.prepare_cached(
            "SELECT t.id, t.title, t.artist, t.thumbnail, t.duration
             FROM tracks_fts f
             JOIN tracks t ON t.rowid = f.rowid
             WHERE tracks_fts MATCH ?1
             ORDER BY rank
             LIMIT 20",
        )?;

        let tracks = stmt
            .query_map(params![fts_query], |row| {
                Ok(Track {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    thumbnail: row.get(3)?,
                    duration_secs: row.get(4)?,
                    stream_url: None,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(tracks)
    }
}
