use serde_json::Value;
use std::fs;
use std::path::PathBuf;

pub fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("python-parity")
}

pub fn load_fixture(name: &str) -> Value {
    let path = fixture_dir().join(name);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}

pub fn cases(fixture: &Value) -> &[Value] {
    fixture
        .get("cases")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .expect("fixture must contain cases array")
}

pub fn case<'a>(fixture: &'a Value, name: &str) -> &'a Value {
    cases(fixture)
        .iter()
        .find(|case| case.get("name").and_then(Value::as_str) == Some(name))
        .unwrap_or_else(|| panic!("fixture case not found: {name}"))
}

pub fn object_keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .expect("expected object")
        .keys()
        .map(String::as_str)
        .collect()
}
