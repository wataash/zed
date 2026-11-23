//! Bounded file reads and an extension-defined, read-only file presentation.
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub const PAGE_SIZE: usize = 16 * 1024;

#[derive(Debug)]
pub struct FilePage {
    pub offset: u64,
    pub size: u64,
    pub bytes: Vec<u8>,
}

pub fn is_binary_file(path: &Path) -> anyhow::Result<bool> {
    if !path.metadata()?.is_file() {
        return Ok(false);
    }
    let mut prefix = Vec::new();
    File::open(path)?.take(8192).read_to_end(&mut prefix)?;
    Ok(file_content::decode_byte_header(&prefix).1 == file_content::ByteContent::Binary)
}

pub fn read_page(path: &Path, offset: u64) -> anyhow::Result<FilePage> {
    anyhow::ensure!(path.metadata()?.is_file(), "Not a regular file");
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::new();
    file.take(PAGE_SIZE as u64).read_to_end(&mut bytes)?;
    Ok(FilePage {
        offset,
        size,
        bytes,
    })
}

#[cfg(test)]
mod file_tests {
    use super::*;
    #[test]
    fn pages_are_bounded_and_do_not_modify_the_file() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!(
            "zed-hex-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let bytes: Vec<u8> = (0..PAGE_SIZE + 17)
            .map(|offset| (offset % 256) as u8)
            .collect();
        std::fs::write(&path, &bytes)?;
        let result = (|| -> anyhow::Result<()> {
            assert!(is_binary_file(&path)?);
            let first = read_page(&path, 0)?;
            assert_eq!(first.bytes.len(), PAGE_SIZE);
            assert_eq!(first.size, bytes.len() as u64);
            let second = read_page(&path, PAGE_SIZE as u64)?;
            assert_eq!(second.bytes.len(), 17);
            assert_eq!(second.offset, PAGE_SIZE as u64);
            assert!(read_page(&path, bytes.len() as u64)?.bytes.is_empty());
            assert_eq!(std::fs::read(&path)?, bytes);
            std::fs::write(&path, b"ordinary text\n")?;
            assert!(!is_binary_file(&path)?);
            Ok(())
        })();
        std::fs::remove_file(&path)?;
        result
    }
}

#[derive(Debug)]
pub struct PreviewPage {
    pub offset: u64,
    pub size: u64,
    pub presentation: zed_editor_command_api::FilePresentation,
}

/// The host grants only one page of the selected file. Formatting and opening
/// policy run through the existing optional WASM command boundary.
pub fn present_file(
    path: &Path,
    offset: u64,
    reason: &str,
    options: serde_json::Value,
    cx: &mut gpui::App,
) -> gpui::Task<anyhow::Result<PreviewPage>> {
    use gpui::AppContext as _;
    let provider = cx
        .try_global::<extension::editor_commands::EditorCommands>()
        .and_then(|registry| {
            let mut providers = registry
                .1
                .iter()
                .filter(|(_, commands)| commands.contains_key("preview-file"));
            let (id, _) = providers.next()?;
            if providers.next().is_some() {
                return None;
            }
            registry.0.get(id).cloned()
        });
    let path = path.to_path_buf();
    let reason = reason.to_owned();
    cx.background_spawn(async move {
        let provider = provider
            .ok_or_else(|| anyhow::anyhow!("Load exactly one extension providing preview-file"))?;
        let page = read_page(&path, offset)?;
        let request = zed_editor_command_api::FilePreviewInput {
            path: path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("File preview requires a UTF-8 path"))?
                .to_owned(),
            offset: page.offset,
            size: page.size,
            bytes: page.bytes,
            reason,
            options,
        };
        let input = zed_editor_command_api::CommandInput {
            arguments: serde_json::to_value(request)?,
            host_capabilities: vec!["file_preview".into()],
            ..Default::default()
        };
        let output = provider("preview-file".into(), serde_json::to_string(&input)?).await?;
        let output: zed_editor_command_api::CommandOutput = serde_json::from_str(&output)?;
        let presentation = output
            .into_file_presentation()
            .map_err(anyhow::Error::msg)?;
        Ok(PreviewPage {
            offset: page.offset,
            size: page.size,
            presentation,
        })
    })
}
