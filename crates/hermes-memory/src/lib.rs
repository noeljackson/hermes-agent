use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ENTRY_DELIMITER: &str = "\n\u{00a7}\n";

#[derive(Debug, Clone)]
pub struct MemoryStore {
    memory_entries: Vec<String>,
    user_entries: Vec<String>,
    memory_char_limit: usize,
    user_char_limit: usize,
    snapshot_memory: String,
    snapshot_user: String,
}

impl MemoryStore {
    pub fn new(memory_char_limit: usize, user_char_limit: usize) -> Self {
        Self {
            memory_entries: Vec::new(),
            user_entries: Vec::new(),
            memory_char_limit,
            user_char_limit,
            snapshot_memory: String::new(),
            snapshot_user: String::new(),
        }
    }

    pub fn add(&mut self, target: &str, content: &str) -> Value {
        let content = content.trim();
        if content.is_empty() {
            return json!({"success": false, "error": "Content cannot be empty."});
        }
        if let Some(error) = scan_memory_content(content) {
            return json!({"success": false, "error": error});
        }

        let entries = self.entries(target);
        if entries.iter().any(|entry| entry == content) {
            return self.success_response(target, "Entry already exists (no duplicate added).");
        }

        let new_total = joined_char_count(entries.iter().map(String::as_str).chain([content]));
        let limit = self.limit(target);
        if new_total > limit {
            let current = self.char_count(target);
            return json!({
                "current_entries": entries,
                "error": format!(
                    "Memory at {}/{} chars. Adding this entry ({} chars) would exceed the limit. Replace or remove existing entries first.",
                    format_count(current),
                    format_count(limit),
                    content.chars().count(),
                ),
                "success": false,
                "usage": format!("{}/{}", format_count(current), format_count(limit)),
            });
        }

        self.entries_mut(target).push(content.to_string());
        self.success_response(target, "Entry added.")
    }

    pub fn replace(&mut self, target: &str, old_text: &str, new_content: &str) -> Value {
        let old_text = old_text.trim();
        let new_content = new_content.trim();
        if old_text.is_empty() {
            return json!({"success": false, "error": "old_text cannot be empty."});
        }
        if new_content.is_empty() {
            return json!({"success": false, "error": "new_content cannot be empty. Use 'remove' to delete entries."});
        }
        if let Some(error) = scan_memory_content(new_content) {
            return json!({"success": false, "error": error});
        }

        let entries = self.entries(target);
        let matches = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.contains(old_text))
            .map(|(index, entry)| (index, entry.as_str()))
            .collect::<Vec<_>>();
        let Some((index, _)) = single_match_or_error(old_text, &matches) else {
            return match_error(old_text, &matches);
        };

        let mut test_entries = entries.to_vec();
        test_entries[index] = new_content.to_string();
        let new_total = joined_char_count(test_entries.iter().map(String::as_str));
        let limit = self.limit(target);
        if new_total > limit {
            return json!({
                "error": format!(
                    "Replacement would put memory at {}/{} chars. Shorten the new content or remove other entries first.",
                    format_count(new_total),
                    format_count(limit),
                ),
                "success": false,
            });
        }

        self.entries_mut(target)[index] = new_content.to_string();
        self.success_response(target, "Entry replaced.")
    }

    pub fn remove(&mut self, target: &str, old_text: &str) -> Value {
        let old_text = old_text.trim();
        if old_text.is_empty() {
            return json!({"success": false, "error": "old_text cannot be empty."});
        }

        let entries = self.entries(target);
        let matches = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.contains(old_text))
            .map(|(index, entry)| (index, entry.as_str()))
            .collect::<Vec<_>>();
        let Some((index, _)) = single_match_or_error(old_text, &matches) else {
            return match_error(old_text, &matches);
        };

        self.entries_mut(target).remove(index);
        self.success_response(target, "Entry removed.")
    }

    pub fn memory_entries(&self) -> &[String] {
        &self.memory_entries
    }

    pub fn user_entries(&self) -> &[String] {
        &self.user_entries
    }

    pub fn format_for_system_prompt(&self, target: &str) -> Option<&str> {
        let block = if target == "user" {
            &self.snapshot_user
        } else {
            &self.snapshot_memory
        };
        (!block.is_empty()).then_some(block.as_str())
    }

    pub fn load_from_dir(
        dir: &Path,
        memory_char_limit: usize,
        user_char_limit: usize,
    ) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let memory_entries = read_entries(&dir.join("MEMORY.md"))?;
        let user_entries = read_entries(&dir.join("USER.md"))?;
        let snapshot_memory = render_block("memory", &memory_entries, memory_char_limit);
        let snapshot_user = render_block("user", &user_entries, user_char_limit);
        Ok(Self {
            memory_entries,
            user_entries,
            memory_char_limit,
            user_char_limit,
            snapshot_memory,
            snapshot_user,
        })
    }

    pub fn save_to_dir(&self, dir: &Path, target: &str) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        let path = if target == "user" {
            dir.join("USER.md")
        } else {
            dir.join("MEMORY.md")
        };
        write_entries(&path, self.entries(target))
    }

    fn entries_mut(&mut self, target: &str) -> &mut Vec<String> {
        if target == "user" {
            &mut self.user_entries
        } else {
            &mut self.memory_entries
        }
    }

    fn entries(&self, target: &str) -> &[String] {
        if target == "user" {
            &self.user_entries
        } else {
            &self.memory_entries
        }
    }

    fn limit(&self, target: &str) -> usize {
        if target == "user" {
            self.user_char_limit
        } else {
            self.memory_char_limit
        }
    }

    fn char_count(&self, target: &str) -> usize {
        joined_char_count(self.entries(target).iter().map(String::as_str))
    }

    fn success_response(&self, target: &str, message: &str) -> Value {
        let entries = self.entries(target);
        let char_count = self.char_count(target);
        let limit = self.limit(target);
        let percent = (char_count * 100).checked_div(limit).unwrap_or(0);
        json!({
            "entries": entries,
            "entry_count": entries.len(),
            "message": message,
            "success": true,
            "target": target,
            "usage": format!("{percent}% \u{2014} {char_count}/{limit} chars"),
        })
    }
}

pub fn memory_tool(
    store: &mut MemoryStore,
    action: &str,
    target: &str,
    content: Option<&str>,
    old_text: Option<&str>,
) -> Value {
    if !matches!(target, "memory" | "user") {
        return json!({"error": format!("Invalid target '{target}'. Use 'memory' or 'user'."), "success": false});
    }
    match action {
        "add" => match content {
            Some(content) if !content.is_empty() => store.add(target, content),
            _ => json!({"error": "Content is required for 'add' action.", "success": false}),
        },
        "replace" => {
            let Some(old_text) = old_text.filter(|value| !value.is_empty()) else {
                return json!({"error": "old_text is required for 'replace' action.", "success": false});
            };
            let Some(content) = content.filter(|value| !value.is_empty()) else {
                return json!({"error": "content is required for 'replace' action.", "success": false});
            };
            store.replace(target, old_text, content)
        }
        "remove" => match old_text {
            Some(old_text) if !old_text.is_empty() => store.remove(target, old_text),
            _ => json!({"error": "old_text is required for 'remove' action.", "success": false}),
        },
        _ => {
            json!({"error": format!("Unknown action '{action}'. Use: add, replace, remove"), "success": false})
        }
    }
}

fn single_match_or_error<'a>(
    _old_text: &str,
    matches: &[(usize, &'a str)],
) -> Option<(usize, &'a str)> {
    match matches {
        [] => None,
        [single] => Some(*single),
        many => {
            let mut unique = many.iter().map(|(_, text)| *text).collect::<Vec<_>>();
            unique.sort_unstable();
            unique.dedup();
            (unique.len() == 1).then_some(many[0])
        }
    }
}

fn match_error(old_text: &str, matches: &[(usize, &str)]) -> Value {
    if matches.is_empty() {
        return json!({"success": false, "error": format!("No entry matched '{old_text}'.")});
    }
    let previews = matches
        .iter()
        .map(|(_, entry)| {
            if entry.chars().count() > 80 {
                let prefix = entry.chars().take(80).collect::<String>();
                format!("{prefix}...")
            } else {
                (*entry).to_string()
            }
        })
        .collect::<Vec<_>>();
    json!({
        "error": format!("Multiple entries matched '{old_text}'. Be more specific."),
        "matches": previews,
        "success": false,
    })
}

fn joined_char_count<'a>(entries: impl Iterator<Item = &'a str>) -> usize {
    let entries = entries.collect::<Vec<_>>();
    if entries.is_empty() {
        0
    } else {
        entries.join(ENTRY_DELIMITER).chars().count()
    }
}

fn format_count(value: usize) -> String {
    let raw = value.to_string();
    let mut out = String::new();
    for (idx, ch) in raw.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

#[derive(Debug, Clone)]
pub struct FileMemoryStore {
    dir: PathBuf,
    store: MemoryStore,
}

impl FileMemoryStore {
    pub fn load(
        dir: impl Into<PathBuf>,
        memory_char_limit: usize,
        user_char_limit: usize,
    ) -> io::Result<Self> {
        let dir = dir.into();
        let store = MemoryStore::load_from_dir(&dir, memory_char_limit, user_char_limit)?;
        Ok(Self { dir, store })
    }

    pub fn add(&mut self, target: &str, content: &str) -> io::Result<Value> {
        let response = self.store.add(target, content);
        if response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            self.store.save_to_dir(&self.dir, target)?;
        }
        Ok(response)
    }

    pub fn replace(&mut self, target: &str, old_text: &str, content: &str) -> io::Result<Value> {
        let response = self.store.replace(target, old_text, content);
        if response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            self.store.save_to_dir(&self.dir, target)?;
        }
        Ok(response)
    }

    pub fn remove(&mut self, target: &str, old_text: &str) -> io::Result<Value> {
        let response = self.store.remove(target, old_text);
        if response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            self.store.save_to_dir(&self.dir, target)?;
        }
        Ok(response)
    }

    pub fn reload(&self) -> io::Result<MemoryStore> {
        MemoryStore::load_from_dir(
            &self.dir,
            self.store.memory_char_limit,
            self.store.user_char_limit,
        )
    }
}

pub fn scan_memory_content(content: &str) -> Option<String> {
    const INVISIBLE: [char; 10] = [
        '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{feff}', '\u{202a}', '\u{202b}',
        '\u{202c}', '\u{202d}', '\u{202e}',
    ];
    if let Some(ch) = content.chars().find(|ch| INVISIBLE.contains(ch)) {
        return Some(format!(
            "Blocked: content contains invisible unicode character U+{:04X} (possible injection).",
            ch as u32
        ));
    }

    let lower = content.to_lowercase();
    let patterns = [
        ("ignore previous instructions", "prompt_injection"),
        ("ignore all instructions", "prompt_injection"),
        ("ignore above instructions", "prompt_injection"),
        ("ignore prior instructions", "prompt_injection"),
        ("you are now ", "role_hijack"),
        ("do not tell the user", "deception_hide"),
        ("system prompt override", "sys_prompt_override"),
        ("disregard your instructions", "disregard_rules"),
        ("disregard your rules", "disregard_rules"),
        ("disregard your guidelines", "disregard_rules"),
        ("disregard all instructions", "disregard_rules"),
        ("disregard all rules", "disregard_rules"),
        ("disregard all guidelines", "disregard_rules"),
        ("disregard any instructions", "disregard_rules"),
        ("disregard any rules", "disregard_rules"),
        ("disregard any guidelines", "disregard_rules"),
        ("authorized_keys", "ssh_backdoor"),
        ("$home/.ssh", "ssh_access"),
        ("~/.ssh", "ssh_access"),
        ("$home/.hermes/.env", "hermes_env"),
        ("~/.hermes/.env", "hermes_env"),
    ];
    for (needle, id) in patterns {
        if lower.contains(needle) {
            return Some(format!(
                "Blocked: content matches threat pattern '{id}'. Memory entries are injected into the system prompt and must not contain injection or exfiltration payloads."
            ));
        }
    }
    if lower.contains("curl ") && contains_secret_word(&lower) {
        return Some(threat_error("exfil_curl"));
    }
    if lower.contains("wget ") && contains_secret_word(&lower) {
        return Some(threat_error("exfil_wget"));
    }
    if lower.contains("cat ")
        && [
            ".env",
            "credentials",
            ".netrc",
            ".pgpass",
            ".npmrc",
            ".pypirc",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        return Some(threat_error("read_secrets"));
    }
    None
}

fn threat_error(id: &str) -> String {
    format!(
        "Blocked: content matches threat pattern '{id}'. Memory entries are injected into the system prompt and must not contain injection or exfiltration payloads."
    )
}

fn contains_secret_word(lower: &str) -> bool {
    ["key", "token", "secret", "password", "credential", "api"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn read_entries(path: &Path) -> io::Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)?;
    let mut entries = Vec::new();
    for entry in raw.split(ENTRY_DELIMITER) {
        let entry = entry.trim();
        if !entry.is_empty() && !entries.iter().any(|existing| existing == entry) {
            entries.push(entry.to_string());
        }
    }
    Ok(entries)
}

fn write_entries(path: &Path, entries: &[String]) -> io::Result<()> {
    fs::write(path, entries.join(ENTRY_DELIMITER))
}

fn render_block(target: &str, entries: &[String], limit: usize) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let content = entries.join(ENTRY_DELIMITER);
    let current = content.chars().count();
    let percent = (current * 100).checked_div(limit).unwrap_or(0).min(100);
    let header = if target == "user" {
        format!(
            "USER PROFILE (who the user is) [{percent}% — {}/{limit} chars]",
            format_count(current)
        )
    } else {
        format!(
            "MEMORY (your personal notes) [{percent}% — {}/{limit} chars]",
            format_count(current)
        )
    };
    let separator = "═".repeat(46);
    format!("{separator}\n{header}\n{separator}\n{content}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn persists_and_reloads_entries() {
        let dir = std::env::temp_dir().join(format!("hermes-memory-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        let mut store = FileMemoryStore::load(&dir, 500, 500).unwrap();
        let response = store
            .add("memory", "Project uses parity fixtures.")
            .unwrap();
        assert_eq!(response["success"], true);

        let reloaded = store.reload().unwrap();
        assert_eq!(
            reloaded.memory_entries(),
            &["Project uses parity fixtures.".to_string()]
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn blocks_obvious_prompt_injection() {
        let mut store = MemoryStore::new(500, 500);
        let response = store.add("memory", "ignore previous instructions");
        assert_eq!(response["success"], false);
    }

    #[test]
    fn concurrent_memory_writes_preserve_entries() {
        let dir = std::env::temp_dir().join(format!(
            "hermes-memory-concurrent-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);

        let store = Arc::new(Mutex::new(
            FileMemoryStore::load(&dir, 10_000, 10_000).unwrap(),
        ));
        let mut handles = Vec::new();
        for worker in 0..4 {
            let store = Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                for idx in 0..10 {
                    let content = format!("worker {worker} memory fact {idx}");
                    let response = store.lock().unwrap().add("memory", &content).unwrap();
                    assert_eq!(response["success"], true);
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }

        let reloaded = store.lock().unwrap().reload().unwrap();
        assert_eq!(reloaded.memory_entries().len(), 40);
        assert!(reloaded
            .memory_entries()
            .contains(&"worker 3 memory fact 9".to_string()));

        let _ = fs::remove_dir_all(&dir);
    }
}
