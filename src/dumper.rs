use std::fs::File;
use std::io::Write;
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

#[derive(Serialize, Debug, PartialEq)]
#[serde(untagged)]
enum RegValueJson {
    String(String),
    StringArray(Vec<String>),
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
                let entry = RegEntry {
                    path: path.to_string(),
                    name: "<invalid>".into(),
                    value: RegValueJson::Invalid {
                        reason: format!("failed to read value: {e}"),
                    },
                };
                writeln!(writer, "{}", serde_json::to_string(&entry)?)?;
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
        let sub_name = match sub_res {
            Ok(s) => s,
            Err(_) => continue,
        };
        let sub_path = format!("{}\\{}", path, sub_name);
        if let Ok(sub_key) = key.open_subkey_with_flags(&sub_name, KEY_READ) {
            recurse_key(&sub_key, &sub_path, writer)?;
        }
    }
    Ok(())
}

fn decode_utf16le(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&wide)
        .ok()
        .map(|s| s.trim_end_matches('\0').to_string())
}

fn match_value_to_json(val: &winreg::RegValue) -> RegValueJson {
    match val.vtype {
        REG_DWORD => {
            if val.bytes.len() == 4 {
                let mut arr = [0u8; 4];
                arr.copy_from_slice(&val.bytes);
                RegValueJson::U32(u32::from_le_bytes(arr))
            } else {
                RegValueJson::Invalid {
                    reason: format!("invalid DWORD length {}", val.bytes.len()),
                }
            }
        }
        REG_QWORD => {
            if val.bytes.len() == 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&val.bytes);
                RegValueJson::U64(u64::from_le_bytes(arr))
            } else {
                RegValueJson::Invalid {
                    reason: format!("invalid QWORD length {}", val.bytes.len()),
                }
            }
        }
        REG_SZ | REG_EXPAND_SZ => decode_utf16le(&val.bytes)
            .map(RegValueJson::String)
            .unwrap_or(RegValueJson::Invalid {
                reason: "invalid UTF-16".into(),
            }),
        REG_MULTI_SZ => {
            match decode_utf16le(&val.bytes) {
                Some(decoded) => {
                    // REG_MULTI_SZ: null-separated strings, double-null terminated
                    let strings: Vec<String> = decoded
                        .split('\0')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect();
                    RegValueJson::StringArray(strings)
                }
                None => RegValueJson::Invalid {
                    reason: "invalid UTF-16 in MULTI_SZ".into(),
                },
            }
        }
        REG_BINARY => RegValueJson::Bytes(val.bytes.to_vec()),
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
    use std::borrow::Cow;
    use winreg::RegValue;

    fn rv(vtype: RegType, bytes: Vec<u8>) -> RegValue<'static> {
        RegValue {
            vtype,
            bytes: Cow::Owned(bytes),
        }
    }

    // --- match_root tests ---

    #[test]
    fn test_match_root_all_long_names() {
        assert!(match_root("HKEY_CLASSES_ROOT").is_ok());
        assert!(match_root("HKEY_CURRENT_USER").is_ok());
        assert!(match_root("HKEY_LOCAL_MACHINE").is_ok());
        assert!(match_root("HKEY_USERS").is_ok());
        assert!(match_root("HKEY_CURRENT_CONFIG").is_ok());
    }

    #[test]
    fn test_match_root_all_short_names() {
        assert!(match_root("HKCR").is_ok());
        assert!(match_root("HKCU").is_ok());
        assert!(match_root("HKLM").is_ok());
        assert!(match_root("HKU").is_ok());
        assert!(match_root("HKCC").is_ok());
    }

    #[test]
    fn test_match_root_case_insensitive() {
        assert!(match_root("hkey_local_machine").is_ok());
        assert!(match_root("hklm").is_ok());
        assert!(match_root("Hklm").is_ok());
    }

    #[test]
    fn test_match_root_invalid() {
        assert!(match_root("INVALID").is_err());
        assert!(match_root("").is_err());
        assert!(match_root("HKEY_NONEXISTENT").is_err());
    }

    // --- DWORD tests ---

    #[test]
    fn test_match_value_dword() {
        let val = rv(REG_DWORD, 0x1234u32.to_le_bytes().to_vec());
        assert_eq!(match_value_to_json(&val), RegValueJson::U32(0x1234));
    }

    #[test]
    fn test_match_value_dword_zero() {
        let val = rv(REG_DWORD, 0u32.to_le_bytes().to_vec());
        assert_eq!(match_value_to_json(&val), RegValueJson::U32(0));
    }

    #[test]
    fn test_match_value_dword_max() {
        let val = rv(REG_DWORD, u32::MAX.to_le_bytes().to_vec());
        assert_eq!(match_value_to_json(&val), RegValueJson::U32(u32::MAX));
    }

    #[test]
    fn test_match_value_invalid_dword() {
        let val = rv(REG_DWORD, vec![1, 2, 3]);
        match match_value_to_json(&val) {
            RegValueJson::Invalid { .. } => {}
            other => panic!("expected Invalid, got {:?}", other),
        }
    }

    // --- QWORD tests ---

    #[test]
    fn test_match_value_qword() {
        let val = rv(REG_QWORD, 0xDEADBEEFCAFEu64.to_le_bytes().to_vec());
        assert_eq!(match_value_to_json(&val), RegValueJson::U64(0xDEADBEEFCAFE));
    }

    #[test]
    fn test_match_value_qword_zero() {
        let val = rv(REG_QWORD, 0u64.to_le_bytes().to_vec());
        assert_eq!(match_value_to_json(&val), RegValueJson::U64(0));
    }

    #[test]
    fn test_match_value_invalid_qword() {
        let val = rv(REG_QWORD, vec![1, 2, 3, 4]);
        match match_value_to_json(&val) {
            RegValueJson::Invalid { .. } => {}
            other => panic!("expected Invalid, got {:?}", other),
        }
    }

    // --- REG_SZ tests (UTF-16LE) ---

    fn encode_utf16le(s: &str) -> Vec<u8> {
        let mut bytes: Vec<u8> = s.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
        // Add null terminator
        bytes.push(0);
        bytes.push(0);
        bytes
    }

    #[test]
    fn test_match_value_sz_ascii() {
        let val = rv(REG_SZ, encode_utf16le("Hello"));
        assert_eq!(
            match_value_to_json(&val),
            RegValueJson::String("Hello".to_string())
        );
    }

    #[test]
    fn test_match_value_sz_unicode() {
        let val = rv(REG_SZ, encode_utf16le("Ünïcödé Tëst äöü"));
        assert_eq!(
            match_value_to_json(&val),
            RegValueJson::String("Ünïcödé Tëst äöü".to_string())
        );
    }

    #[test]
    fn test_match_value_sz_empty() {
        let val = rv(REG_SZ, encode_utf16le(""));
        assert_eq!(
            match_value_to_json(&val),
            RegValueJson::String("".to_string())
        );
    }

    #[test]
    fn test_match_value_sz_cjk() {
        let val = rv(REG_SZ, encode_utf16le("日本語テスト"));
        assert_eq!(
            match_value_to_json(&val),
            RegValueJson::String("日本語テスト".to_string())
        );
    }

    #[test]
    fn test_match_value_sz_invalid_odd_bytes() {
        let val = rv(REG_SZ, vec![0x41, 0x00, 0x42]);
        match match_value_to_json(&val) {
            RegValueJson::Invalid { .. } => {}
            other => panic!("expected Invalid, got {:?}", other),
        }
    }

    #[test]
    fn test_match_value_expand_sz() {
        let val = rv(REG_EXPAND_SZ, encode_utf16le("%SystemRoot%\\system32"));
        assert_eq!(
            match_value_to_json(&val),
            RegValueJson::String("%SystemRoot%\\system32".to_string())
        );
    }

    // --- REG_MULTI_SZ tests ---

    fn encode_multi_sz(strings: &[&str]) -> Vec<u8> {
        let mut result = String::new();
        for s in strings {
            result.push_str(s);
            result.push('\0');
        }
        result.push('\0'); // double-null terminator
        result
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect()
    }

    #[test]
    fn test_match_value_multi_sz() {
        let val = rv(REG_MULTI_SZ, encode_multi_sz(&["alpha", "beta", "gamma"]));
        assert_eq!(
            match_value_to_json(&val),
            RegValueJson::StringArray(vec![
                "alpha".to_string(),
                "beta".to_string(),
                "gamma".to_string()
            ])
        );
    }

    #[test]
    fn test_match_value_multi_sz_single() {
        let val = rv(REG_MULTI_SZ, encode_multi_sz(&["only"]));
        assert_eq!(
            match_value_to_json(&val),
            RegValueJson::StringArray(vec!["only".to_string()])
        );
    }

    #[test]
    fn test_match_value_multi_sz_empty() {
        let val = rv(REG_MULTI_SZ, encode_multi_sz(&[]));
        assert_eq!(match_value_to_json(&val), RegValueJson::StringArray(vec![]));
    }

    #[test]
    fn test_match_value_multi_sz_unicode() {
        let val = rv(REG_MULTI_SZ, encode_multi_sz(&["Ünïcödé", "日本語"]));
        assert_eq!(
            match_value_to_json(&val),
            RegValueJson::StringArray(vec!["Ünïcödé".to_string(), "日本語".to_string()])
        );
    }

    // --- REG_BINARY tests ---

    #[test]
    fn test_match_value_binary() {
        let val = rv(REG_BINARY, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(
            match_value_to_json(&val),
            RegValueJson::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF])
        );
    }

    #[test]
    fn test_match_value_binary_empty() {
        let val = rv(REG_BINARY, vec![]);
        assert_eq!(match_value_to_json(&val), RegValueJson::Bytes(vec![]));
    }

    // --- Unknown type tests ---

    #[test]
    fn test_match_value_unknown_type() {
        let val = rv(REG_NONE, vec![1, 2, 3]);
        assert_eq!(match_value_to_json(&val), RegValueJson::None);
    }

    // --- decode_utf16le tests ---

    #[test]
    fn test_decode_utf16le_valid() {
        let bytes = encode_utf16le("test");
        assert_eq!(decode_utf16le(&bytes), Some("test".to_string()));
    }

    #[test]
    fn test_decode_utf16le_odd_bytes() {
        assert_eq!(decode_utf16le(&[0x41, 0x00, 0x42]), None);
    }

    #[test]
    fn test_decode_utf16le_empty() {
        assert_eq!(decode_utf16le(&[]), Some("".to_string()));
    }

    // --- Integration test: dump_registry ---

    #[test]
    fn test_dump_registry_hkcu() {
        let dir = std::env::temp_dir();
        let output = dir.join("regdumpy_test_dump.jsonl");
        let result = dump_registry(&output, "HKCU");
        assert!(result.is_ok(), "dump_registry failed: {:?}", result.err());

        let content = std::fs::read_to_string(&output).unwrap();
        assert!(!content.is_empty(), "dump should not be empty");

        // Each line should be valid JSON
        for (i, line) in content.lines().enumerate().take(10) {
            let parsed: serde_json::Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("line {} is not valid JSON: {}", i, e));
            assert!(parsed.get("path").is_some(), "line {} missing 'path'", i);
            assert!(parsed.get("name").is_some(), "line {} missing 'name'", i);
            assert!(parsed.get("value").is_some(), "line {} missing 'value'", i);
        }

        // Cleanup
        let _ = std::fs::remove_file(&output);
    }
}
