use settings::Settings;

#[derive(Clone, Debug, PartialEq)]
pub struct CommandPaletteSettings {
    pub commands: std::collections::BTreeMap<String, serde_json::Value>,
    pub use_command_history: bool,
}

impl Settings for CommandPaletteSettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        Self {
            commands: content
                .command_palette
                .as_ref()
                .and_then(|settings| settings.commands.as_ref())
                .map(|commands| {
                    commands
                        .iter()
                        .map(|(title, action)| (title.clone(), action.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            use_command_history: content
                .command_palette
                .as_ref()
                .and_then(|settings| settings.use_command_history)
                .expect("missing default for command_palette.use_command_history"),
        }
    }
}
