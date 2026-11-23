use std::path::Path;

pub(crate) fn workspace_color_hue(path: &Path, remote: Option<(&str, &str)>) -> f32 {
    // FNV-1a has fixed output across launches and Rust versions, unlike DefaultHasher.
    let mut hash = 0xcbf29ce484222325u64;
    let (connection_type, host) = remote.unwrap_or(("local", ""));
    for component in [
        connection_type.as_bytes(),
        host.as_bytes(),
        path.as_os_str().as_encoded_bytes(),
    ] {
        // A separator keeps distinct component boundaries from hashing identically.
        for byte in component.iter().copied().chain(std::iter::once(0)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    (hash % 3600) as f32 / 3600.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_known_workspace_color_stable() {
        assert_eq!(
            workspace_color_hue(Path::new("/home/wsh/fork/zed"), None),
            1696.0 / 3600.0,
        );
    }

    #[test]
    fn distinguishes_same_directory_name_under_different_parents() {
        assert_ne!(
            workspace_color_hue(Path::new("/work/project"), None),
            workspace_color_hue(Path::new("/personal/project"), None),
        );
    }

    #[test]
    fn distinguishes_local_and_remote_hosts() {
        let path = Path::new("/work/project");
        let local = workspace_color_hue(path, None);
        let first_host = workspace_color_hue(path, Some(("ssh", "first")));
        let second_host = workspace_color_hue(path, Some(("ssh", "second")));
        assert_ne!(local, first_host);
        assert_ne!(first_host, second_host);
        assert_ne!(
            first_host,
            workspace_color_hue(path, Some(("wsl", "first")))
        );
    }

    #[test]
    fn supports_unicode_paths_and_bounds_hue() {
        for path in ["/work/project", "/home/wsh/日本語", "/", ""] {
            let hue = workspace_color_hue(Path::new(path), None);
            assert!((0.0..1.0).contains(&hue));
        }
    }
}
