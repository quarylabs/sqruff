use zed_extension_api::{self as zed, settings::LspSettings, LanguageServerId, Result};

const LANGUAGE_SERVER_ID: &str = "sqruff";

struct SqruffExtension;

impl zed::Extension for SqruffExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        if language_server_id.as_ref() != LANGUAGE_SERVER_ID {
            return Err(format!(
                "Unrecognized language server for Sqruff: {language_server_id}"
            ));
        }

        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree).ok();
        let binary_settings = settings.as_ref().and_then(|s| s.binary.as_ref());

        let command = binary_settings
            .and_then(|b| b.path.clone())
            .or_else(|| worktree.which("sqruff"))
            .ok_or_else(|| {
                "Could not find `sqruff` on $PATH. Install it, or set `lsp.sqruff.binary.path` in Zed settings."
                    .to_string()
            })?;

        let args = binary_settings
            .and_then(|b| b.arguments.clone())
            .unwrap_or_else(|| vec!["lsp".to_string()]);

        let env = binary_settings
            .and_then(|b| b.env.clone())
            .map(|env| env.into_iter().collect())
            .unwrap_or_default();

        Ok(zed::Command { command, args, env })
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        Ok(LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|s| s.initialization_options))
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        Ok(LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|s| s.settings))
    }
}

zed::register_extension!(SqruffExtension);
