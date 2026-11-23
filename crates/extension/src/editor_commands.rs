// SPDX-FileCopyrightText: Copyright (c) 2026 Wataru Ashihara <wataash0607@gmail.com>
// SPDX-License-Identifier: Apache-2.0
use crate::Extension;
use futures::{FutureExt, future::BoxFuture};
use gpui::{App, Global};
use std::{collections::HashMap, sync::Arc};
pub use zed_editor_command_api::{CommandInput, CommandOutput};

pub type CommandProvider =
    Arc<dyn Fn(String, String) -> BoxFuture<'static, anyhow::Result<String>> + Send + Sync>;

/// Loaded extensions providing optional editor commands. Removal follows extension unloading.
#[derive(Default)]
pub struct EditorCommands(
    pub HashMap<Arc<str>, CommandProvider>,
    pub  HashMap<
        Arc<str>,
        std::collections::BTreeMap<Arc<str>, zed_editor_command_api::CommandDefinition>,
    >,
);
impl Global for EditorCommands {}

pub fn register(extension: Arc<dyn Extension>, cx: &mut App) {
    let manifest = extension.manifest();
    let id = manifest.id.clone();
    cx.default_global::<EditorCommands>()
        .1
        .insert(id.clone(), manifest.editor_commands.clone());
    cx.default_global::<EditorCommands>().0.insert(
        id,
        Arc::new(move |command, input| {
            let extension = extension.clone();
            async move { extension.run_editor_command(command, input).await }.boxed()
        }),
    );
}

pub fn unregister(id: &str, cx: &mut App) {
    cx.default_global::<EditorCommands>().0.remove(id);
    cx.default_global::<EditorCommands>().1.remove(id);
}
