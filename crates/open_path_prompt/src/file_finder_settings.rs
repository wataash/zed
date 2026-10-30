use anyhow::Context as _;
use settings::{ModalWidthContent, RegisterSetting, Settings};
use util::{
    ResultExt as _,
    paths::{PathMatcher, PathStyle},
};

#[derive(Debug, Clone, PartialEq, RegisterSetting)]
pub struct FileFinderSettings {
    pub file_icons: bool,
    pub modal_max_width: ModalWidthContent,
    pub skip_focus_for_active_in_search: bool,
    pub include_ignored: Option<bool>,
    /// Whether including ignored files also walks directories the worktree has not loaded.
    pub walk_unloaded_ignored: bool,
    pub include_channels: bool,
    pub prefill_query_from_selection: bool,
    pub exclusions: PathMatcher,
}

impl Settings for FileFinderSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        let file_finder = content.file_finder.as_ref().unwrap();
        let include_ignored = file_finder.include_ignored.unwrap();
        let search = content
            .project
            .zhack
            .as_ref()
            .and_then(|zhack| zhack.search.as_ref());
        let all_files = search.and_then(|search| search.all_files).unwrap_or(false);

        Self {
            file_icons: file_finder.file_icons.unwrap(),
            modal_max_width: file_finder.modal_max_width.unwrap(),
            skip_focus_for_active_in_search: file_finder.skip_focus_for_active_in_search.unwrap(),
            include_ignored: if all_files {
                Some(true)
            } else {
                match include_ignored {
                    settings::IncludeIgnoredContent::All => Some(true),
                    settings::IncludeIgnoredContent::Indexed => Some(false),
                    settings::IncludeIgnoredContent::Smart => None,
                }
            },
            walk_unloaded_ignored: all_files,
            include_channels: file_finder.include_channels.unwrap(),
            prefill_query_from_selection: file_finder.prefill_query_from_selection.unwrap(),
            exclusions: PathMatcher::new(
                search
                    .and_then(|search| search.exclusions.as_ref())
                    .map(|exclusions| exclusions.0.clone())
                    .unwrap_or_default(),
                PathStyle::local(),
            )
            .context("Failed to parse globs from zhack.search.exclusions")
            .log_err()
            .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn zhack_search_controls_ignored_files_and_exclusions() -> anyhow::Result<()> {
        let mut content: settings::SettingsContent = serde_json::from_value(json!({
            "file_finder": {
                "file_icons": true, "modal_max_width": "small",
                "skip_focus_for_active_in_search": true,
                "include_ignored": "indexed", "include_channels": false,
                "prefill_query_from_selection": false
            },
            "zhack": {"search": {"all_files": true, "exclusions": ["**/target"]}}
        }))?;
        let settings = FileFinderSettings::from_settings(&content);
        assert_eq!(settings.include_ignored, Some(true));
        assert!(settings.walk_unloaded_ignored);
        assert!(settings.exclusions.is_match_std_path("target"));
        content.project.zhack = None;
        let settings = FileFinderSettings::from_settings(&content);
        assert_eq!(settings.include_ignored, Some(false));
        assert!(!settings.walk_unloaded_ignored);
        assert_eq!(settings.exclusions.sources().count(), 0);
        Ok(())
    }
}
