use std::{fs::File, path::Path};

use anyhow::Context;
use serde_json::Value;

/// Get the mod name from modinfo.json, normalized for easy use later in MEGA file paths.
pub fn get_mod_name(source_dir: &Path) -> anyhow::Result<String> {
    let file = File::open(source_dir.join("modinfo.json"))
        .with_context(|| "Error while opening modinfo.json")?;
    let mod_info: Value =
        serde_json::from_reader(file).with_context(|| "Error while reading modinfo.json")?;

    let raw_name = mod_info["name"]
        .as_str()
        .ok_or(anyhow::anyhow!("failed to parse modinfo.json"))?;

    let mut name = String::new();
    for c in raw_name.chars() {
        if c.is_alphanumeric() {
            name.push(c);
        }
    }

    Ok(name)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use crate::mod_info::get_mod_name;

    #[test]
    fn should_fail_if_modinfo_json_is_missing_or_invalid() {
        let tmp = TempDir::new().expect("failed to create test dir");
        let dir_with_invalid_modinfo = tmp.path().join("InvalidModinfo");
        fs::create_dir(&dir_with_invalid_modinfo).expect("failed to create test subdir");
        fs::write(
            dir_with_invalid_modinfo.join("modinfo.json"),
            "{\"name\":{}}",
        )
        .expect("failed to create modinfo.json");

        let no_modinfo = get_mod_name(tmp.path()).expect_err("should be an error");
        let invalid_modinfo =
            get_mod_name(&dir_with_invalid_modinfo).expect_err("should be an error");

        assert!(
            no_modinfo.is::<std::io::Error>(),
            "should fail when modinfo.json is missing"
        );
        assert_eq!(
            "failed to parse modinfo.json",
            invalid_modinfo.to_string(),
            "should fail when modinfo.json is invalid"
        );
    }

    #[test]
    fn should_return_normalized_name_from_modinfo_json() {
        let tmp = TempDir::new().expect("failed to create test dir");

        fs::write(tmp.path().join("modinfo.json"), "{\"name\":\"Test Mod\"}")
            .expect("failed to create modinfo.json");

        let name = get_mod_name(tmp.path()).expect("should not fail");

        assert_eq!("TestMod", name);
    }
}
