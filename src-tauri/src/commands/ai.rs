use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, State};

use crate::ai::danger::{DangerDetector, DangerResult};
use crate::crypto::aes;
use crate::keychain;
use crate::state::AppState;
use crate::storage::models::{AiProvider, ProviderType};

// ── AI Provider CRUD ──────────────────────────────────────────

/// Input for creating/updating an AI provider.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    pub name: String,
    pub provider_type: String,
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i32,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    pub is_default: bool,
}

fn default_max_tokens() -> i32 { 4096 }
fn default_temperature() -> f64 { 0.7 }

/// Lists all AI providers.
#[tauri::command]
pub fn ai_provider_list(state: State<'_, AppState>) -> Result<Vec<AiProvider>, String> {
    state
        .db
        .with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, provider_type, api_key_enc, api_base_url, model,
                        max_tokens, temperature, is_default, created_at, updated_at
                 FROM ai_providers ORDER BY is_default DESC, name",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    let pt: String = row.get(2)?;
                    Ok(AiProvider {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        provider_type: parse_provider_type(&pt),
                        api_key_enc: row.get(3)?,
                        api_base_url: row.get(4)?,
                        model: row.get(5)?,
                        max_tokens: row.get(6)?,
                        temperature: row.get(7)?,
                        is_default: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
        .map_err(|e| e.to_string())
}

/// Adds a new AI provider.
#[tauri::command]
pub fn ai_provider_add(
    state: State<'_, AppState>,
    input: ProviderInput,
) -> Result<AiProvider, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = time::OffsetDateTime::now_utc().to_string();

    // Store API key in OS keychain
    let keychain_key = keychain::ai_apikey_key(&id);
    let api_key_keychain_id = if let Some(key) = input.api_key.as_deref().filter(|s| !s.is_empty()) {
        keychain::store(&keychain_key, key).ok();
        Some(keychain_key)
    } else {
        None
    };

    // If setting as default, unset other defaults
    if input.is_default {
        let _ = state.db.with_conn(|conn| {
            conn.execute("UPDATE ai_providers SET is_default = 0", [])?;
            Ok(())
        });
    }

    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO ai_providers (id, name, provider_type, api_key_keychain_id,
                    api_base_url, model, max_tokens, temperature, is_default, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                rusqlite::params![
                    id,
                    input.name,
                    input.provider_type,
                    api_key_keychain_id,
                    input.api_base_url,
                    input.model,
                    input.max_tokens,
                    input.temperature,
                    input.is_default,
                    now,
                    now,
                ],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    Ok(AiProvider {
        id,
        name: input.name,
        provider_type: parse_provider_type(&input.provider_type),
        api_key_enc: None,
        api_base_url: input.api_base_url,
        model: input.model,
        max_tokens: input.max_tokens,
        temperature: input.temperature,
        is_default: input.is_default,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Updates an AI provider.
#[tauri::command]
pub fn ai_provider_update(
    state: State<'_, AppState>,
    id: String,
    input: ProviderInput,
) -> Result<(), String> {
    let now = time::OffsetDateTime::now_utc().to_string();

    // Update keychain API key if provided
    let keychain_key = keychain::ai_apikey_key(&id);
    let api_key_keychain_id = if let Some(key) = input.api_key.as_deref().filter(|s| !s.is_empty()) {
        keychain::store(&keychain_key, key).ok();
        Some(keychain_key)
    } else {
        None
    };

    if input.is_default {
        let _ = state.db.with_conn(|conn| {
            conn.execute("UPDATE ai_providers SET is_default = 0", [])?;
            Ok(())
        });
    }

    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE ai_providers SET name=?1, provider_type=?2,
                    api_key_keychain_id=COALESCE(?3, api_key_keychain_id), api_base_url=?4,
                    model=?5, max_tokens=?6, temperature=?7,
                    is_default=?8, updated_at=?9
                 WHERE id=?10",
                rusqlite::params![
                    input.name,
                    input.provider_type,
                    api_key_keychain_id,
                    input.api_base_url,
                    input.model,
                    input.max_tokens,
                    input.temperature,
                    input.is_default,
                    now,
                    id,
                ],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())
}

/// Deletes an AI provider.
#[tauri::command]
pub fn ai_provider_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    // Clean up keychain
    let _ = keychain::delete(&keychain::ai_apikey_key(&id));

    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "DELETE FROM ai_providers WHERE id = ?1",
                rusqlite::params![id],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())
}

/// Sets a provider as the default.
#[tauri::command]
pub fn ai_provider_set_default(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .db
        .with_conn(|conn| {
            conn.execute("UPDATE ai_providers SET is_default = 0", [])?;
            conn.execute(
                "UPDATE ai_providers SET is_default = 1 WHERE id = ?1",
                rusqlite::params![id],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())
}

// ── Danger Detection ──────────────────────────────────────────

/// Checks a command for dangerous patterns (local regex detection).
#[tauri::command]
pub fn ai_check_danger(command: String) -> DangerResult {
    let detector = DangerDetector::new();
    detector.check(&command)
}

// ── Command Explanation ───────────────────────────────────────

/// Explains a command using the default AI provider.
/// Streams response chunks via `ai://explain/{request_id}` events.
#[tauri::command]
pub async fn ai_explain_command(
    state: State<'_, AppState>,
    app: AppHandle,
    command: String,
    request_id: String,
) -> Result<(), String> {
    // Load default provider config
    let provider_info = state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT id, provider_type, api_key_enc, api_base_url, model
                 FROM ai_providers WHERE is_default = 1 LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<Vec<u8>>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
        })
        .map_err(|_| "no default AI provider configured".to_string())?;

    let (pid, provider_type, api_key_enc, api_base_url, model) = provider_info;
    let api_key = resolve_api_key(&state, &pid, api_key_enc);

    let event = format!("ai://explain/{request_id}");
    let system = "You are a command-line expert. Explain what the given shell command does, step by step. Be concise and clear. Use the user's language.";
    let user_msg = format!("Explain this command:\n```\n{command}\n```");

    // Make HTTP request to the AI provider
    let response = call_ai_provider(
        &state,
        &provider_type,
        &api_key,
        api_base_url.as_deref(),
        &model,
        system,
        &user_msg,
        None,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Emit the full response as a single chunk
    let _ = app.emit(
        &event,
        serde_json::json!({ "text": response, "done": true }),
    );

    Ok(())
}

// ── NL2Cmd ────────────────────────────────────────────────────

/// Converts a natural language description to a shell command.
/// Returns the generated command string.
#[tauri::command]
pub async fn ai_nl2cmd(
    state: State<'_, AppState>,
    app: AppHandle,
    description: String,
    context: NlContext,
    request_id: String,
) -> Result<(), String> {
    let provider_info = state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT id, provider_type, api_key_enc, api_base_url, model
                 FROM ai_providers WHERE is_default = 1 LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<Vec<u8>>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
        })
        .map_err(|_| "no default AI provider configured".to_string())?;

    let (pid, provider_type, api_key_enc, api_base_url, model) = provider_info;
    let api_key = resolve_api_key(&state, &pid, api_key_enc);

    let system = format!(
        "You are a shell command expert. Convert the user's natural language description into a shell command.\n\
         Context: OS={}, Shell={}, CWD={}\n\
         Rules:\n\
         - Output ONLY the command, no explanation\n\
         - If multiple commands are needed, join with && or ;\n\
         - Use common, portable commands when possible",
        context.os.as_deref().unwrap_or("Linux"),
        context.shell.as_deref().unwrap_or("bash"),
        context.cwd.as_deref().unwrap_or("~"),
    );

    let event = format!("ai://nl2cmd/{request_id}");

    let response = call_ai_provider(
        &state,
        &provider_type,
        &api_key,
        api_base_url.as_deref(),
        &model,
        &system,
        &description,
        None,
    )
    .await
    .map_err(|e| e.to_string())?;

    let _ = app.emit(
        &event,
        serde_json::json!({ "command": response.trim(), "done": true }),
    );

    Ok(())
}

/// Context information for NL2Cmd.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NlContext {
    pub os: Option<String>,
    pub shell: Option<String>,
    pub cwd: Option<String>,
}

/// Tests the connection to an AI provider.
#[tauri::command]
pub async fn ai_provider_test(
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let provider_info = state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT provider_type, api_key_enc, api_base_url, model
                 FROM ai_providers WHERE id = ?1",
                rusqlite::params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<Vec<u8>>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
        })
        .map_err(|e| e.to_string())?;

    let (provider_type, api_key_enc, api_base_url, model) = provider_info;
    let api_key = resolve_api_key(&state, &id, api_key_enc);

    let response = call_ai_provider(
        &state,
        &provider_type,
        &api_key,
        api_base_url.as_deref(),
        &model,
        "Reply with exactly: OK",
        "Test connection",
        Some(64),
    )
    .await?;

    Ok(response)
}

/// Returns the API key for an AI provider (from keychain or legacy).
#[tauri::command]
pub fn ai_provider_get_key(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<String, String> {
    // Try keychain first
    if let Ok(key) = keychain::get(&keychain::ai_apikey_key(&provider_id)) {
        return Ok(key);
    }

    // Fallback: legacy encrypted field
    let api_key_enc: Option<Vec<u8>> = state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT api_key_enc FROM ai_providers WHERE id = ?1",
                rusqlite::params![provider_id],
                |row| row.get(0),
            )
        })
        .map_err(|e| e.to_string())?;

    let mk = state.master_key.read().expect("master_key lock poisoned");
    match (&*mk, api_key_enc) {
        (Some(key), Some(enc)) => {
            let plain = aes::decrypt(key, &enc).map_err(|e| e.to_string())?;
            String::from_utf8(plain).map_err(|e| e.to_string())
        }
        (None, Some(enc)) => String::from_utf8(enc).map_err(|e| e.to_string()),
        _ => Ok(String::new()),
    }
}

/// Tests AI provider connectivity using provided credentials directly (without saving).
#[tauri::command]
pub async fn ai_provider_test_direct(
    state: State<'_, AppState>,
    provider_type: String,
    api_key: String,
    api_base_url: Option<String>,
    model: String,
) -> Result<String, String> {
    // Local provider doesn't need connection test
    if provider_type == "local" {
        return Err("Local AI provider doesn't require connection test".to_string());
    }

    let response = call_ai_provider(
        &state,
        &provider_type,
        &api_key,
        api_base_url.as_deref(),
        &model,
        "Reply with exactly: OK",
        "Test connection",
        Some(64),
    )
    .await?;

    Ok(response)
}

// ── Internal Helpers ──────────────────────────────────────────

fn parse_provider_type(s: &str) -> ProviderType {
    match s {
        "claude" => ProviderType::Claude,
        "openai" => ProviderType::Openai,
        "gemini" => ProviderType::Gemini,
        "deepseek" => ProviderType::Deepseek,
        "ollama" => ProviderType::Ollama,
        "grok" => ProviderType::Grok,
        "mistral" => ProviderType::Mistral,
        "glm" => ProviderType::Glm,
        "minimax" => ProviderType::Minimax,
        "doubao" => ProviderType::Doubao,
        "local" => ProviderType::Local,
        _ => ProviderType::Custom,
    }
}

/// Resolves the API key for a provider: keychain first, then legacy DB field.
fn resolve_api_key(
    state: &State<'_, AppState>,
    provider_id: &str,
    api_key_enc: Option<Vec<u8>>,
) -> String {
    // Try keychain
    if let Ok(key) = keychain::get(&keychain::ai_apikey_key(provider_id)) {
        return key;
    }
    // Fallback: legacy encrypted field
    let mk = state.master_key.read().expect("master_key lock poisoned");
    match (&*mk, api_key_enc) {
        (Some(key), Some(enc)) => {
            aes::decrypt(key, &enc)
                .map(|p| String::from_utf8(p).unwrap_or_default())
                .unwrap_or_default()
        }
        (_, Some(enc)) => String::from_utf8(enc).unwrap_or_default(),
        _ => String::new(),
    }
}

/// Calls an AI provider's completion API.
async fn call_ai_provider(
    state: &AppState,
    provider_type: &str,
    api_key: &str,
    base_url: Option<&str>,
    model: &str,
    system: &str,
    user_message: &str,
    max_tokens: Option<i32>,
) -> Result<String, String> {
    let max_tok = max_tokens.unwrap_or(4096);
    let client = reqwest::Client::new();

    match provider_type {
        // Claude — uses Anthropic Messages API
        "claude" => {
            let url = format!(
                "{}/messages",
                base_url.unwrap_or("https://api.anthropic.com/v1")
            );
            let body = serde_json::json!({
                "model": model,
                "max_tokens": max_tok,
                "system": system,
                "messages": [
                    { "role": "user", "content": user_message },
                ],
            });
            let resp = client
                .post(&url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            if let Some(err) = json["error"]["message"].as_str() {
                return Err(err.to_string());
            }
            Ok(json["content"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string())
        }
        // Gemini — uses Google Generative Language API
        "gemini" => {
            let url = format!(
                "{}/v1beta/models/{}:generateContent?key={}",
                base_url.unwrap_or("https://generativelanguage.googleapis.com"),
                model,
                api_key
            );
            let body = serde_json::json!({
                "system_instruction": { "parts": [{ "text": system }] },
                "contents": [{ "parts": [{ "text": user_message }] }],
            });
            let resp = client
                .post(&url)
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            if let Some(err) = json["error"]["message"].as_str() {
                return Err(err.to_string());
            }
            Ok(json["candidates"][0]["content"]["parts"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string())
        }
        // Local model via llama-server
        "local" => {
            let server = state.llama_server.read().await;
            let port = server
                .port
                .ok_or("Local AI engine is not running. Start it first.")?;
            drop(server);

            let url = format!("http://localhost:{}/v1/chat/completions", port);
            let body = serde_json::json!({
                "model": model,
                "messages": [
                    { "role": "system", "content": system },
                    { "role": "user", "content": user_message },
                ],
                "max_tokens": max_tok,
            });
            let resp = client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            if let Some(err) = json["error"]["message"].as_str() {
                return Err(err.to_string());
            }
            Ok(json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string())
        }
        // Ollama — local REST API
        "ollama" => {
            let url = format!(
                "{}/api/generate",
                base_url.unwrap_or("http://localhost:11434")
            );
            let body = serde_json::json!({
                "model": model,
                "prompt": format!("{system}\n\n{user_message}"),
                "stream": false,
            });
            let resp = client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            Ok(json["response"].as_str().unwrap_or("").to_string())
        }
        // OpenAI-compatible: openai, deepseek, grok, mistral, glm, minimax, doubao, custom
        _ => {
            let default_base = match provider_type {
                "openai" => "https://api.openai.com/v1",
                "deepseek" => "https://api.deepseek.com/v1",
                "grok" => "https://api.x.ai/v1",
                "mistral" => "https://api.mistral.ai/v1",
                "glm" => "https://open.bigmodel.cn/api/paas/v4",
                "minimax" => "https://api.minimax.io/v1",
                "doubao" => "https://ark.cn-beijing.volces.com/api/v3",
                _ => "https://api.openai.com/v1",
            };
            let url = format!(
                "{}/chat/completions",
                base_url.unwrap_or(default_base)
            );
            let body = serde_json::json!({
                "model": model,
                "messages": [
                    { "role": "system", "content": system },
                    { "role": "user", "content": user_message },
                ],
                "max_tokens": max_tok,
            });
            let resp = client
                .post(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            if let Some(err) = json["error"]["message"].as_str() {
                return Err(err.to_string());
            }
            Ok(json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string())
        }
    }
}

// ── Autocomplete ─────────────────────────────────────────────

/// Context information for AI-powered command autocomplete.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutocompleteContext {
    pub partial_command: String,
    pub os: Option<String>,
    pub shell: Option<String>,
    pub cwd: Option<String>,
    pub recent_commands: Vec<String>,
    /// When true, prefer local AI engine over cloud providers (default: true).
    #[serde(default = "default_true")]
    pub prefer_local: bool,
    /// When true, the partial command may contain sensitive data (passwords, tokens).
    /// Cloud providers will be skipped; only local AI is allowed.
    #[serde(default)]
    pub has_sensitive: bool,
}

fn default_true() -> bool {
    true
}

/// Extracts the content between the first pair of ``` markers in the text.
pub fn extract_code_fence(text: &str) -> Option<&str> {
    let start = text.find("```")?;
    let after_open = start + 3;
    // Skip optional language tag on the same line as opening ```
    let content_start = text[after_open..].find('\n').map(|i| after_open + i + 1)?;
    let end = text[content_start..].find("```")?;
    let content = &text[content_start..content_start + end];
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Parses AI response text into a list of command suggestions with 6-layer fallback.
///
/// Layers:
/// 1. Direct JSON array parse
/// 2. Extract markdown code fence, then JSON parse
/// 3. Find `[...]` in text via string search, then JSON parse
/// 4. Split by newlines, trim list markers (-, *, 1., etc.), filter empty/noise
/// 5. Single JSON string parse
/// 6. Raw text as single command (if < 200 chars)
///
/// Returns at most 5 non-empty items.
pub fn parse_suggestions(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // Layer 1: Direct JSON array parse
    if let Ok(arr) = serde_json::from_str::<Vec<String>>(trimmed) {
        return finalize_suggestions(arr);
    }

    // Layer 2: Extract markdown code fence then JSON parse
    if let Some(fenced) = extract_code_fence(trimmed) {
        if let Ok(arr) = serde_json::from_str::<Vec<String>>(fenced) {
            return finalize_suggestions(arr);
        }
    }

    // Layer 3: Find [...] in text via string search, then JSON parse
    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            if end > start {
                let bracket_content = &trimmed[start..=end];
                if let Ok(arr) = serde_json::from_str::<Vec<String>>(bracket_content) {
                    return finalize_suggestions(arr);
                }
            }
        }
    }

    // Layer 4: Split by newlines, trim list markers (-, *, 1., etc.), filter empty/noise
    let lines: Vec<String> = trimmed
        .lines()
        .map(|line| {
            let l = line.trim();
            // Strip leading list markers: "- ", "* ", "1. ", "2) ", etc.
            let stripped = l
                .strip_prefix("- ")
                .or_else(|| l.strip_prefix("* "))
                .or_else(|| {
                    // Handle numbered lists: "1. ", "2. ", "1) ", "2) ", etc.
                    let bytes = l.as_bytes();
                    let mut i = 0;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i > 0 && i < bytes.len() {
                        if bytes[i] == b'.' || bytes[i] == b')' {
                            let after = &l[i + 1..];
                            Some(after.strip_prefix(' ').unwrap_or(after))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .unwrap_or(l);
            // Remove surrounding backticks
            let stripped = stripped.trim_matches('`').trim().to_string();
            stripped
        })
        .filter(|s| !s.is_empty())
        // Filter noise lines (explanatory text, not commands)
        .filter(|s| !s.starts_with("Here ") && !s.starts_with("The ") && !s.starts_with("Note"))
        .collect();

    // Layer 5: Single JSON string parse (before line split — catches `"git checkout"`)
    if let Ok(single) = serde_json::from_str::<String>(trimmed) {
        if !single.is_empty() {
            return finalize_suggestions(vec![single]);
        }
    }

    if !lines.is_empty() && lines.iter().any(|l| !l.contains(' ') || l.len() < 120) {
        return finalize_suggestions(lines);
    }

    // Layer 6: Raw text as single command (if < 200 chars)
    if trimmed.len() < 200 {
        return finalize_suggestions(vec![trimmed.to_string()]);
    }

    Vec::new()
}

/// Filters and repairs suggestions from LLM output.
///
/// Small models often return partial results (e.g., ["checkout", "commit"] for "git co").
/// This function:
/// 1. Keeps suggestions that already start with the partial command
/// 2. Tries to repair partial suggestions by prepending the command prefix
/// 3. Deduplicates results
fn filter_by_prefix(suggestions: Vec<String>, partial: &str) -> Vec<String> {
    let lower_partial = partial.to_lowercase();
    // Extract the base command (e.g., "git" from "git co")
    let base_cmd = partial.split_whitespace().next().unwrap_or("");
    // Extract the subcommand prefix (e.g., "co" from "git co")
    let sub_prefix = partial.strip_prefix(base_cmd).unwrap_or("").trim_start();

    let mut result: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for s in suggestions {
        let lower_s = s.to_lowercase();

        if lower_s.starts_with(&lower_partial) && s.len() > partial.len() {
            // Already a valid full completion
            if seen.insert(s.to_lowercase()) {
                result.push(s);
            }
        } else if !base_cmd.is_empty() && !sub_prefix.is_empty() {
            // Try to repair: the LLM might have returned just the subcommand
            // e.g., "checkout" instead of "git checkout" for partial "git ch"
            let lower_s_trimmed = s.trim().to_lowercase();
            if lower_s_trimmed.starts_with(sub_prefix) {
                let repaired = format!("{} {}", base_cmd, s.trim());
                if repaired.len() > partial.len() && seen.insert(repaired.to_lowercase()) {
                    result.push(repaired);
                }
            }
        }
    }

    result.into_iter().take(5).collect()
}

/// Filters empty strings and truncates to at most 5 items.
fn finalize_suggestions(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .take(5)
        .collect()
}

/// AI-powered command autocomplete.
///
/// Uses local AI first strategy: if llama-server is running, try local with 1.5s timeout.
/// Falls back to the default cloud provider with 3s timeout.
/// On timeout or error, returns empty vec (graceful degradation).
#[tauri::command]
pub async fn ai_autocomplete(
    state: State<'_, AppState>,
    context: AutocompleteContext,
) -> Result<Vec<String>, String> {
    let system = format!(
        "You are a shell command autocomplete tool.\n\
         OS={}, Shell={}, CWD={}\n\
         The user has typed a partial command. Suggest complete commands they might want.\n\
         IMPORTANT: Each suggestion must be a FULL command starting with exactly what the user typed.\n\
         Example: if the user typed \"git co\", good suggestions are [\"git commit\", \"git checkout\"].\n\
         Bad suggestions: [\"co\", \"git\", \"commit\"] — these do NOT start with \"git co\".\n\
         Return ONLY a JSON array. No explanation.",
        context.os.as_deref().unwrap_or("Linux"),
        context.shell.as_deref().unwrap_or("bash"),
        context.cwd.as_deref().unwrap_or("~"),
    );

    let user_msg = if context.recent_commands.is_empty() {
        format!("Partial command: \"{}\"\nSuggest completions:", context.partial_command)
    } else {
        format!(
            "Recent: {}\nPartial command: \"{}\"\nSuggest completions:",
            context.recent_commands.join(", "),
            context.partial_command,
        )
    };

    // Strategy: try local AI first (faster, no network), fall back to cloud provider.
    // If has_sensitive is true, only local AI is allowed (data never leaves machine).

    // Check if local llama-server is running
    let local_port = if context.prefer_local {
        let server = state.llama_server.read().await;
        server.port
    } else {
        None
    };

    log::info!("[autocomplete] partial={}, prefer_local={}, has_sensitive={}, local_port={:?}",
        context.partial_command, context.prefer_local, context.has_sensitive, local_port);

    // If local_port is None but prefer_local is true, try to discover a running llama-server
    // (handles dev hot-reload or app restart where state is lost but process survives)
    let local_port = if local_port.is_none() && context.prefer_local {
        let discovered = discover_llama_port().await;
        if let Some(port) = discovered {
            log::info!("[autocomplete] discovered llama-server on port {}, recovering state", port);
            // Recover the state so future calls don't need to scan
            let mut server = state.llama_server.write().await;
            server.port = Some(port);
            drop(server);
        }
        discovered
    } else {
        local_port
    };

    if local_port.is_some() {
        // Try local AI with 1.5s timeout
        let local_result = tokio::time::timeout(
            Duration::from_millis(1500),
            call_ai_provider(
                &state,
                "local",
                "",
                None,
                "local",
                &system,
                &user_msg,
                Some(256),
            ),
        )
        .await;

        match &local_result {
            Ok(Ok(response)) => {
                log::info!("[autocomplete] local response: {}", &response[..response.len().min(200)]);
                let suggestions = filter_by_prefix(parse_suggestions(response), &context.partial_command);
                if !suggestions.is_empty() {
                    return Ok(suggestions);
                }
                log::info!("[autocomplete] local returned empty suggestions, falling through to cloud");
            }
            Ok(Err(e)) => {
                log::info!("[autocomplete] local AI error: {}", e);
            }
            Err(_) => {
                log::info!("[autocomplete] local AI timed out (1.5s)");
            }
        }
    }

    // Security: if command contains sensitive data, do NOT send to cloud providers
    if context.has_sensitive {
        log::info!("[autocomplete] skipping cloud — sensitive command");
        return Ok(Vec::new());
    }

    // Fall back to default cloud provider with 3s timeout
    let provider_info = state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT id, provider_type, api_key_enc, api_base_url, model
                 FROM ai_providers WHERE is_default = 1 LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<Vec<u8>>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
        });

    let (pid, provider_type, api_key_enc, api_base_url, model) = match provider_info {
        Ok(info) => {
            // Skip local-type providers in cloud fallback — we already tried local above
            if info.1 == "local" {
                log::info!("[autocomplete] default provider is local but engine not running, no fallback available");
                return Ok(Vec::new());
            }
            log::info!("[autocomplete] using cloud provider: type={}, model={}", info.1, info.4);
            info
        }
        Err(e) => {
            log::info!("[autocomplete] no default provider configured: {}", e);
            return Ok(Vec::new());
        }
    };

    let api_key = resolve_api_key(&state, &pid, api_key_enc);

    let cloud_result = tokio::time::timeout(
        Duration::from_secs(3),
        call_ai_provider(
            &state,
            &provider_type,
            &api_key,
            api_base_url.as_deref(),
            &model,
            &system,
            &user_msg,
            Some(256),
        ),
    )
    .await;

    match &cloud_result {
        Ok(Ok(response)) => {
            log::info!("[autocomplete] cloud response: {}", &response[..response.len().min(200)]);
            Ok(filter_by_prefix(parse_suggestions(response), &context.partial_command))
        }
        Ok(Err(e)) => {
            log::info!("[autocomplete] cloud error: {}", e);
            Ok(Vec::new())
        }
        Err(_) => {
            log::info!("[autocomplete] cloud timed out (3s)");
            Ok(Vec::new())
        }
    }
}

/// Scans the llama-server port range (15000-16000) for a running instance.
/// Used to recover from state loss after dev hot-reload or app restart.
async fn discover_llama_port() -> Option<u16> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(200))
        .build()
        .ok()?;

    for port in 15000..=15020 {
        let url = format!("http://localhost:{}/health", port);
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return Some(port);
            }
        }
    }
    None
}
