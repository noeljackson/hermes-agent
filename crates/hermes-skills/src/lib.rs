use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillCommand {
    pub command: String,
    pub name: String,
    pub description: String,
    pub skill_md_basename: String,
}

pub fn parse_skill_command(skill_md: &str) -> Option<SkillCommand> {
    let (frontmatter, body) = frontmatter_and_body(skill_md)?;
    if !platform_matches(frontmatter) {
        return None;
    }
    let name = frontmatter_value(frontmatter, "name")?;
    let description = frontmatter_value(frontmatter, "description")
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| fallback_description(body));
    let command = format!("/{}", slugify(&name));
    if command == "/" {
        return None;
    }
    Some(SkillCommand {
        command,
        name,
        description,
        skill_md_basename: "SKILL.md".to_string(),
    })
}

pub fn scan_skill_commands(root: impl AsRef<Path>) -> io::Result<Vec<SkillCommand>> {
    let mut commands = Vec::new();
    let mut seen_names = BTreeSet::new();
    scan_dir(root.as_ref(), &mut commands, &mut seen_names)?;
    commands.sort_by(|left, right| left.command.cmp(&right.command));
    Ok(commands)
}

pub fn command_map_json(commands: &[SkillCommand]) -> Value {
    let mut map = serde_json::Map::new();
    for command in commands {
        map.insert(
            command.command.clone(),
            json!({
                "description": command.description,
                "name": command.name,
                "skill_md_basename": command.skill_md_basename,
            }),
        );
    }
    Value::Object(map)
}

pub fn skills_list_json(root: impl AsRef<Path>, category: Option<&str>) -> io::Result<Value> {
    let root = root.as_ref();
    if !root.exists() {
        fs::create_dir_all(root)?;
        return Ok(json!({
            "categories": [],
            "message": "No skills found. Skills directory created at <HERMES_HOME>/skills/",
            "skills": [],
            "success": true,
        }));
    }

    let mut skills = Vec::new();
    let mut seen_names = BTreeSet::new();
    collect_skill_list_items(root, root, &mut skills, &mut seen_names)?;
    if let Some(category) = category.filter(|value| !value.is_empty()) {
        skills.retain(|skill| skill["category"].as_str() == Some(category));
    }
    skills.sort_by(|left, right| {
        let left_key = (
            left["category"].as_str().unwrap_or_default(),
            left["name"].as_str().unwrap_or_default(),
        );
        let right_key = (
            right["category"].as_str().unwrap_or_default(),
            right["name"].as_str().unwrap_or_default(),
        );
        left_key.cmp(&right_key)
    });

    if skills.is_empty() && category.is_none() {
        return Ok(json!({
            "categories": [],
            "message": "No skills found in skills/ directory.",
            "skills": [],
            "success": true,
        }));
    }

    let categories = skills
        .iter()
        .filter_map(|skill| skill["category"].as_str().map(str::to_string))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(json!({
        "categories": categories,
        "count": skills.len(),
        "hint": "Use skill_view(name) to see full content, tags, and linked files",
        "skills": skills,
        "success": true,
    }))
}

pub fn resolve_skill_command_key<'a>(
    command: &str,
    commands: &'a BTreeMap<String, SkillCommand>,
) -> Option<&'a str> {
    if command.is_empty() {
        return None;
    }
    let key = format!("/{}", command.replace('_', "-"));
    commands.get_key_value(&key).map(|(key, _)| key.as_str())
}

pub fn reload_diff(
    before: &BTreeMap<String, SkillCommand>,
    after: &BTreeMap<String, SkillCommand>,
) -> Value {
    let before_names = snapshot_names(before);
    let after_names = snapshot_names(after);
    let added_names = after_names
        .keys()
        .filter(|name| !before_names.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();
    let removed_names = before_names
        .keys()
        .filter(|name| !after_names.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();
    let unchanged = after_names
        .keys()
        .filter(|name| before_names.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();
    json!({
        "added": added_names
            .iter()
            .map(|name| json!({"description": after_names[name], "name": name}))
            .collect::<Vec<_>>(),
        "commands": after.len(),
        "removed": removed_names
            .iter()
            .map(|name| json!({"description": before_names[name], "name": name}))
            .collect::<Vec<_>>(),
        "total": after_names.len(),
        "unchanged": unchanged,
    })
}

pub fn build_skill_invocation_message_from_dir(
    hermes_home: impl AsRef<Path>,
    skill_dir: impl AsRef<Path>,
    user_instruction: &str,
    runtime_note: &str,
) -> io::Result<Option<String>> {
    let hermes_home = hermes_home.as_ref();
    let skill_dir = skill_dir.as_ref();
    let skill_md = skill_dir.join("SKILL.md");
    let raw_content = fs::read_to_string(&skill_md)?;
    let Some((frontmatter, _body)) = frontmatter_and_body(&raw_content) else {
        return Ok(None);
    };
    let Some(skill_name) = frontmatter_value(frontmatter, "name") else {
        return Ok(None);
    };

    let activation_note = format!(
        "[IMPORTANT: The user has invoked the \"{skill_name}\" skill, indicating they want you to follow its instructions. The full skill content is loaded below.]"
    );
    let mut parts = vec![
        activation_note,
        String::new(),
        raw_content.trim().to_string(),
    ];

    let skill_dir_display = skill_dir.to_string_lossy().to_string();
    parts.push(String::new());
    parts.push(format!("[Skill directory: {skill_dir_display}]"));
    parts.push(
        "Resolve any relative paths in this skill (e.g. `scripts/foo.js`, `templates/config.yaml`) against that directory, then run them with the terminal tool using the absolute path."
            .to_string(),
    );

    let supporting = supporting_files(skill_dir)?;
    if !supporting.is_empty() {
        let skill_view_target = skill_dir
            .strip_prefix(hermes_home.join("skills"))
            .ok()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                skill_dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string()
            });
        parts.push(String::new());
        parts.push("[This skill has supporting files:]".to_string());
        for file in &supporting {
            parts.push(format!("- {file}  ->  {skill_dir_display}/{file}"));
        }
        parts.push(format!(
            "\nLoad any of these with skill_view(name=\"{skill_view_target}\", file_path=\"<path>\"), or run scripts directly by absolute path (e.g. `node {skill_dir_display}/scripts/foo.js`)."
        ));
    }

    if !user_instruction.is_empty() {
        parts.push(String::new());
        parts.push(format!(
            "The user has provided the following instruction alongside the skill invocation: {user_instruction}"
        ));
    }
    if !runtime_note.is_empty() {
        parts.push(String::new());
        parts.push(format!("[Runtime note: {runtime_note}]"));
    }

    Ok(Some(parts.join("\n")))
}

fn frontmatter_and_body(skill_md: &str) -> Option<(&str, &str)> {
    let rest = skill_md.strip_prefix("---\n")?;
    let (fm, body) = rest.split_once("\n---")?;
    Some((fm, body.trim_start_matches(['\r', '\n'])))
}

fn snapshot_names(commands: &BTreeMap<String, SkillCommand>) -> BTreeMap<String, String> {
    commands
        .iter()
        .map(|(key, command)| {
            (
                key.trim_start_matches('/').to_string(),
                command.description.clone(),
            )
        })
        .collect()
}

fn frontmatter_value(frontmatter: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix(&prefix) {
            return Some(value.trim().trim_matches(['"', '\'']).to_string());
        }
    }
    None
}

fn fallback_description(body: &str) -> String {
    for line in body.lines() {
        let line = line.trim();
        if !line.is_empty() && !line.starts_with('#') {
            return line.chars().take(80).collect();
        }
    }
    String::new()
}

fn platform_matches(frontmatter: &str) -> bool {
    let Some(platforms) = frontmatter_value(frontmatter, "platforms") else {
        return true;
    };
    let normalized = platforms.to_lowercase();
    if normalized.trim().is_empty() {
        return true;
    }
    let current = match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        _ => "linux",
    };
    normalized
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| token == current)
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut last_hyphen = false;
    for ch in name.to_lowercase().chars() {
        let next = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else if ch == ' ' || ch == '_' || ch == '-' {
            Some('-')
        } else {
            None
        };
        if let Some(ch) = next {
            if ch == '-' {
                if !last_hyphen && !out.is_empty() {
                    out.push(ch);
                    last_hyphen = true;
                }
            } else {
                out.push(ch);
                last_hyphen = false;
            }
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn scan_dir(
    dir: &Path,
    commands: &mut Vec<SkillCommand>,
    seen_names: &mut BTreeSet<String>,
) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path_has_ignored_component(&path) {
            continue;
        }
        if path.is_dir() {
            scan_dir(&path, commands, seen_names)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
            let skill_md = fs::read_to_string(&path)?;
            if let Some(mut command) = parse_skill_command(&skill_md) {
                if !seen_names.insert(command.name.clone()) {
                    continue;
                }
                command.skill_md_basename = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("SKILL.md")
                    .to_string();
                commands.push(command);
            }
        }
    }
    Ok(())
}

fn collect_skill_list_items(
    root: &Path,
    dir: &Path,
    skills: &mut Vec<Value>,
    seen_names: &mut BTreeSet<String>,
) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path_has_ignored_component(&path) {
            continue;
        }
        if path.is_dir() {
            collect_skill_list_items(root, &path, skills, seen_names)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
            let content = fs::read_to_string(&path)?;
            let Some((frontmatter, body)) = frontmatter_and_body(&content) else {
                continue;
            };
            if !platform_matches(frontmatter) {
                continue;
            }
            let fallback_name = path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            let name = frontmatter_value(frontmatter, "name")
                .unwrap_or(fallback_name)
                .chars()
                .take(64)
                .collect::<String>();
            if !seen_names.insert(name.clone()) {
                continue;
            }
            let mut description = frontmatter_value(frontmatter, "description")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| fallback_description(body));
            if description.chars().count() > 1024 {
                description = format!("{}...", description.chars().take(1021).collect::<String>());
            }
            skills.push(json!({
                "category": category_from_skill_path(root, &path),
                "description": description,
                "name": name,
            }));
        }
    }
    Ok(())
}

fn category_from_skill_path(root: &Path, skill_md: &Path) -> Option<String> {
    let relative = skill_md.strip_prefix(root).ok()?;
    let parts = relative.components().collect::<Vec<_>>();
    (parts.len() >= 3).then(|| parts[0].as_os_str().to_string_lossy().to_string())
}

fn path_has_ignored_component(path: &Path) -> bool {
    path.components().any(|component| {
        let text = component.as_os_str().to_string_lossy();
        matches!(text.as_ref(), ".git" | ".github" | ".hub" | ".archive")
    })
}

fn supporting_files(skill_dir: &Path) -> io::Result<Vec<String>> {
    let mut files = Vec::new();
    for subdir in ["references", "templates", "scripts", "assets"] {
        let dir = skill_dir.join(subdir);
        collect_supporting_files(skill_dir, &dir, &mut files)?;
    }
    files.sort();
    Ok(files)
}

fn collect_supporting_files(
    skill_dir: &Path,
    dir: &Path,
    files: &mut Vec<String>,
) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_symlink() {
            continue;
        }
        if path.is_dir() {
            collect_supporting_files(skill_dir, &path, files)?;
        } else if path.is_file() {
            files.push(
                path.strip_prefix(skill_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn scans_nested_skill_commands() {
        let root = std::env::temp_dir().join(format!(
            "hermes-skills-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let skill_dir = root.join("demo").join("demo-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: Demo Skill
description: Demonstrates parity loading.
---
# Demo Skill
"#,
        )
        .unwrap();

        let commands = scan_skill_commands(&root).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "/demo-skill");
        assert_eq!(commands[0].name, "Demo Skill");
        assert_eq!(commands[0].skill_md_basename, "SKILL.md");

        let _ = fs::remove_dir_all(root);
    }
}
