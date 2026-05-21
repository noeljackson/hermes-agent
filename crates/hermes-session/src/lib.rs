use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

const WAL_INCOMPAT_MARKERS: [&str; 3] = ["locking protocol", "not authorized", "disk i/o error"];

pub fn format_session_db_unavailable(prefix: &str, cause: Option<&str>) -> String {
    let Some(cause) = cause.filter(|value| !value.is_empty()) else {
        return format!("{prefix}.");
    };
    let lower = cause.to_ascii_lowercase();
    let hint = if WAL_INCOMPAT_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
    {
        " (state.db may be on NFS/SMB/FUSE \u{2014} see https://www.sqlite.org/wal.html)"
    } else {
        ""
    };
    format!("{prefix}: {cause}{hint}.")
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    session_id: String,
    messages: Vec<Value>,
}

impl SessionStore {
    pub fn parity_fixture_store() -> Self {
        let session_id = "parity-session-1".to_string();
        let messages = vec![
            message(1, &session_id, "user", "hello", None, None, None, None),
            message(
                2,
                &session_id,
                "assistant",
                "calling tool",
                Some("tool_calls"),
                None,
                Some(json!([
                    {
                        "function": {"arguments": "{}", "name": "memory"},
                        "id": "call-1",
                        "type": "function"
                    }
                ])),
                None,
            ),
            message(
                3,
                &session_id,
                "tool",
                "{\"success\": true}",
                None,
                Some("call-1"),
                None,
                Some("memory"),
            ),
        ];
        Self {
            session_id,
            messages,
        }
    }

    pub fn export_session(&self) -> Value {
        let mut session = base_session(&self.session_id);
        session["messages"] = Value::Array(self.messages.clone());
        session
    }

    pub fn resume_conversation(&self) -> Value {
        let messages = self
            .messages
            .iter()
            .map(conversation_message)
            .collect::<Vec<_>>();
        Value::Array(messages)
    }

    pub fn export_all(&self) -> Value {
        let mut session = self.export_session();
        session["last_active"] = json!("<timestamp>");
        Value::Array(vec![session])
    }
}

fn base_session(session_id: &str) -> Value {
    json!({
        "actual_cost_usd": null,
        "api_call_count": 0,
        "billing_base_url": null,
        "billing_mode": null,
        "billing_provider": null,
        "cache_read_tokens": 0,
        "cache_write_tokens": 0,
        "cost_source": null,
        "cost_status": null,
        "end_reason": null,
        "ended_at": null,
        "estimated_cost_usd": null,
        "handoff_error": null,
        "handoff_platform": null,
        "handoff_state": null,
        "id": session_id,
        "input_tokens": 0,
        "message_count": 3,
        "model": "fake/model",
        "model_config": "{\"provider\": \"fake\"}",
        "output_tokens": 0,
        "parent_session_id": null,
        "pricing_version": null,
        "reasoning_tokens": 0,
        "source": "cli",
        "started_at": "<timestamp>",
        "system_prompt": "system prompt",
        "title": null,
        "tool_call_count": 1,
        "user_id": "user-1",
    })
}

#[allow(clippy::too_many_arguments)]
fn message(
    id: i64,
    session_id: &str,
    role: &str,
    content: &str,
    finish_reason: Option<&str>,
    tool_call_id: Option<&str>,
    tool_calls: Option<Value>,
    tool_name: Option<&str>,
) -> Value {
    json!({
        "codex_message_items": null,
        "codex_reasoning_items": null,
        "content": content,
        "finish_reason": finish_reason,
        "id": id,
        "reasoning": null,
        "reasoning_content": null,
        "reasoning_details": null,
        "role": role,
        "session_id": session_id,
        "timestamp": "<timestamp>",
        "token_count": null,
        "tool_call_id": tool_call_id,
        "tool_calls": tool_calls,
        "tool_name": tool_name,
    })
}

fn conversation_message(message: &Value) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("content".to_string(), message["content"].clone());
    if !message["finish_reason"].is_null() {
        out.insert(
            "finish_reason".to_string(),
            message["finish_reason"].clone(),
        );
    }
    out.insert("role".to_string(), message["role"].clone());
    if !message["tool_call_id"].is_null() {
        out.insert("tool_call_id".to_string(), message["tool_call_id"].clone());
    }
    if !message["tool_calls"].is_null() {
        out.insert("tool_calls".to_string(), message["tool_calls"].clone());
    }
    if !message["tool_name"].is_null() {
        out.insert("tool_name".to_string(), message["tool_name"].clone());
    }
    Value::Object(out)
}

pub struct SqliteSessionStore {
    conn: Connection,
}

impl SqliteSessionStore {
    pub fn open(path: impl AsRef<Path>) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn create_session(
        &self,
        session_id: &str,
        source: &str,
        user_id: &str,
        model: &str,
        model_config: &str,
        system_prompt: &str,
    ) -> SqlResult<()> {
        self.create_session_with_parent(
            session_id,
            source,
            user_id,
            model,
            model_config,
            system_prompt,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_session_with_parent(
        &self,
        session_id: &str,
        source: &str,
        user_id: &str,
        model: &str,
        model_config: &str,
        system_prompt: &str,
        parent_session_id: Option<&str>,
    ) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO sessions (
                id, source, user_id, model, model_config, system_prompt, parent_session_id, started_at,
                message_count, tool_call_count, input_tokens, output_tokens,
                cache_read_tokens, cache_write_tokens, reasoning_tokens, api_call_count
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '<timestamp>', 0, 0, 0, 0, 0, 0, 0, 0)",
            params![
                session_id,
                source,
                user_id,
                model,
                model_config,
                system_prompt,
                parent_session_id
            ],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        finish_reason: Option<&str>,
        tool_call_id: Option<&str>,
        tool_calls: Option<&Value>,
        tool_name: Option<&str>,
    ) -> SqlResult<i64> {
        let tool_calls_text = tool_calls.map(Value::to_string);
        self.conn.execute(
            "INSERT INTO messages (
                session_id, role, content, finish_reason, tool_call_id,
                tool_calls, tool_name, timestamp
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '<timestamp>')",
            params![
                session_id,
                role,
                content,
                finish_reason,
                tool_call_id,
                tool_calls_text,
                tool_name
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        let tool_delta = i64::from(tool_calls.is_some());
        self.conn.execute(
            "UPDATE sessions
             SET message_count = message_count + 1,
                 tool_call_count = tool_call_count + ?1
             WHERE id = ?2",
            params![tool_delta, session_id],
        )?;
        Ok(id)
    }

    pub fn export_session(&self, session_id: &str) -> SqlResult<Option<Value>> {
        let session = self.session_json(session_id)?;
        let Some(mut session) = session else {
            return Ok(None);
        };
        session["messages"] = self.messages_json(session_id)?;
        Ok(Some(session))
    }

    pub fn resume_conversation(&self, session_id: &str) -> SqlResult<Value> {
        let messages = self.messages_json(session_id)?;
        Ok(Value::Array(
            messages
                .as_array()
                .unwrap()
                .iter()
                .map(conversation_message)
                .collect(),
        ))
    }

    pub fn export_all(&self, source: &str) -> SqlResult<Value> {
        self.export_all_optional(Some(source))
    }

    pub fn export_all_optional(&self, source: Option<&str>) -> SqlResult<Value> {
        let ids = if let Some(source) = source {
            let mut stmt = self.conn.prepare(
                "SELECT s.id FROM sessions s
                 LEFT JOIN (
                     SELECT session_id, MAX(timestamp) AS last_active
                     FROM messages GROUP BY session_id
                 ) m ON m.session_id = s.id
                 WHERE s.source = ?1
                 ORDER BY COALESCE(m.last_active, s.started_at) DESC, s.started_at DESC, s.id DESC",
            )?;
            let ids = stmt
                .query_map(params![source], |row| row.get::<_, String>(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            ids
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT s.id FROM sessions s
                 LEFT JOIN (
                     SELECT session_id, MAX(timestamp) AS last_active
                     FROM messages GROUP BY session_id
                 ) m ON m.session_id = s.id
                 ORDER BY COALESCE(m.last_active, s.started_at) DESC, s.started_at DESC, s.id DESC",
            )?;
            let ids = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            ids
        };
        let mut sessions = Vec::new();
        for id in ids {
            if let Some(mut session) = self.export_session(&id)? {
                session["last_active"] = json!("<timestamp>");
                sessions.push(session);
            }
        }
        Ok(Value::Array(sessions))
    }

    pub fn list_sessions_for_cli(&self, limit: usize) -> SqlResult<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.source,
                    COALESCE(
                        (SELECT SUBSTR(REPLACE(REPLACE(m.content, X'0A', ' '), X'0D', ' '), 1, 63)
                         FROM messages m
                         WHERE m.session_id = s.id AND m.role = 'user' AND m.content IS NOT NULL
                         ORDER BY m.timestamp, m.id LIMIT 1),
                        ''
                    ) AS preview,
                    s.title
             FROM sessions s
             WHERE s.parent_session_id IS NULL AND s.source != 'tool'
             ORDER BY s.started_at DESC, s.id DESC
             LIMIT ?1",
        )?;
        let sessions = stmt
            .query_map(params![limit as i64], |row| {
                let preview = row.get::<_, Option<String>>(2)?.unwrap_or_default();
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "source": row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    "preview": if preview.chars().count() > 60 {
                        format!("{}...", preview.chars().take(60).collect::<String>())
                    } else {
                        preview
                    },
                    "last_active": "<timestamp>",
                    "title": row.get::<_, Option<String>>(3)?,
                }))
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(sessions)
    }

    pub fn get_session(&self, session_id: &str) -> SqlResult<Option<Value>> {
        self.session_json(session_id)
    }

    pub fn resolve_session_id(&self, session_id_or_prefix: &str) -> SqlResult<Option<String>> {
        if self.get_session(session_id_or_prefix)?.is_some() {
            return Ok(Some(session_id_or_prefix.to_string()));
        }

        let mut stmt = self.conn.prepare("SELECT id FROM sessions")?;
        let matches = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<_>>>()?
            .into_iter()
            .filter(|id| id.starts_with(session_id_or_prefix))
            .take(2)
            .collect::<Vec<_>>();
        Ok((matches.len() == 1).then(|| matches[0].clone()))
    }

    pub fn sanitize_title(title: Option<&str>) -> Result<Option<String>, String> {
        let Some(title) = title else {
            return Ok(None);
        };
        if title.is_empty() {
            return Ok(None);
        }

        let cleaned = title
            .chars()
            .filter(|ch| !is_removed_title_control(*ch))
            .collect::<String>();
        let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            return Ok(None);
        }
        let len = collapsed.chars().count();
        if len > 100 {
            return Err(format!("Title too long ({len} chars, max 100)"));
        }
        Ok(Some(collapsed))
    }

    pub fn set_session_title(&self, session_id: &str, title: Option<&str>) -> Result<bool, String> {
        let title = Self::sanitize_title(title)?;
        if let Some(title) = title.as_deref() {
            let conflict = self
                .conn
                .query_row(
                    "SELECT id FROM sessions WHERE title = ?1 AND id != ?2",
                    params![title, session_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|err| err.to_string())?;
            if let Some(conflict) = conflict {
                return Err(format!(
                    "Title '{title}' is already in use by session {conflict}"
                ));
            }
        }
        let rowcount = self
            .conn
            .execute(
                "UPDATE sessions SET title = ?1 WHERE id = ?2",
                params![title, session_id],
            )
            .map_err(|err| err.to_string())?;
        Ok(rowcount > 0)
    }

    pub fn get_session_title(&self, session_id: &str) -> SqlResult<Option<String>> {
        self.conn
            .query_row(
                "SELECT title FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(|row| row.flatten())
    }

    pub fn get_session_by_title(&self, title: &str) -> SqlResult<Option<Value>> {
        let id = self
            .conn
            .query_row(
                "SELECT id FROM sessions WHERE title = ?1",
                params![title],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        id.map(|id| self.session_json(&id))
            .transpose()
            .map(Option::flatten)
    }

    pub fn end_session(&self, session_id: &str, end_reason: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE sessions
             SET ended_at = '<timestamp>', end_reason = ?1
             WHERE id = ?2 AND ended_at IS NULL",
            params![end_reason, session_id],
        )?;
        Ok(())
    }

    pub fn reopen_session(&self, session_id: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE sessions SET ended_at = NULL, end_reason = NULL WHERE id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    pub fn session_count(&self, source: Option<&str>) -> SqlResult<i64> {
        match source {
            Some(source) => self.conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE source = ?1",
                params![source],
                |row| row.get(0),
            ),
            None => self
                .conn
                .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0)),
        }
    }

    pub fn message_count(&self, session_id: Option<&str>) -> SqlResult<i64> {
        match session_id {
            Some(session_id) => self.conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            ),
            None => self
                .conn
                .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0)),
        }
    }

    pub fn delete_session(&self, session_id: &str, sessions_dir: Option<&Path>) -> SqlResult<bool> {
        let exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE id = ?1",
            params![session_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Ok(false);
        }
        self.conn.execute(
            "UPDATE sessions SET parent_session_id = NULL WHERE parent_session_id = ?1",
            params![session_id],
        )?;
        self.conn.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )?;
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?1", params![session_id])?;
        if let Some(sessions_dir) = sessions_dir {
            remove_session_files(sessions_dir, session_id);
        }
        Ok(true)
    }

    pub fn prune_sessions(
        &self,
        older_than_days: i64,
        source: Option<&str>,
        sessions_dir: Option<&Path>,
    ) -> SqlResult<i64> {
        let cutoff = (current_unix_timestamp() as f64) - (older_than_days as f64 * 86_400.0);
        let session_ids = if let Some(source) = source {
            let mut stmt = self.conn.prepare(
                "SELECT id FROM sessions
                 WHERE CAST(started_at AS REAL) < ?1
                   AND ended_at IS NOT NULL
                   AND source = ?2",
            )?;
            let ids = stmt
                .query_map(params![cutoff, source], |row| row.get::<_, String>(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            ids
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id FROM sessions
                 WHERE CAST(started_at AS REAL) < ?1
                   AND ended_at IS NOT NULL",
            )?;
            let ids = stmt
                .query_map(params![cutoff], |row| row.get::<_, String>(0))?
                .collect::<SqlResult<Vec<_>>>()?;
            ids
        };

        if session_ids.is_empty() {
            return Ok(0);
        }
        for session_id in &session_ids {
            self.conn.execute(
                "UPDATE sessions SET parent_session_id = NULL WHERE parent_session_id = ?1",
                params![session_id],
            )?;
        }
        for session_id in &session_ids {
            self.conn.execute(
                "DELETE FROM messages WHERE session_id = ?1",
                params![session_id],
            )?;
            self.conn
                .execute("DELETE FROM sessions WHERE id = ?1", params![session_id])?;
        }
        if let Some(sessions_dir) = sessions_dir {
            for session_id in &session_ids {
                remove_session_files(sessions_dir, session_id);
            }
        }
        Ok(session_ids.len() as i64)
    }

    #[doc(hidden)]
    pub fn set_session_times_for_test(
        &self,
        session_id: &str,
        started_at: f64,
        ended_at: Option<f64>,
    ) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE sessions SET started_at = ?1, ended_at = ?2 WHERE id = ?3",
            params![started_at, ended_at, session_id],
        )?;
        Ok(())
    }

    pub fn schema_version(&self) -> SqlResult<i64> {
        self.conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
    }

    pub fn table_columns(&self, table: &str) -> SqlResult<Vec<String>> {
        self.existing_columns(table)
    }

    pub fn fts_table_names(&self) -> SqlResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name LIKE 'messages_fts%'
             ORDER BY name",
        )?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(names)
    }

    pub fn fts_match_rows(&self, table: &str, term: &str) -> SqlResult<Value> {
        if !matches!(table, "messages_fts" | "messages_fts_trigram") {
            return Ok(Value::Array(Vec::new()));
        }
        let mut stmt = self.conn.prepare(&format!(
            "SELECT rowid, content FROM {table} WHERE {table} MATCH ?1 ORDER BY rowid"
        ))?;
        let rows = stmt
            .query_map(params![term], |row| {
                Ok(json!({
                    "content": row.get::<_, String>(1)?,
                    "rowid": row.get::<_, i64>(0)?,
                }))
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(Value::Array(rows))
    }

    pub fn sanitize_fts5_query(query: &str) -> String {
        let trimmed = query.trim();
        if trimmed.is_empty() || trimmed.chars().all(|ch| ch == '*') {
            return String::new();
        }
        let quote_count = trimmed.chars().filter(|ch| *ch == '"').count();
        if quote_count >= 2 && quote_count % 2 == 0 {
            return trimmed.to_string();
        }

        let cleaned = trimmed
            .replace('"', "")
            .replace(['+', '(', ')', '{', '}'], "");
        let mut tokens = cleaned
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        while tokens
            .first()
            .is_some_and(|token| is_boolean_operator(token))
        {
            tokens.remove(0);
        }
        while tokens
            .last()
            .is_some_and(|token| is_boolean_operator(token))
        {
            tokens.pop();
        }
        tokens
            .into_iter()
            .map(|token| {
                if token.contains('-') || token.contains('.') {
                    format!("\"{token}\"")
                } else {
                    token
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn search_messages(
        &self,
        query: &str,
        source_filter: &[&str],
        role_filter: &[&str],
        limit: usize,
    ) -> SqlResult<Value> {
        let sanitized = Self::sanitize_fts5_query(query);
        if sanitized.is_empty() || limit == 0 {
            return Ok(Value::Array(Vec::new()));
        }
        let needle = sanitized.trim_matches('"').to_lowercase();
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.content, s.source, m.tool_name, m.tool_calls
             FROM messages m
             JOIN sessions s ON s.id = m.session_id
             ORDER BY m.id DESC",
        )?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            if out.len() >= limit {
                break;
            }
            let id = row.get::<_, i64>(0)?;
            let session_id = row.get::<_, String>(1)?;
            let role = row.get::<_, String>(2)?;
            let content = row.get::<_, Option<String>>(3)?.unwrap_or_default();
            let source = row.get::<_, Option<String>>(4)?.unwrap_or_default();
            let tool_name = row.get::<_, Option<String>>(5)?.unwrap_or_default();
            let tool_calls_raw = row.get::<_, Option<String>>(6)?.unwrap_or_default();
            let tool_calls = pythonish_tool_calls_json(&tool_calls_raw);
            if !source_filter.is_empty() && !source_filter.iter().any(|item| *item == source) {
                continue;
            }
            if !role_filter.is_empty() && !role_filter.iter().any(|item| *item == role) {
                continue;
            }
            let has_tool_fields = !tool_name.is_empty() || !tool_calls.is_empty();
            let searchable = if has_tool_fields {
                format!("{content} {tool_name} {tool_calls}")
            } else {
                content.clone()
            };
            if !searchable.to_lowercase().contains(&needle) {
                continue;
            }
            out.push(json!({
                "context": self.search_context(id, &session_id)?,
                "role": role,
                "session_id": session_id,
                "snippet": snippet(&searchable, &needle, !has_tool_fields),
                "source": source,
            }));
        }
        Ok(Value::Array(out))
    }

    fn search_context(&self, id: i64, session_id: &str) -> SqlResult<Value> {
        let mut stmt = self.conn.prepare(
            "SELECT role, content FROM messages
             WHERE session_id = ?1 AND id BETWEEN ?2 AND ?3
             ORDER BY id ASC",
        )?;
        let messages = stmt
            .query_map(params![session_id, id - 1, id + 1], |row| {
                Ok(json!({
                    "content": row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    "role": row.get::<_, String>(0)?,
                }))
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(Value::Array(messages))
    }

    fn init_schema(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                source TEXT,
                user_id TEXT,
                model TEXT,
                model_config TEXT,
                system_prompt TEXT,
                parent_session_id TEXT,
                started_at TEXT,
                ended_at TEXT,
                end_reason TEXT,
                message_count INTEGER DEFAULT 0,
                tool_call_count INTEGER DEFAULT 0,
                input_tokens INTEGER DEFAULT 0,
                output_tokens INTEGER DEFAULT 0,
                cache_read_tokens INTEGER DEFAULT 0,
                cache_write_tokens INTEGER DEFAULT 0,
                reasoning_tokens INTEGER DEFAULT 0,
                billing_provider TEXT,
                billing_base_url TEXT,
                billing_mode TEXT,
                estimated_cost_usd REAL,
                actual_cost_usd REAL,
                cost_status TEXT,
                cost_source TEXT,
                pricing_version TEXT,
                title TEXT,
                api_call_count INTEGER DEFAULT 0,
                handoff_state TEXT,
                handoff_platform TEXT,
                handoff_error TEXT
            );
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                role TEXT,
                content TEXT,
                tool_call_id TEXT,
                tool_calls TEXT,
                tool_name TEXT,
                timestamp TEXT,
                token_count INTEGER,
                finish_reason TEXT,
                reasoning TEXT,
                reasoning_content TEXT,
                reasoning_details TEXT,
                codex_reasoning_items TEXT,
                codex_message_items TEXT
            );
            ",
        )?;
        self.ensure_schema_columns()?;
        let previous_schema_version = self.current_schema_version()?;
        self.ensure_fts_tables(previous_schema_version)?;
        self.ensure_schema_version(previous_schema_version)?;
        Ok(())
    }

    fn current_schema_version(&self) -> SqlResult<Option<i64>> {
        self.conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get::<_, i64>(0)
            })
            .optional()
    }

    fn ensure_schema_version(&self, current: Option<i64>) -> SqlResult<()> {
        match current {
            Some(version) if version < SESSION_SCHEMA_VERSION => {
                self.conn.execute(
                    "UPDATE schema_version SET version = ?1",
                    params![SESSION_SCHEMA_VERSION],
                )?;
            }
            Some(_) => {}
            None => {
                self.conn.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    params![SESSION_SCHEMA_VERSION],
                )?;
            }
        }
        Ok(())
    }

    fn ensure_fts_tables(&self, previous_schema_version: Option<i64>) -> SqlResult<()> {
        if previous_schema_version.is_some_and(|version| version < 11) {
            for trigger in [
                "messages_fts_insert",
                "messages_fts_delete",
                "messages_fts_update",
                "messages_fts_trigram_insert",
                "messages_fts_trigram_delete",
                "messages_fts_trigram_update",
            ] {
                self.conn
                    .execute(&format!("DROP TRIGGER IF EXISTS {trigger}"), [])?;
            }
            for table in ["messages_fts", "messages_fts_trigram"] {
                self.conn
                    .execute(&format!("DROP TABLE IF EXISTS {table}"), [])?;
            }
        }
        self.conn.execute_batch(
            "
            CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(content);
            CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts_trigram USING fts5(content, tokenize='trigram');
            ",
        )?;
        if previous_schema_version.is_some_and(|version| version < 11) {
            self.conn.execute(
                "INSERT INTO messages_fts(rowid, content)
                 SELECT id,
                        COALESCE(content, '') || ' ' ||
                        COALESCE(tool_name, '') || ' ' ||
                        COALESCE(tool_calls, '')
                 FROM messages",
                [],
            )?;
            self.conn.execute(
                "INSERT INTO messages_fts_trigram(rowid, content)
                 SELECT id,
                        COALESCE(content, '') || ' ' ||
                        COALESCE(tool_name, '') || ' ' ||
                        COALESCE(tool_calls, '')
                 FROM messages",
                [],
            )?;
        }
        Ok(())
    }

    fn ensure_schema_columns(&self) -> SqlResult<()> {
        for (table, columns) in [
            ("sessions", SESSION_COLUMNS.as_slice()),
            ("messages", MESSAGE_COLUMNS.as_slice()),
        ] {
            let existing = self.existing_columns(table)?;
            for (name, definition) in columns {
                if !existing.contains(&name.to_string()) {
                    self.conn.execute(
                        &format!("ALTER TABLE {table} ADD COLUMN {name} {definition}"),
                        [],
                    )?;
                }
            }
        }
        Ok(())
    }

    fn existing_columns(&self, table: &str) -> SqlResult<Vec<String>> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(columns)
    }

    fn session_json(&self, session_id: &str) -> SqlResult<Option<Value>> {
        self.conn
            .query_row(
                "SELECT id, source, user_id, model, model_config, system_prompt,
                        started_at, ended_at, end_reason, parent_session_id, title,
                        message_count, tool_call_count, input_tokens, output_tokens,
                        cache_read_tokens, cache_write_tokens, reasoning_tokens,
                        estimated_cost_usd, actual_cost_usd, cost_status, cost_source,
                        pricing_version, billing_provider, billing_base_url, billing_mode,
                        api_call_count, handoff_state, handoff_platform, handoff_error
                 FROM sessions WHERE id = ?1",
                params![session_id],
                |row| {
                    Ok(json!({
                        "actual_cost_usd": row.get::<_, Option<f64>>(19)?,
                        "api_call_count": row.get::<_, i64>(26)?,
                        "billing_base_url": row.get::<_, Option<String>>(24)?,
                        "billing_mode": row.get::<_, Option<String>>(25)?,
                        "billing_provider": row.get::<_, Option<String>>(23)?,
                        "cache_read_tokens": row.get::<_, i64>(15)?,
                        "cache_write_tokens": row.get::<_, i64>(16)?,
                        "cost_source": row.get::<_, Option<String>>(21)?,
                        "cost_status": row.get::<_, Option<String>>(20)?,
                        "end_reason": row.get::<_, Option<String>>(8)?,
                        "ended_at": row.get::<_, Option<String>>(7)?,
                        "estimated_cost_usd": row.get::<_, Option<f64>>(18)?,
                        "handoff_error": row.get::<_, Option<String>>(29)?,
                        "handoff_platform": row.get::<_, Option<String>>(28)?,
                        "handoff_state": row.get::<_, Option<String>>(27)?,
                        "id": row.get::<_, String>(0)?,
                        "input_tokens": row.get::<_, i64>(13)?,
                        "message_count": row.get::<_, i64>(11)?,
                        "model": row.get::<_, Option<String>>(3)?,
                        "model_config": row.get::<_, Option<String>>(4)?,
                        "output_tokens": row.get::<_, i64>(14)?,
                        "parent_session_id": row.get::<_, Option<String>>(9)?,
                        "pricing_version": row.get::<_, Option<String>>(22)?,
                        "reasoning_tokens": row.get::<_, i64>(17)?,
                        "source": row.get::<_, Option<String>>(1)?,
                        "started_at": row.get::<_, Option<String>>(6)?,
                        "system_prompt": row.get::<_, Option<String>>(5)?,
                        "title": row.get::<_, Option<String>>(10)?,
                        "tool_call_count": row.get::<_, i64>(12)?,
                        "user_id": row.get::<_, Option<String>>(2)?,
                    }))
                },
            )
            .optional()
    }

    fn messages_json(&self, session_id: &str) -> SqlResult<Value> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, role, content, tool_call_id, tool_calls, tool_name,
                    timestamp, token_count, finish_reason, reasoning, reasoning_content,
                    reasoning_details, codex_reasoning_items, codex_message_items
             FROM messages WHERE session_id = ?1 ORDER BY id",
        )?;
        let messages = stmt
            .query_map(params![session_id], |row| {
                let tool_calls: Option<String> = row.get(5)?;
                let tool_calls = tool_calls
                    .as_deref()
                    .map(|text| serde_json::from_str(text).unwrap_or(Value::Null))
                    .unwrap_or(Value::Null);
                Ok(json!({
                    "codex_message_items": row.get::<_, Option<String>>(14)?,
                    "codex_reasoning_items": row.get::<_, Option<String>>(13)?,
                    "content": row.get::<_, Option<String>>(3)?,
                    "finish_reason": row.get::<_, Option<String>>(9)?,
                    "id": row.get::<_, i64>(0)?,
                    "reasoning": row.get::<_, Option<String>>(10)?,
                    "reasoning_content": row.get::<_, Option<String>>(11)?,
                    "reasoning_details": row.get::<_, Option<String>>(12)?,
                    "role": row.get::<_, String>(2)?,
                    "session_id": row.get::<_, String>(1)?,
                    "timestamp": row.get::<_, Option<String>>(7)?,
                    "token_count": row.get::<_, Option<i64>>(8)?,
                    "tool_call_id": row.get::<_, Option<String>>(4)?,
                    "tool_calls": tool_calls,
                    "tool_name": row.get::<_, Option<String>>(6)?,
                }))
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        Ok(Value::Array(messages))
    }
}

fn is_boolean_operator(token: &str) -> bool {
    matches!(token.to_ascii_uppercase().as_str(), "AND" | "OR" | "NOT")
}

fn is_removed_title_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{0000}'..='\u{0008}'
            | '\u{000b}'
            | '\u{000c}'
            | '\u{000e}'..='\u{001f}'
            | '\u{007f}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{2069}'
            | '\u{feff}'
            | '\u{fffc}'
            | '\u{fff9}'..='\u{fffb}'
    )
}

fn remove_session_files(sessions_dir: &Path, session_id: &str) {
    let _ = fs::remove_file(sessions_dir.join(format!("{session_id}.json")));
    let _ = fs::remove_file(sessions_dir.join(format!("{session_id}.jsonl")));
    let Ok(entries) = fs::read_dir(sessions_dir) else {
        return;
    };
    let prefix = format!("request_dump_{session_id}_");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) && name.ends_with(".json") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn snippet(content: &str, needle: &str, append_padding: bool) -> String {
    let content_lower = content.to_lowercase();
    let Some(start) = content_lower.find(needle) else {
        return if append_padding {
            format!("{content}  ")
        } else {
            content.to_string()
        };
    };
    let end = start + needle.len();
    let snippet = format!(
        "{}>>>{}<<<{}  ",
        &content[..start],
        &content[start..end],
        &content[end..]
    );
    if append_padding {
        snippet
    } else {
        snippet.strip_suffix("  ").unwrap_or(&snippet).to_string()
    }
}

fn pythonish_tool_calls_json(raw: &str) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return raw.to_string();
    };
    pythonish_json(&value)
}

fn pythonish_json(value: &Value) -> String {
    match value {
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(pythonish_json)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(object) => {
            let mut keys = Vec::new();
            for preferred in ["id", "type", "function", "name", "arguments"] {
                if object.contains_key(preferred) {
                    keys.push(preferred.to_string());
                }
            }
            for key in object.keys() {
                if !keys.iter().any(|existing| existing == key) {
                    keys.push(key.clone());
                }
            }
            format!(
                "{{{}}}",
                keys.iter()
                    .filter_map(|key| {
                        object.get(key).map(|value| {
                            format!(
                                "{}: {}",
                                serde_json::to_string(key).unwrap(),
                                pythonish_json(value)
                            )
                        })
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        _ => serde_json::to_string(value).unwrap(),
    }
}

const SESSION_COLUMNS: [(&str, &str); 30] = [
    ("id", "TEXT PRIMARY KEY"),
    ("source", "TEXT"),
    ("user_id", "TEXT"),
    ("model", "TEXT"),
    ("model_config", "TEXT"),
    ("system_prompt", "TEXT"),
    ("parent_session_id", "TEXT"),
    ("started_at", "TEXT"),
    ("ended_at", "TEXT"),
    ("end_reason", "TEXT"),
    ("message_count", "INTEGER DEFAULT 0"),
    ("tool_call_count", "INTEGER DEFAULT 0"),
    ("input_tokens", "INTEGER DEFAULT 0"),
    ("output_tokens", "INTEGER DEFAULT 0"),
    ("cache_read_tokens", "INTEGER DEFAULT 0"),
    ("cache_write_tokens", "INTEGER DEFAULT 0"),
    ("reasoning_tokens", "INTEGER DEFAULT 0"),
    ("billing_provider", "TEXT"),
    ("billing_base_url", "TEXT"),
    ("billing_mode", "TEXT"),
    ("estimated_cost_usd", "REAL"),
    ("actual_cost_usd", "REAL"),
    ("cost_status", "TEXT"),
    ("cost_source", "TEXT"),
    ("pricing_version", "TEXT"),
    ("title", "TEXT"),
    ("api_call_count", "INTEGER DEFAULT 0"),
    ("handoff_state", "TEXT"),
    ("handoff_platform", "TEXT"),
    ("handoff_error", "TEXT"),
];

const MESSAGE_COLUMNS: [(&str, &str); 15] = [
    ("id", "INTEGER PRIMARY KEY AUTOINCREMENT"),
    ("session_id", "TEXT"),
    ("role", "TEXT"),
    ("content", "TEXT"),
    ("tool_call_id", "TEXT"),
    ("tool_calls", "TEXT"),
    ("tool_name", "TEXT"),
    ("timestamp", "TEXT"),
    ("token_count", "INTEGER"),
    ("finish_reason", "TEXT"),
    ("reasoning", "TEXT"),
    ("reasoning_content", "TEXT"),
    ("reasoning_details", "TEXT"),
    ("codex_reasoning_items", "TEXT"),
    ("codex_message_items", "TEXT"),
];

const SESSION_SCHEMA_VERSION: i64 = 11;

#[cfg(test)]
mod sqlite_tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn sqlite_export_matches_synthetic_store() {
        let db = SqliteSessionStore::open_in_memory().unwrap();
        db.create_session(
            "parity-session-1",
            "cli",
            "user-1",
            "fake/model",
            "{\"provider\": \"fake\"}",
            "system prompt",
        )
        .unwrap();
        db.append_message("parity-session-1", "user", "hello", None, None, None, None)
            .unwrap();
        db.append_message(
            "parity-session-1",
            "assistant",
            "calling tool",
            Some("tool_calls"),
            None,
            Some(&json!([
                {
                    "function": {"arguments": "{}", "name": "memory"},
                    "id": "call-1",
                    "type": "function"
                }
            ])),
            None,
        )
        .unwrap();
        db.append_message(
            "parity-session-1",
            "tool",
            "{\"success\": true}",
            None,
            Some("call-1"),
            None,
            Some("memory"),
        )
        .unwrap();

        let synthetic = SessionStore::parity_fixture_store();
        assert_eq!(
            db.export_session("parity-session-1").unwrap().unwrap(),
            synthetic.export_session()
        );
        assert_eq!(
            db.resume_conversation("parity-session-1").unwrap(),
            synthetic.resume_conversation()
        );
    }

    #[test]
    fn opening_legacy_schema_preserves_rows_and_adds_missing_columns() {
        let path = std::env::temp_dir().join(format!(
            "hermes-session-legacy-{}.db",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "
                CREATE TABLE sessions (
                    id TEXT PRIMARY KEY,
                    source TEXT,
                    user_id TEXT,
                    model TEXT,
                    model_config TEXT,
                    system_prompt TEXT,
                    started_at TEXT
                );
                CREATE TABLE messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT,
                    role TEXT,
                    content TEXT,
                    timestamp TEXT
                );
                INSERT INTO sessions (
                    id, source, user_id, model, model_config, system_prompt, started_at
                ) VALUES (
                    'legacy-session', 'cli', 'user-legacy', 'fake/model',
                    '{\"provider\":\"fake\"}', 'system prompt', '<timestamp>'
                );
                INSERT INTO messages (session_id, role, content, timestamp)
                VALUES ('legacy-session', 'user', 'legacy hello', '<timestamp>');
                ",
            )
            .unwrap();
        }

        let db = SqliteSessionStore::open(&path).unwrap();
        let exported = db.export_session("legacy-session").unwrap().unwrap();
        assert_eq!(exported["id"], "legacy-session");
        assert_eq!(exported["message_count"], 0);
        assert_eq!(exported["tool_call_count"], 0);
        assert_eq!(exported["messages"][0]["content"], "legacy hello");
        assert_eq!(db.schema_version().unwrap(), SESSION_SCHEMA_VERSION);
        assert!(db
            .fts_table_names()
            .unwrap()
            .contains(&"messages_fts".to_string()));
        assert_eq!(
            db.resume_conversation("legacy-session").unwrap(),
            json!([{"content": "legacy hello", "role": "user"}])
        );

        SqliteSessionStore::open(&path).unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn concurrent_session_writes_preserve_messages() {
        let path = std::env::temp_dir().join(format!(
            "hermes-session-concurrent-{}-{}.db",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let session_id = "concurrent-session";
        {
            let db = SqliteSessionStore::open(&path).unwrap();
            db.create_session(
                session_id,
                "cli",
                "user-concurrent",
                "fake/model",
                "{\"provider\":\"fake\"}",
                "system prompt",
            )
            .unwrap();
        }

        let mut handles = Vec::new();
        for worker in 0..4 {
            let path = path.clone();
            handles.push(thread::spawn(move || {
                let db = SqliteSessionStore::open(&path).unwrap();
                for idx in 0..10 {
                    let content = format!("worker {worker} message {idx}");
                    let mut attempts = 0;
                    loop {
                        match db
                            .append_message(session_id, "user", &content, None, None, None, None)
                        {
                            Ok(_) => break,
                            Err(err) if attempts < 20 => {
                                attempts += 1;
                                eprintln!("sqlite append retry after {err}");
                                thread::sleep(Duration::from_millis(5));
                            }
                            Err(err) => panic!("append failed after retries: {err}"),
                        }
                    }
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }

        let db = SqliteSessionStore::open(&path).unwrap();
        assert_eq!(db.message_count(Some(session_id)).unwrap(), 40);
        let exported = db.export_session(session_id).unwrap().unwrap();
        assert_eq!(exported["message_count"], 40);
        assert_eq!(exported["messages"].as_array().unwrap().len(), 40);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }
}
