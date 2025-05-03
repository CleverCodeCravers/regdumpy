use std::io::Write;
use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Serialize)]
struct RegEntry {
    path: String,
    name: String,
    value: RegValueJson,
}

#[derive(Serialize)]
#[serde(untagged)]
enum RegValueJson {
    String(String),
    U32(u32),
    U64(u64),
    Bytes(Vec<u8>),
    Invalid { reason: String },
    None,
}

/// Recursively dump the registry starting from `root_key_str` into `output_path`.
/// The dump is written as JSON lines, one entry per line.
pub fn dump_registry(output_path: &Path, root_key_str: &str) -> Result<()> {
    let file = File::create(output_path).context("create output file")?;
    let mut writer = std::io::BufWriter::new(file);

    let system_hive = match_root(root_key_str)?;
    recurse_key(&system_hive, root_key_str, &mut writer)?;
    Ok(())
}

fn recurse_key<W: Write>(key: &RegKey, path: &str, writer: &mut W) -> Result<()> {
    // Enumerate values
    for value_res in key.enum_values() {
        let (name, value) = match value_res {
            Ok(v) => v,
            Err(e) => {
                writeln!(writer, "{}", serde_json::to_string(&RegEntry{path: path.to_string(), name: "<invalid>".into(), value: RegValueJson::Invalid{ reason: format!("failed to read value: {e}") }} )?)?;
                continue;
            }
        };

        let json_value = match_value_to_json(&value);
        let entry = RegEntry {
            path: path.to_string(),
            name,
            value: json_value,
        };
        writeln!(writer, "{}", serde_json::to_string(&entry)?)?;
    }

    // Recurse into subkeys
    for sub_res in key.enum_keys() {
        let sub_name = match sub_res { Ok(s) => s, Err(_) => continue };
        let sub_path = format!("{}\\{}", path, sub_name);
        if let Ok(sub_key) = key.open_subkey_with_flags(&sub_name, KEY_READ) {
            recurse_key(&sub_key, &sub_path, writer)?;
        }
    }
    Ok(())
}

fn match_value_to_json(val: &winreg::RegValue) -> RegValueJson {
    match val.vtype {
        REG_DWORD => {
            if val.bytes.len() == 4 {
                let mut arr = [0u8; 4];
                arr.copy_from_slice(&val.bytes);
                RegValueJson::U32(u32::from_le_bytes(arr))
            } else {
                RegValueJson::Invalid { reason: format!("invalid DWORD length {}", val.bytes.len()) }
            }
        }
        REG_QWORD => {
            if val.bytes.len() == 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&val.bytes);
                RegValueJson::U64(u64::from_le_bytes(arr))
            } else {
                RegValueJson::Invalid { reason: format!("invalid QWORD length {}", val.bytes.len()) }
            }
        }
        REG_SZ | REG_EXPAND_SZ => String::from_utf8(val.bytes.clone()).map(|mut s| { if let Some(0) = s.chars().last().map(|c| c as u32) { s.pop(); } s }).map(RegValueJson::String).unwrap_or(RegValueJson::Invalid{ reason: "invalid UTF-8".into() }),
        REG_BINARY => RegValueJson::Bytes(val.bytes.clone()),
        _ => RegValueJson::None,
    }
}

fn match_root(root: &str) -> Result<RegKey> {
    let hkey = match root.to_uppercase().as_str() {
        "HKEY_CLASSES_ROOT" | "HKCR" => RegKey::predef(HKEY_CLASSES_ROOT),
        "HKEY_CURRENT_USER" | "HKCU" => RegKey::predef(HKEY_CURRENT_USER),
        "HKEY_LOCAL_MACHINE" | "HKLM" => RegKey::predef(HKEY_LOCAL_MACHINE),
        "HKEY_USERS" | "HKU" => RegKey::predef(HKEY_USERS),
        "HKEY_CURRENT_CONFIG" | "HKCC" => RegKey::predef(HKEY_CURRENT_CONFIG),
        _ => anyhow::bail!("Unknown root hive {}", root),
    };
    Ok(hkey)
}

#[cfg(test)]
mod tests {
    use super::*;
    use winreg::RegValue;

    #[test]
    fn test_match_root_hklm() {
        assert!(match_root("HKLM").is_ok());
    }

    #[test]
    fn test_match_value_dword() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x1234u32.to_le_bytes());
        let val = RegValue { vtype: REG_DWORD, bytes };
        match match_value_to_json(&val) {
            RegValueJson::U32(v) => assert_eq!(v, 0x1234),
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn test_match_value_invalid_dword() {
        let val = RegValue { vtype: REG_DWORD, bytes: vec![1, 2, 3] };
        match match_value_to_json(&val) {
            RegValueJson::Invalid { .. } => {},
            _ => panic!("expected invalid"),
        }
    }
}
