use anyhow::{Context as _, Result};
use serde_json::{Map, Value};

use super::migrate_settings_with_inheritance;

pub(crate) fn move_search_settings(value: &mut Value) -> Result<()> {
    migrate_settings_with_inheritance(value, &mut |settings, inherited| {
        let Some(file_finder) = settings.get("file_finder").and_then(Value::as_object) else {
            return Ok(false);
        };
        let mode = file_finder.get("include_ignored").and_then(Value::as_str);
        let all_files = mode == Some("zall");
        let overrides_zall = inherited && matches!(mode, Some("indexed" | "smart" | "all"));
        let exclusions = file_finder.get("exclusions").cloned();
        if !all_files && !overrides_zall && exclusions.is_none() {
            return Ok(false);
        }

        let zhack = settings
            .entry("zhack")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .context("Expected zhack to be an object")?;
        let search = zhack
            .entry("search")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .context("Expected zhack.search to be an object")?;
        let introduced = all_files && !search.contains_key("all_files");
        if all_files || overrides_zall {
            search.entry("all_files").or_insert(Value::Bool(all_files));
        }
        if let Some(exclusions) = exclusions {
            search.entry("exclusions").or_insert(exclusions);
        }
        if let Some(file_finder) = settings
            .get_mut("file_finder")
            .and_then(Value::as_object_mut)
        {
            if all_files {
                file_finder.remove("include_ignored");
            }
            file_finder.remove("exclusions");
        }
        Ok(introduced)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn standard_modes_override_inherited_zall() -> Result<()> {
        let mut value = json!({
            "file_finder": {"include_ignored": "zall"},
            "linux": {"file_finder": {"include_ignored": "smart"}},
            "profiles": {
                "notes": {"settings": {"file_finder": {"include_ignored": "indexed"}}},
                "clean": {"base": "default", "settings": {"file_finder": {"include_ignored": "all"}}}
            }
        });
        move_search_settings(&mut value)?;
        assert_eq!(value["zhack"]["search"]["all_files"], true);
        assert_eq!(value["linux"]["zhack"]["search"]["all_files"], false);
        assert_eq!(
            value["profiles"]["notes"]["settings"]["zhack"]["search"]["all_files"],
            false
        );
        assert_eq!(
            value["profiles"]["notes"]["settings"]["file_finder"]["include_ignored"],
            "indexed"
        );
        assert!(
            value["profiles"]["clean"]["settings"]
                .get("zhack")
                .is_none()
        );
        let once = value.clone();
        move_search_settings(&mut value)?;
        assert_eq!(value, once);
        Ok(())
    }

    #[test]
    fn moves_search_settings_and_preserves_standard_settings() -> Result<()> {
        let mut value = json!({"file_finder": {
            "include_ignored": "zall", "exclusions": ["**/target"], "file_icons": false
        }});
        move_search_settings(&mut value)?;
        assert_eq!(
            value,
            json!({
                "file_finder": {"file_icons": false},
                "zhack": {"search": {"all_files": true, "exclusions": ["**/target"]}}
            })
        );
        let migrated = value.clone();
        move_search_settings(&mut value)?;
        assert_eq!(value, migrated);
        Ok(())
    }

    #[test]
    fn explicit_values_win_in_overrides() -> Result<()> {
        let settings = json!({
            "file_finder": {"include_ignored": "zall", "exclusions": ["old"]},
            "zhack": {"watch_exclusions": ["**/.git"], "search": {
                "all_files": false, "exclusions": []
            }}
        });
        let mut value = json!({"linux": settings, "profiles": {
            "test": {"settings": settings}
        }});
        move_search_settings(&mut value)?;
        for settings in [&value["linux"], &value["profiles"]["test"]["settings"]] {
            assert_eq!(
                settings["zhack"]["search"],
                json!({"all_files": false, "exclusions": []})
            );
            assert_eq!(settings["zhack"]["watch_exclusions"], json!(["**/.git"]));
            assert_eq!(settings["file_finder"], json!({}));
        }
        Ok(())
    }
}
