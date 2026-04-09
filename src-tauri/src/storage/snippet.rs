use rusqlite::Connection;

use super::models::{Snippet, SnippetFolder, SnippetFolderInput, SnippetInput};

// ============================================================
// Snippet CRUD
// ============================================================

/// Lists snippets, optionally filtered by folder and/or search query.
/// Sorted by: is_favorite DESC, usage_count DESC, updated_at DESC.
pub fn list(
    conn: &Connection,
    folder_id: Option<&str>,
    search: Option<&str>,
) -> Result<Vec<Snippet>, String> {
    let mut sql = String::from(
        "SELECT id, title, description, command, tags, folder_id, is_favorite,
                usage_count, last_used_at, created_at, updated_at
         FROM snippets WHERE 1=1",
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(fid) = folder_id {
        sql.push_str(" AND folder_id = ?");
        params.push(Box::new(fid.to_string()));
    }

    if let Some(q) = search {
        if !q.is_empty() {
            let pattern = format!("%{}%", q);
            sql.push_str(" AND (title LIKE ? OR command LIKE ? OR tags LIKE ? OR description LIKE ?)");
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern));
        }
    }

    sql.push_str(" ORDER BY is_favorite DESC, usage_count DESC, updated_at DESC");

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(row_to_snippet(row))
        })
        .map_err(|e| e.to_string())?;

    let mut snippets = Vec::new();
    for row in rows {
        snippets.push(row.map_err(|e| e.to_string())?);
    }
    Ok(snippets)
}

/// Gets a single snippet by ID.
pub fn get(conn: &Connection, id: &str) -> Result<Option<Snippet>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, description, command, tags, folder_id, is_favorite,
                    usage_count, last_used_at, created_at, updated_at
             FROM snippets WHERE id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let result = stmt
        .query_row(rusqlite::params![id], |row| Ok(row_to_snippet(row)))
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(result)
}

/// Creates a new snippet.
pub fn create(conn: &Connection, input: &SnippetInput) -> Result<Snippet, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = time::OffsetDateTime::now_utc().to_string();
    let tags_str = input.tags.join(",");

    conn.execute(
        "INSERT INTO snippets (id, title, description, command, tags, folder_id,
                               is_favorite, usage_count, last_used_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, NULL, ?8, ?8)",
        rusqlite::params![
            id,
            input.title,
            input.description,
            input.command,
            tags_str,
            input.folder_id,
            input.is_favorite as i32,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    get(conn, &id)?.ok_or_else(|| "Failed to read created snippet".to_string())
}

/// Updates an existing snippet.
pub fn update(conn: &Connection, id: &str, input: &SnippetInput) -> Result<Snippet, String> {
    let now = time::OffsetDateTime::now_utc().to_string();
    let tags_str = input.tags.join(",");

    let changed = conn
        .execute(
            "UPDATE snippets SET title=?1, description=?2, command=?3, tags=?4,
                    folder_id=?5, is_favorite=?6, updated_at=?7
             WHERE id=?8",
            rusqlite::params![
                input.title,
                input.description,
                input.command,
                tags_str,
                input.folder_id,
                input.is_favorite as i32,
                now,
                id,
            ],
        )
        .map_err(|e| e.to_string())?;

    if changed == 0 {
        return Err("Snippet not found".to_string());
    }
    get(conn, id)?.ok_or_else(|| "Failed to read updated snippet".to_string())
}

/// Deletes a snippet by ID.
pub fn delete(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM snippets WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Increments usage_count and updates last_used_at for a snippet.
pub fn record_usage(conn: &Connection, id: &str) -> Result<(), String> {
    let now = time::OffsetDateTime::now_utc().to_string();
    conn.execute(
        "UPDATE snippets SET usage_count = usage_count + 1, last_used_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================
// Snippet Folder CRUD
// ============================================================

/// Lists all snippet folders.
pub fn folder_list(conn: &Connection) -> Result<Vec<SnippetFolder>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, parent_id, sort_order, created_at FROM snippet_folders ORDER BY sort_order")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(SnippetFolder {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                sort_order: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut folders = Vec::new();
    for row in rows {
        folders.push(row.map_err(|e| e.to_string())?);
    }
    Ok(folders)
}

/// Creates a new snippet folder.
pub fn folder_create(conn: &Connection, input: &SnippetFolderInput) -> Result<SnippetFolder, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = time::OffsetDateTime::now_utc().to_string();

    conn.execute(
        "INSERT INTO snippet_folders (id, name, parent_id, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, input.name, input.parent_id, input.sort_order, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(SnippetFolder {
        id,
        name: input.name.clone(),
        parent_id: input.parent_id.clone(),
        sort_order: input.sort_order,
        created_at: now,
    })
}

/// Updates a snippet folder.
pub fn folder_update(conn: &Connection, id: &str, input: &SnippetFolderInput) -> Result<(), String> {
    let changed = conn
        .execute(
            "UPDATE snippet_folders SET name=?1, parent_id=?2, sort_order=?3 WHERE id=?4",
            rusqlite::params![input.name, input.parent_id, input.sort_order, id],
        )
        .map_err(|e| e.to_string())?;

    if changed == 0 {
        return Err("Snippet folder not found".to_string());
    }
    Ok(())
}

/// Deletes a snippet folder. Orphaned snippets have folder_id set to NULL.
pub fn folder_delete(conn: &Connection, id: &str) -> Result<(), String> {
    // Orphan snippets in this folder
    conn.execute(
        "UPDATE snippets SET folder_id = NULL WHERE folder_id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "DELETE FROM snippet_folders WHERE id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================
// Variable Template Helpers
// ============================================================

/// Extracts all `${VAR_NAME}` variable names from a command template.
/// Returns deduplicated names in order of first appearance.
pub fn extract_variables(command: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                name.push(c);
            }
            if !name.is_empty() && seen.insert(name.clone()) {
                vars.push(name);
            }
        }
    }
    vars
}

/// Resolves `${VAR_NAME}` placeholders in a command template.
/// Unresolved variables are left as-is.
pub fn resolve_variables(
    command: &str,
    variables: &std::collections::HashMap<String, String>,
) -> String {
    let mut result = String::with_capacity(command.len());
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                name.push(c);
            }
            if let Some(value) = variables.get(&name) {
                result.push_str(value);
            } else {
                // Leave unresolved
                result.push_str("${");
                result.push_str(&name);
                result.push('}');
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ── helpers ──

fn row_to_snippet(row: &rusqlite::Row) -> Snippet {
    let tags_raw: String = row.get::<_, String>(4).unwrap_or_default();
    let tags: Vec<String> = if tags_raw.is_empty() {
        Vec::new()
    } else {
        tags_raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    };

    Snippet {
        id: row.get(0).unwrap_or_default(),
        title: row.get(1).unwrap_or_default(),
        description: row.get(2).unwrap_or_default(),
        command: row.get(3).unwrap_or_default(),
        tags,
        folder_id: row.get(5).unwrap_or_default(),
        is_favorite: row.get::<_, i32>(6).unwrap_or(0) != 0,
        usage_count: row.get(7).unwrap_or(0),
        last_used_at: row.get(8).unwrap_or_default(),
        created_at: row.get(9).unwrap_or_default(),
        updated_at: row.get(10).unwrap_or_default(),
    }
}

use rusqlite::OptionalExtension;
