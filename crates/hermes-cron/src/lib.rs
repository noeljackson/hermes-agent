use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::Path;

pub fn parse_duration_minutes(input: &str) -> Result<i64, String> {
    let trimmed = input.trim().to_lowercase();
    let digits: String = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return Err(format!("Invalid duration: '{trimmed}'"));
    }
    let unit = trimmed[digits.len()..].trim();
    let value: i64 = digits
        .parse()
        .map_err(|_| format!("Invalid duration: '{trimmed}'"))?;
    let multiplier = match unit {
        "m" | "min" | "mins" | "minute" | "minutes" => 1,
        "h" | "hr" | "hrs" | "hour" | "hours" => 60,
        "d" | "day" | "days" => 1440,
        _ => return Err(format!("Invalid duration: '{trimmed}'")),
    };
    Ok(value * multiplier)
}

pub fn parse_schedule(input: &str) -> Result<Value, String> {
    let schedule = input.trim();
    let lower = schedule.to_lowercase();

    if let Some(duration) = lower.strip_prefix("every ") {
        let minutes = parse_duration_minutes(duration)?;
        return Ok(json!({
            "display": format!("every {minutes}m"),
            "kind": "interval",
            "minutes": minutes,
        }));
    }

    let parts: Vec<&str> = schedule.split_whitespace().collect();
    if parts.len() >= 5
        && parts[..5].iter().all(|part| {
            part.chars()
                .all(|ch| ch.is_ascii_digit() || matches!(ch, '*' | '-' | ',' | '/'))
        })
    {
        return Ok(json!({
            "display": schedule,
            "expr": schedule,
            "kind": "cron",
        }));
    }

    if schedule.contains('T') || looks_like_date(schedule) {
        let display = schedule.get(0..16).unwrap_or(schedule).replace('T', " ");
        return Ok(json!({
            "display": format!("once at {display}"),
            "kind": "once",
            "run_at": schedule,
        }));
    }

    let minutes = parse_duration_minutes(schedule)?;
    Ok(json!({
        "display": format!("once in {schedule}"),
        "kind": "once",
        "minutes_from_now": minutes,
    }))
}

pub fn normalize_job_record(job: &Value) -> Value {
    let mut normalized = job.as_object().cloned().unwrap_or_default();

    let skills = normalize_skill_list(normalized.get("skill"), normalized.get("skills"));
    normalized.insert("skills".to_string(), Value::Array(skills.clone()));
    normalized.insert(
        "skill".to_string(),
        skills.first().cloned().unwrap_or(Value::Null),
    );

    let job_id = coerce_job_text(normalized.get("id"), "unknown");
    let prompt = coerce_job_text(normalized.get("prompt"), "");
    normalized.insert("id".to_string(), json!(job_id));
    normalized.insert("prompt".to_string(), json!(prompt));

    let name = normalized
        .get("name")
        .map(|value| coerce_value_text(value, ""))
        .unwrap_or_default()
        .trim()
        .to_string();
    let name = if name.is_empty() {
        let script = normalized
            .get("script")
            .map(|value| coerce_value_text(value, ""))
            .unwrap_or_default()
            .trim()
            .to_string();
        let label_source = if !prompt.is_empty() {
            prompt
        } else if let Some(skill) = skills.first().and_then(Value::as_str) {
            skill.to_string()
        } else if !script.is_empty() {
            script
        } else if !job_id.is_empty() {
            job_id
        } else {
            "cron job".to_string()
        };
        let truncated: String = label_source.chars().take(50).collect();
        let truncated = truncated.trim().to_string();
        if truncated.is_empty() {
            "cron job".to_string()
        } else {
            truncated
        }
    } else {
        name
    };
    normalized.insert("name".to_string(), json!(name));

    normalized.insert(
        "schedule_display".to_string(),
        json!(schedule_display_for_job(&Value::Object(normalized.clone()))),
    );

    if normalized
        .get("state")
        .map(|value| coerce_value_text(value, ""))
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        let enabled = normalized
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        normalized.insert(
            "state".to_string(),
            json!(if enabled { "scheduled" } else { "paused" }),
        );
    }

    let profile = normalized
        .get("profile")
        .map(|value| coerce_value_text(value, ""))
        .unwrap_or_default()
        .trim()
        .to_string();
    normalized.insert(
        "profile".to_string(),
        if profile.is_empty() {
            Value::Null
        } else {
            json!(profile)
        },
    );
    Value::Object(normalized)
}

pub fn compute_grace_seconds(schedule: &Value) -> i64 {
    const MIN_GRACE: i64 = 120;
    const MAX_GRACE: i64 = 7200;
    if schedule.get("kind") == Some(&json!("interval")) {
        let period = schedule.get("minutes").and_then(Value::as_i64).unwrap_or(1) * 60;
        return (period / 2).clamp(MIN_GRACE, MAX_GRACE);
    }
    MIN_GRACE
}

pub fn compute_next_run(
    schedule: &Value,
    now_iso: &str,
    last_run_at: Option<&str>,
) -> Option<String> {
    match schedule.get("kind").and_then(Value::as_str) {
        Some("once") => {
            if last_run_at.is_some() {
                return None;
            }
            let run_at = schedule.get("run_at").and_then(Value::as_str)?;
            let now = parse_utc_minutes(now_iso)?;
            let run = parse_utc_minutes(run_at)?;
            if run >= now - (120 / 60) {
                Some(run_at.to_string())
            } else {
                None
            }
        }
        Some("interval") => {
            let minutes = schedule.get("minutes").and_then(Value::as_i64)?;
            let base = last_run_at.unwrap_or(now_iso);
            add_utc_minutes(base, minutes)
        }
        _ => None,
    }
}

pub fn save_jobs(path: impl AsRef<Path>, jobs: &[Value]) -> io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let normalized = jobs.iter().map(normalize_job_record).collect::<Vec<_>>();
    let bytes = serde_json::to_vec_pretty(&normalized).map_err(io::Error::other)?;
    fs::write(path, bytes)
}

pub fn load_jobs(path: impl AsRef<Path>) -> io::Result<Vec<Value>> {
    if !path.as_ref().exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)?;
    let parsed = serde_json::from_str::<Value>(&raw).map_err(io::Error::other)?;
    let jobs = match parsed {
        Value::Array(items) => items,
        Value::Object(mut object) => object
            .remove("jobs")
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    Ok(jobs.iter().map(normalize_job_record).collect())
}

fn looks_like_date(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.len() >= 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

fn parse_utc_minutes(input: &str) -> Option<i64> {
    let (_, _, _, hour, minute) = parse_utc_parts(input)?;
    Some(hour * 60 + minute)
}

fn add_utc_minutes(input: &str, minutes: i64) -> Option<String> {
    let (date, second, offset, hour, minute) = parse_utc_parts(input)?;
    let total = hour * 60 + minute + minutes;
    let next_hour = total.div_euclid(60);
    let next_minute = total.rem_euclid(60);
    Some(format!(
        "{date}T{next_hour:02}:{next_minute:02}:{second}{offset}"
    ))
}

fn parse_utc_parts(input: &str) -> Option<(&str, &str, &str, i64, i64)> {
    let (date, rest) = input.split_once('T')?;
    let offset_start = rest.find('+').or_else(|| rest.rfind('-'))?;
    let (time, offset) = rest.split_at(offset_start);
    let mut pieces = time.split(':');
    let hour = pieces.next()?.parse().ok()?;
    let minute = pieces.next()?.parse().ok()?;
    let second = pieces.next().unwrap_or("00");
    Some((date, second, offset, hour, minute))
}

fn normalize_skill_list(skill: Option<&Value>, skills: Option<&Value>) -> Vec<Value> {
    let raw_items = match skills {
        None => skill.into_iter().cloned().collect::<Vec<_>>(),
        Some(Value::String(text)) => vec![json!(text)],
        Some(Value::Array(items)) => items.clone(),
        Some(other) => vec![other.clone()],
    };

    let mut normalized = Vec::new();
    let mut seen = Vec::<String>::new();
    for item in raw_items {
        let text = coerce_value_text(&item, "").trim().to_string();
        if !text.is_empty() && !seen.contains(&text) {
            seen.push(text.clone());
            normalized.push(json!(text));
        }
    }
    normalized
}

fn schedule_display_for_job(job: &Value) -> String {
    let explicit = job
        .get("schedule_display")
        .map(|value| coerce_value_text(value, ""))
        .unwrap_or_default()
        .trim()
        .to_string();
    if !explicit.is_empty() {
        return explicit;
    }

    match job.get("schedule") {
        Some(Value::Object(schedule)) => {
            for key in ["display", "value", "expr", "run_at"] {
                let text = schedule
                    .get(key)
                    .map(|value| coerce_value_text(value, ""))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    return text;
                }
            }
            "?".to_string()
        }
        Some(other) if !other.is_null() => coerce_value_text(other, ""),
        _ => "?".to_string(),
    }
}

fn coerce_job_text(value: Option<&Value>, fallback: &str) -> String {
    value
        .map(|value| coerce_value_text(value, fallback))
        .unwrap_or_else(|| fallback.to_string())
}

fn coerce_value_text(value: &Value, fallback: &str) -> String {
    match value {
        Value::Null => fallback.to_string(),
        Value::String(text) => text.clone(),
        Value::Bool(value) => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Number(value) => value.to_string(),
        other => other.to_string(),
    }
}
