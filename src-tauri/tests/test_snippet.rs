//! Integration tests for snippet storage and variable template helpers.

use std::collections::HashMap;

use rusqlite::Connection;
use termex_lib::storage::migrations::run_migrations;
use termex_lib::storage::models::{SnippetFolderInput, SnippetInput};
use termex_lib::storage::snippet;

fn fresh_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    run_migrations(&conn).unwrap();
    conn
}

fn sample_input(title: &str, command: &str) -> SnippetInput {
    SnippetInput {
        title: title.to_string(),
        description: Some(format!("Desc for {title}")),
        command: command.to_string(),
        tags: vec!["test".to_string()],
        folder_id: None,
        is_favorite: false,
    }
}

// ── Snippet CRUD ──

#[test]
fn test_snippet_create_and_get() {
    let conn = fresh_db();
    let input = sample_input("List files", "ls -la");
    let created = snippet::create(&conn, &input).unwrap();

    assert_eq!(created.title, "List files");
    assert_eq!(created.command, "ls -la");
    assert_eq!(created.description.as_deref(), Some("Desc for List files"));
    assert_eq!(created.tags, vec!["test"]);
    assert!(!created.is_favorite);
    assert_eq!(created.usage_count, 0);
    assert!(created.last_used_at.is_none());

    // Get by ID
    let fetched = snippet::get(&conn, &created.id).unwrap().unwrap();
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.title, created.title);
}

#[test]
fn test_snippet_update() {
    let conn = fresh_db();
    let created = snippet::create(&conn, &sample_input("Old", "echo old")).unwrap();

    let update_input = SnippetInput {
        title: "New".to_string(),
        description: Some("Updated desc".to_string()),
        command: "echo new".to_string(),
        tags: vec!["updated".to_string()],
        folder_id: None,
        is_favorite: true,
    };
    let updated = snippet::update(&conn, &created.id, &update_input).unwrap();

    assert_eq!(updated.title, "New");
    assert_eq!(updated.command, "echo new");
    assert!(updated.is_favorite);
    assert_eq!(updated.tags, vec!["updated"]);
}

#[test]
fn test_snippet_delete() {
    let conn = fresh_db();
    let created = snippet::create(&conn, &sample_input("ToDelete", "rm -rf /tmp/junk")).unwrap();
    let id = created.id.clone();

    snippet::delete(&conn, &id).unwrap();

    let fetched = snippet::get(&conn, &id).unwrap();
    assert!(fetched.is_none(), "snippet should be deleted");
}

// ── Search ──

#[test]
fn test_snippet_search_title() {
    let conn = fresh_db();
    snippet::create(&conn, &sample_input("Deploy app", "ansible-playbook deploy.yml")).unwrap();
    snippet::create(&conn, &sample_input("Restart service", "systemctl restart nginx")).unwrap();
    snippet::create(&conn, &sample_input("Check logs", "tail -f /var/log/syslog")).unwrap();

    let results = snippet::list(&conn, None, Some("deploy")).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Deploy app");
}

#[test]
fn test_snippet_search_command() {
    let conn = fresh_db();
    snippet::create(&conn, &sample_input("Disk usage", "df -h")).unwrap();
    snippet::create(&conn, &sample_input("Memory", "free -m")).unwrap();

    let results = snippet::list(&conn, None, Some("df")).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].command, "df -h");
}

#[test]
fn test_snippet_search_tags() {
    let conn = fresh_db();
    let mut input = sample_input("Tagged", "echo tagged");
    input.tags = vec!["network".to_string(), "debug".to_string()];
    snippet::create(&conn, &input).unwrap();

    snippet::create(&conn, &sample_input("Unrelated", "echo other")).unwrap();

    let results = snippet::list(&conn, None, Some("network")).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Tagged");
}

// ── Favorite Sort ──

#[test]
fn test_snippet_favorite_sort() {
    let conn = fresh_db();
    snippet::create(&conn, &sample_input("Normal", "echo normal")).unwrap();

    let mut fav_input = sample_input("Favorite", "echo fav");
    fav_input.is_favorite = true;
    snippet::create(&conn, &fav_input).unwrap();

    let all = snippet::list(&conn, None, None).unwrap();
    assert!(all.len() >= 2);
    assert_eq!(all[0].title, "Favorite", "favorited snippet should appear first");
}

// ── Usage Count ──

#[test]
fn test_snippet_usage_count() {
    let conn = fresh_db();
    let created = snippet::create(&conn, &sample_input("Used", "echo used")).unwrap();
    assert_eq!(created.usage_count, 0);
    assert!(created.last_used_at.is_none());

    snippet::record_usage(&conn, &created.id).unwrap();
    snippet::record_usage(&conn, &created.id).unwrap();
    snippet::record_usage(&conn, &created.id).unwrap();

    let fetched = snippet::get(&conn, &created.id).unwrap().unwrap();
    assert_eq!(fetched.usage_count, 3);
    assert!(fetched.last_used_at.is_some(), "last_used_at should be set after recording usage");
}

// ── Folder CRUD ──

#[test]
fn test_snippet_folder_crud() {
    let conn = fresh_db();

    // Create
    let folder = snippet::folder_create(
        &conn,
        &SnippetFolderInput {
            name: "DevOps".to_string(),
            parent_id: None,
            sort_order: 0,
        },
    )
    .unwrap();
    assert_eq!(folder.name, "DevOps");

    // List
    let folders = snippet::folder_list(&conn).unwrap();
    assert_eq!(folders.len(), 1);

    // Update
    snippet::folder_update(
        &conn,
        &folder.id,
        &SnippetFolderInput {
            name: "Operations".to_string(),
            parent_id: None,
            sort_order: 1,
        },
    )
    .unwrap();
    let folders = snippet::folder_list(&conn).unwrap();
    assert_eq!(folders[0].name, "Operations");

    // Delete
    snippet::folder_delete(&conn, &folder.id).unwrap();
    let folders = snippet::folder_list(&conn).unwrap();
    assert!(folders.is_empty());
}

#[test]
fn test_snippet_folder_delete_orphans() {
    let conn = fresh_db();

    // Create folder
    let folder = snippet::folder_create(
        &conn,
        &SnippetFolderInput {
            name: "Scripts".to_string(),
            parent_id: None,
            sort_order: 0,
        },
    )
    .unwrap();

    // Create snippet in folder
    let mut input = sample_input("In folder", "echo folder");
    input.folder_id = Some(folder.id.clone());
    let created = snippet::create(&conn, &input).unwrap();
    assert_eq!(created.folder_id.as_deref(), Some(folder.id.as_str()));

    // Delete folder — snippet should be orphaned (folder_id → NULL)
    snippet::folder_delete(&conn, &folder.id).unwrap();

    let fetched = snippet::get(&conn, &created.id).unwrap().unwrap();
    assert!(
        fetched.folder_id.is_none(),
        "snippet folder_id should be NULL after folder deletion"
    );
}

// ── Variable Template Helpers ──

#[test]
fn test_variable_extract_basic() {
    let vars = snippet::extract_variables("kubectl apply -f ${FILE}");
    assert_eq!(vars, vec!["FILE"]);
}

#[test]
fn test_variable_extract_multiple() {
    let vars = snippet::extract_variables("scp ${USER}@${HOST}:${PATH} .");
    assert_eq!(vars, vec!["USER", "HOST", "PATH"]);
}

#[test]
fn test_variable_extract_dedup() {
    let vars = snippet::extract_variables("echo ${X} ${X}");
    assert_eq!(vars, vec!["X"], "duplicate variables should be deduplicated");
}

#[test]
fn test_variable_resolve() {
    let mut vars = HashMap::new();
    vars.insert("USER".to_string(), "admin".to_string());
    vars.insert("HOST".to_string(), "10.0.1.1".to_string());

    let resolved = snippet::resolve_variables("ssh ${USER}@${HOST}", &vars);
    assert_eq!(resolved, "ssh admin@10.0.1.1");
}

#[test]
fn test_variable_resolve_unresolved() {
    let vars = HashMap::new();
    let resolved = snippet::resolve_variables("echo ${MISSING}", &vars);
    assert_eq!(
        resolved, "echo ${MISSING}",
        "unresolved variables should be left as-is"
    );
}
