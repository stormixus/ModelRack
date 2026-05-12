//! SQLite persistence for per-library scan index and metadata.
//!
//! ## DB location (Option B)
//! We use **one database per configured library root** at
//! `{library_root}/.modelrack/modelrack.db`.
//!
//! Rationale (PR-style): `AppPrefs` already supports multiple `library_folders`, and
//! filesystem scans are naturally rooted per folder. Isolating each root into its own
//! DB avoids a global `library_root_id` join on every query, keeps backups portable with
//! the tree on removable drives, and matches the existing sidecar layout (metadata lives
//! beside models). A single app-wide DB remains a viable future option if cross-root
//! reporting becomes dominant.
//!
//! Sidecar JSON (`.modelrack.json`) remains the interchange / backup format; this module
//! is the primary structured index for search/sort fields populated from scans.
//!
//! ## Library JSON
//! The app does **not** load a monolithic library index JSON on startup (only
//! `prefs.json` for UI prefs and per-model sidecars during scan). Hydrating the grid from
//! SQLite instead of rescanning is left for a follow-up once query paths move to SQL.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::scanner::{self, StlFileInfo};
use crate::view_model::expand_user_pref_path;

/// Hidden directory inside each library root that holds `modelrack.db`.
pub fn modelrack_data_dir(library_root: &Path) -> PathBuf {
    library_root.join(".modelrack")
}

/// On-disk path for the SQLite database for `library_root`.
pub fn library_db_path(library_root: &Path) -> PathBuf {
    modelrack_data_dir(library_root).join("modelrack.db")
}

/// Picks the longest `library_folders` prefix that contains `model_path`, if any.
pub fn library_root_for_model_path(
    model_path: &Path,
    library_folders: &[PathBuf],
) -> Option<PathBuf> {
    let mut best: Option<(usize, PathBuf)> = None;
    for root in library_folders {
        let root = expand_user_pref_path(root);
        if model_path.starts_with(&root) {
            let len = root.as_os_str().len();
            if best.as_ref().map_or(true, |(n, _)| len > *n) {
                best = Some((len, root));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn system_time_secs(t: Option<SystemTime>) -> i64 {
    t.and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn scan_status_for_entry(entry: &StlFileInfo) -> &'static str {
    use scanner::StlType;
    match entry.stl_type {
        StlType::Unknown => "error",
        StlType::LargeStl => "large_stl",
        _ => "indexed",
    }
}

fn hash_partial_hex(hash: &[u8; 32]) -> String {
    const N: usize = 8;
    let mut s = String::with_capacity(N * 2);
    for b in hash.iter().take(N) {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn extension_for_db(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default()
}

/// Open (creating parent dirs and DB file as needed), enable FKs, run migrations.
pub fn open_library_db(library_root: &Path) -> Result<Connection> {
    let dir = modelrack_data_dir(library_root);
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = library_db_path(library_root);
    let conn = Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<()> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 1 {
        migrate_v1(conn)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    Ok(())
}

fn migrate_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            file_name TEXT NOT NULL,
            extension TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            added_at INTEGER NOT NULL,
            last_scanned_at INTEGER,
            scan_status TEXT NOT NULL DEFAULT 'pending',
            hash_partial TEXT,
            favorite INTEGER NOT NULL DEFAULT 0,
            printed_count INTEGER NOT NULL DEFAULT 0,
            triangle_count INTEGER,
            vertex_count INTEGER,
            dimension_x REAL, dimension_y REAL, dimension_z REAL,
            mesh_health TEXT,
            thumbnail_path TEXT,
            sidecar_path TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_files_file_name ON files(file_name);
        CREATE INDEX IF NOT EXISTS idx_files_extension ON files(extension);
        CREATE INDEX IF NOT EXISTS idx_files_modified_at ON files(modified_at);
        CREATE INDEX IF NOT EXISTS idx_files_favorite ON files(favorite);
        CREATE INDEX IF NOT EXISTS idx_files_scan_status ON files(scan_status);

        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS file_tags (
            file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (file_id, tag_id)
        );

        CREATE INDEX IF NOT EXISTS idx_file_tags_tag_id ON file_tags(tag_id);

        CREATE TABLE IF NOT EXISTS print_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            date TEXT NOT NULL,
            material TEXT NOT NULL DEFAULT '',
            printer TEXT NOT NULL DEFAULT '',
            profile TEXT NOT NULL DEFAULT '',
            nozzle TEXT NOT NULL DEFAULT '',
            layer_height TEXT NOT NULL DEFAULT '',
            duration TEXT NOT NULL DEFAULT '',
            success INTEGER NOT NULL DEFAULT 1,
            notes TEXT NOT NULL DEFAULT ''
        );

        CREATE INDEX IF NOT EXISTS idx_print_history_file_id ON print_history(file_id);
        "#,
    )?;
    Ok(())
}

/// Upsert scan rows for `library_root` (opens DB per call — fine for batch sizes in UI).
pub fn upsert_entries_for_library(library_root: &Path, entries: &[StlFileInfo]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let mut conn = open_library_db(library_root)?;
    let tx = conn.transaction()?;
    for entry in entries {
        upsert_one_file(&tx, entry)?;
    }
    tx.commit()?;
    Ok(())
}

fn upsert_one_file(tx: &rusqlite::Transaction<'_>, entry: &StlFileInfo) -> Result<()> {
    let path_str = entry.path.to_string_lossy();
    let file_name = entry.filename.clone();
    let extension = extension_for_db(&entry.path);
    let size_bytes = entry.size as i64;
    let modified_at = system_time_secs(entry.modified);
    let now = now_unix_secs();
    let last_scanned_at = Some(now);
    let scan_status = scan_status_for_entry(entry);
    let hash_partial = hash_partial_hex(&entry.hash);
    let favorite = if entry.meta.as_ref().is_some_and(|m| m.favorite) {
        1i32
    } else {
        0i32
    };
    let printed_count = entry.meta.as_ref().map(|m| m.printed as i64).unwrap_or(0);
    let triangle_count = entry.triangle_count.map(|n| n as i64).filter(|&n| n >= 0);
    let (dx, dy, dz) = entry
        .dimensions
        .map(|d| (Some(d[0] as f64), Some(d[1] as f64), Some(d[2] as f64)))
        .unwrap_or((None, None, None));
    let thumbnail_path = entry
        .thumbnail_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    let sidecar_path = scanner::sidecar_path(&entry.path);
    let sidecar_str = sidecar_path.to_string_lossy();

    let id: i64 = tx.query_row(
        r#"
        INSERT INTO files (
            path, file_name, extension, size_bytes, modified_at, added_at, last_scanned_at,
            scan_status, hash_partial, favorite, printed_count, triangle_count, vertex_count,
            dimension_x, dimension_y, dimension_z, mesh_health, thumbnail_path, sidecar_path
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, ?13, ?14, ?15, NULL, ?16, ?17)
        ON CONFLICT(path) DO UPDATE SET
            file_name = excluded.file_name,
            extension = excluded.extension,
            size_bytes = excluded.size_bytes,
            modified_at = excluded.modified_at,
            added_at = files.added_at,
            last_scanned_at = excluded.last_scanned_at,
            scan_status = excluded.scan_status,
            hash_partial = excluded.hash_partial,
            favorite = excluded.favorite,
            printed_count = excluded.printed_count,
            triangle_count = excluded.triangle_count,
            dimension_x = excluded.dimension_x,
            dimension_y = excluded.dimension_y,
            dimension_z = excluded.dimension_z,
            thumbnail_path = excluded.thumbnail_path,
            sidecar_path = excluded.sidecar_path
        RETURNING id
        "#,
        params![
            path_str,
            file_name,
            extension,
            size_bytes,
            modified_at,
            now,
            last_scanned_at,
            scan_status,
            hash_partial,
            favorite,
            printed_count,
            triangle_count,
            dx,
            dy,
            dz,
            thumbnail_path,
            sidecar_str,
        ],
        |row| row.get(0),
    )?;

    tx.execute("DELETE FROM file_tags WHERE file_id = ?1", params![id])?;
    tx.execute("DELETE FROM print_history WHERE file_id = ?1", params![id])?;

    if let Some(meta) = &entry.meta {
        for tag_name in &meta.tags {
            let tag = tag_name.trim();
            if tag.is_empty() {
                continue;
            }
            tx.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", [tag])?;
            let tag_id: i64 =
                tx.query_row("SELECT id FROM tags WHERE name = ?1", [tag], |row| {
                    row.get(0)
                })?;
            tx.execute(
                "INSERT OR IGNORE INTO file_tags (file_id, tag_id) VALUES (?1, ?2)",
                params![id, tag_id],
            )?;
        }

        for record in &meta.print_history {
            tx.execute(
                r#"INSERT INTO print_history (
                    file_id, date, material, printer, profile, nozzle, layer_height,
                    duration, success, notes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                params![
                    id,
                    record.date,
                    record.material,
                    record.printer,
                    record.profile,
                    record.nozzle,
                    record.layer_height,
                    record.duration,
                    if record.success { 1i32 } else { 0i32 },
                    record.notes,
                ],
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{SidecarMeta, StlType};
    use std::fs;

    fn sample_entry(path: PathBuf) -> StlFileInfo {
        StlFileInfo {
            path: path.clone(),
            filename: path.file_name().unwrap().to_string_lossy().into_owned(),
            size: 42,
            hash: [7u8; 32],
            stl_type: StlType::Binary,
            triangle_count: Some(12),
            dimensions: Some([1.0, 2.0, 3.0]),
            three_mf_plate_count: None,
            modified: Some(UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)),
            thumbnail_path: None,
            meta: Some(SidecarMeta {
                tags: vec!["a".into(), "b".into()],
                notes: String::new(),
                favorite: true,
                printed: 3,
                print_history: vec![scanner::PrintRecord {
                    date: "2024-01-01".into(),
                    material: "PLA".into(),
                    printer: "p".into(),
                    profile: "prof".into(),
                    nozzle: "0.4".into(),
                    layer_height: "0.2".into(),
                    duration: "1h".into(),
                    success: true,
                    notes: String::new(),
                }],
                author: String::new(),
                added: None,
                title: String::new(),
            }),
        }
    }

    #[test]
    fn migration_applies_and_user_version_set() {
        let tmp = std::env::temp_dir().join(format!("modelrack-db-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let conn = open_library_db(&tmp).unwrap();
        let v: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(v, 1);
        let _: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='files'",
                [],
                |row| row.get(0),
            )
            .unwrap();
    }

    #[test]
    fn upsert_idempotent_unique_path() {
        let tmp = std::env::temp_dir().join(format!("modelrack-db-upsert-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp.join("models")).unwrap();
        let stl = tmp.join("models").join("part.stl");
        fs::write(&stl, b"x").unwrap();
        let e = sample_entry(stl);

        upsert_entries_for_library(&tmp, &[e.clone()]).unwrap();
        upsert_entries_for_library(&tmp, &[e]).unwrap();

        let conn = open_library_db(&tmp).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn foreign_keys_enforced() {
        let tmp = std::env::temp_dir().join(format!("modelrack-db-fk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let conn = open_library_db(&tmp).unwrap();
        let err = conn.execute(
            "INSERT INTO file_tags (file_id, tag_id) VALUES (99999, 99999)",
            [],
        );
        assert!(err.is_err());
    }

    #[test]
    fn library_root_for_model_path_prefers_longest_prefix() {
        let a = PathBuf::from("/data/lib");
        let b = PathBuf::from("/data/lib/sub");
        let roots = vec![a.clone(), b.clone()];
        let p = PathBuf::from("/data/lib/sub/x.stl");
        assert_eq!(library_root_for_model_path(&p, &roots), Some(b));
    }

    #[test]
    fn library_root_for_model_path_expands_tilde_prefix() {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return;
        };
        let rel = format!(".modelrack-db-tilde-{}", std::process::id());
        let dir = home.join(&rel);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let stl = dir.join("m.stl");
        std::fs::write(&stl, b"x").unwrap();
        let tilde_root = PathBuf::from(format!("~/{}", rel));
        assert_eq!(
            library_root_for_model_path(&stl, &[tilde_root]),
            Some(dir.clone())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
