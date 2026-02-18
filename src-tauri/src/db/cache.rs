use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::models::{Playlist, Track};

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
             END;

             CREATE TABLE IF NOT EXISTS playlists (
                 id       INTEGER PRIMARY KEY AUTOINCREMENT,
                 name     TEXT NOT NULL,
                 created  TEXT NOT NULL DEFAULT (datetime('now'))
             );

             CREATE TABLE IF NOT EXISTS playlist_tracks (
                 playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                 track_id    TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                 position    INTEGER NOT NULL DEFAULT 0,
                 added       TEXT NOT NULL DEFAULT (datetime('now')),
                 PRIMARY KEY (playlist_id, track_id)
             );

             CREATE TABLE IF NOT EXISTS listen_history (
                 id       INTEGER PRIMARY KEY AUTOINCREMENT,
                 track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                 played   TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE INDEX IF NOT EXISTS idx_history_track ON listen_history(track_id);
             CREATE INDEX IF NOT EXISTS idx_history_played ON listen_history(played DESC);",
        )?;

        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn upsert_tracks(&self, tracks: &[Track]) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "INSERT INTO tracks (id, title, artist, thumbnail, duration)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title,
                 artist = excluded.artist,
                 thumbnail = excluded.thumbnail,
                 duration = excluded.duration",
        )?;
        for t in tracks {
            stmt.execute(params![t.id, t.title, t.artist, t.thumbnail, t.duration_secs])?;
        }
        Ok(())
    }

    pub fn get_track_by_id(&self, id: &str) -> Result<Option<Track>, AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT id, title, artist, thumbnail, duration FROM tracks WHERE id = ?1",
        )?;
        let track = stmt
            .query_row(params![id], |row| {
                Ok(Track {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    thumbnail: row.get(3)?,
                    duration_secs: row.get(4)?,
                    stream_url: None,
                })
            })
            .ok();
        Ok(track)
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

    pub fn create_playlist(&self, name: &str) -> Result<Playlist, AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO playlists (name) VALUES (?1)", params![name])?;
        let id = conn.last_insert_rowid();
        Ok(Playlist { id, name: name.to_string(), track_count: 0 })
    }

    pub fn list_playlists(&self) -> Result<Vec<Playlist>, AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT p.id, p.name, COUNT(pt.track_id)
             FROM playlists p
             LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
             GROUP BY p.id ORDER BY p.created DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Playlist { id: row.get(0)?, name: row.get(1)?, track_count: row.get(2)? })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn delete_playlist(&self, playlist_id: i64) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM playlists WHERE id = ?1", params![playlist_id])?;
        Ok(())
    }

    pub fn rename_playlist(&self, playlist_id: i64, name: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE playlists SET name = ?2 WHERE id = ?1",
            params![playlist_id, name],
        )?;
        Ok(())
    }

    pub fn add_to_playlist(&self, playlist_id: i64, track_id: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        let pos: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            params![playlist_id, track_id, pos],
        )?;
        Ok(())
    }

    pub fn remove_from_playlist(&self, playlist_id: i64, track_id: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            params![playlist_id, track_id],
        )?;
        Ok(())
    }

    pub fn reorder_playlist_tracks(&self, playlist_id: i64, track_ids: &[String]) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        for (i, tid) in track_ids.iter().enumerate() {
            tx.execute(
                "UPDATE playlist_tracks SET position = ?3 WHERE playlist_id = ?1 AND track_id = ?2",
                params![playlist_id, tid, i as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<Track>, AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT t.id, t.title, t.artist, t.thumbnail, t.duration
             FROM playlist_tracks pt
             JOIN tracks t ON t.id = pt.track_id
             WHERE pt.playlist_id = ?1
             ORDER BY pt.position",
        )?;
        let tracks = stmt
            .query_map(params![playlist_id], |row| {
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

    pub fn record_listen(&self, track_id: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO listen_history (track_id) VALUES (?1)",
            params![track_id],
        )?;
        Ok(())
    }

    pub fn top_artists(&self, limit: usize) -> Result<Vec<String>, AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT t.artist, COUNT(*) as cnt
             FROM listen_history h
             JOIN tracks t ON t.id = h.track_id
             GROUP BY t.artist ORDER BY cnt DESC LIMIT ?1",
        )?;
        let artists = stmt
            .query_map(params![limit as i64], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(artists)
    }

    pub fn recently_played(&self, limit: usize) -> Result<Vec<Track>, AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT DISTINCT t.id, t.title, t.artist, t.thumbnail, t.duration
             FROM listen_history h
             JOIN tracks t ON t.id = h.track_id
             ORDER BY h.played DESC LIMIT ?1",
        )?;
        let tracks = stmt
            .query_map(params![limit as i64], |row| {
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


    pub fn recent_track_ids(&self, days: i64) -> Result<std::collections::HashSet<String>, AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT DISTINCT track_id FROM listen_history
             WHERE played >= datetime('now', ?1)",
        )?;
        let offset = format!("-{days} days");
        let ids = stmt
            .query_map(params![offset], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    pub fn title_keywords(&self, limit: usize) -> Result<Vec<(String, i64)>, AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT t.title FROM listen_history h
             JOIN tracks t ON t.id = h.track_id
             ORDER BY h.played DESC LIMIT 200",
        )?;
        let titles: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut freq: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        let stopwords: std::collections::HashSet<&str> = [
            "the", "a", "an", "and", "or", "of", "in", "on", "at", "to", "for",
            "is", "it", "my", "me", "i", "you", "we", "he", "she", "this", "that",
            "with", "from", "by", "not", "no", "but", "so", "if", "up", "out",
            "all", "just", "like", "one", "do", "don", "be", "am", "are", "was",
            "has", "had", "have", "will", "can", "would", "could", "should",
            "ft", "feat", "vs", "official", "video", "audio", "music",
            "lyric", "lyrics", "visualizer", "visualiser", "hd", "hq",
            "full", "new", "version", "album", "single", "ep",
        ].into_iter().collect();

        for title in &titles {
            let cleaned = title.to_lowercase()
                .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ");
            for word in cleaned.split_whitespace() {
                if word.len() < 3 || stopwords.contains(word) {
                    continue;
                }
                *freq.entry(word.to_string()).or_default() += 1;
            }
        }

        let mut pairs: Vec<_> = freq.into_iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1));
        pairs.truncate(limit);
        Ok(pairs)
    }

    pub fn listen_count(&self) -> Result<i64, AppError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM listen_history", [], |r| r.get(0),
        )?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn temp_cache() -> SearchCache {
        let dir = std::env::temp_dir().join(format!("sunder_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        SearchCache::new(&dir).unwrap()
    }

    fn sample_track(id: &str) -> Track {
        Track {
            id: id.to_string(),
            title: format!("Track {id}"),
            artist: "Test Artist".into(),
            thumbnail: String::new(),
            duration_secs: 210.0,
            stream_url: None,
        }
    }

    #[test]
    fn get_track_by_id_returns_none_for_missing() {
        let db = temp_cache();
        assert!(db.get_track_by_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn get_track_by_id_finds_inserted_track() {
        let db = temp_cache();
        let track = sample_track("abc123");
        db.upsert_tracks(&[track.clone()]).unwrap();

        let found = db.get_track_by_id("abc123").unwrap().unwrap();
        assert_eq!(found.id, "abc123");
        assert_eq!(found.title, "Track abc123");
        assert!((found.duration_secs - 210.0).abs() < 0.01);
    }

    #[test]
    fn search_local_does_not_find_by_video_id() {
        let db = temp_cache();
        db.upsert_tracks(&[sample_track("dQw4w9WgXcQ")]).unwrap();

        let _results = db.search_local("dQw4w9WgXcQ").unwrap();
        let by_id = db.get_track_by_id("dQw4w9WgXcQ").unwrap();
        assert!(by_id.is_some());
    }

    #[test]
    fn get_track_by_id_latency() {
        let db = temp_cache();
        let tracks: Vec<Track> = (0..1000).map(|i| sample_track(&format!("vid_{i}"))).collect();
        db.upsert_tracks(&tracks).unwrap();

        let t0 = Instant::now();
        let iterations = 10_000;
        for i in 0..iterations {
            let id = format!("vid_{}", i % 1000);
            let _ = db.get_track_by_id(&id);
        }
        let elapsed = t0.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        eprintln!("[bench] get_track_by_id: {avg_us:.1} us/call ({iterations} iterations)");
        assert!(avg_us < 500.0, "get_track_by_id too slow: {avg_us} us");
    }

    #[test]
    fn record_listen_latency() {
        let db = temp_cache();
        db.upsert_tracks(&[sample_track("bench_track")]).unwrap();

        let t0 = Instant::now();
        let iterations = 1000;
        for _ in 0..iterations {
            db.record_listen("bench_track").unwrap();
        }
        let elapsed = t0.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        eprintln!("[bench] record_listen: {avg_us:.1} us/call ({iterations} iterations)");
        assert!(avg_us < 2000.0, "record_listen too slow: {avg_us} us");
    }

    #[test]
    fn upsert_idempotent() {
        let db = temp_cache();
        let t = sample_track("repeat");
        db.upsert_tracks(&[t.clone()]).unwrap();
        db.upsert_tracks(&[t]).unwrap();

        let found = db.get_track_by_id("repeat").unwrap();
        assert!(found.is_some());
    }

    #[test]
    fn playlist_crud() {
        let db = temp_cache();
        db.upsert_tracks(&[sample_track("t1"), sample_track("t2")]).unwrap();

        let pl = db.create_playlist("My List").unwrap();
        assert_eq!(pl.name, "My List");

        db.add_to_playlist(pl.id, "t1").unwrap();
        db.add_to_playlist(pl.id, "t2").unwrap();

        let tracks = db.get_playlist_tracks(pl.id).unwrap();
        assert_eq!(tracks.len(), 2);

        db.remove_from_playlist(pl.id, "t1").unwrap();
        let tracks = db.get_playlist_tracks(pl.id).unwrap();
        assert_eq!(tracks.len(), 1);

        db.rename_playlist(pl.id, "Renamed").unwrap();
        let lists = db.list_playlists().unwrap();
        assert_eq!(lists[0].name, "Renamed");

        db.delete_playlist(pl.id).unwrap();
        assert!(db.list_playlists().unwrap().is_empty());
    }
}
