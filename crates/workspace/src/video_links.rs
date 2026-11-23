use std::path::Path;

pub(super) fn should_open_with_system(path: &Path, is_local: bool, is_directory: bool) -> bool {
    is_local
        && !is_directory
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "mp4"
                        | "m4v"
                        | "mkv"
                        | "webm"
                        | "mov"
                        | "avi"
                        | "mpeg"
                        | "mpg"
                        | "ogv"
                        | "mts"
                        | "m2ts"
                )
            })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_paths_use_system_player() {
        for path in [
            "clip.mp4",
            "../media/clip.MP4",
            "/media/演奏 session.webm",
            "clip.mkv",
        ] {
            assert!(
                should_open_with_system(Path::new(path), true, false),
                "{path}"
            );
        }
    }

    #[test]
    fn editor_files_and_other_targets_stay_in_editor() {
        for path in [
            "video.mp4.txt",
            "module.ts",
            "picture.png",
            "README",
            "clip",
        ] {
            assert!(
                !should_open_with_system(Path::new(path), true, false),
                "{path}"
            );
        }
        assert!(!should_open_with_system(
            Path::new("clip.mp4"),
            false,
            false
        ));
        assert!(!should_open_with_system(
            Path::new("folder.mp4"),
            true,
            true
        ));
    }
}
