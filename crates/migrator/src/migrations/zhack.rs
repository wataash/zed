use anyhow::{Context as _, Result};
use serde_json::{Map, Value};

use super::migrate_settings;

pub(crate) fn move_search_settings(value: &mut Value) -> Result<()> {
    migrate_settings(value, &mut |settings| {
        let Some(file_finder) = settings.get("file_finder").and_then(Value::as_object) else {
            return Ok(());
        };
        let all_files = file_finder.get("include_ignored").and_then(Value::as_str) == Some("zall");
        let exclusions = file_finder.get("exclusions").cloned();
        if !all_files && exclusions.is_none() {
            return Ok(());
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
        if all_files {
            search.entry("all_files").or_insert(Value::Bool(true));
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
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
