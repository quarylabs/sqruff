//! Zed extension that runs `sqruff lsp` as a language server for SQL.

use std::fs;

use zed_extension_api::{self as zed, LanguageServerId, Result, settings::LspSettings};

/// The language server declared in `extension.toml`.
const SERVER_ID: &str = "sqruff";
const GITHUB_REPO: &str = "quarylabs/sqruff";

#[derive(Default)]
struct SqruffExtension {
    cached_binary_path: Option<String>,
}

impl SqruffExtension {
    /// Resolves the `sqruff` binary, preferring, in order: an explicitly
    /// configured path, one already on `$PATH`, then a GitHub release download.
    fn binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
        configured_path: Option<String>,
    ) -> Result<String> {
        if let Some(path) = configured_path {
            return Ok(path);
        }

        if let Some(path) = worktree.which("sqruff") {
            return Ok(path);
        }

        if let Some(path) = self
            .cached_binary_path
            .as_ref()
            .filter(|path| fs::metadata(path).is_ok_and(|stat| stat.is_file()))
        {
            return Ok(path.clone());
        }

        self.download_binary(language_server_id)
    }

    fn download_binary(&mut self, language_server_id: &LanguageServerId) -> Result<String> {
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let (platform, arch) = zed::current_platform();
        let binary_name = binary_name(platform);

        let release = match zed::latest_github_release(
            GITHUB_REPO,
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        ) {
            Ok(release) => release,
            // Offline, or the unauthenticated GitHub API is rate limited. A
            // binary left by an earlier download beats no language server.
            Err(err) => {
                let Some(path) = downloaded_binary(binary_name) else {
                    return Err(err);
                };
                return Ok(self.cache(language_server_id, path));
            }
        };

        let asset_name = asset_name(platform, arch)?;
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| {
                format!(
                    "sqruff {} has no asset named `{asset_name}`",
                    release.version
                )
            })?;

        // Every archive holds the bare binary at its root.
        let version_dir = format!("sqruff-{}", release.version);
        let binary_path = format!("{version_dir}/{binary_name}");

        if !fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            let file_type = match platform {
                zed::Os::Windows => zed::DownloadedFileType::Zip,
                zed::Os::Mac | zed::Os::Linux => zed::DownloadedFileType::GzipTar,
            };
            zed::download_file(&asset.download_url, &version_dir, file_type)
                .map_err(|err| format!("failed to download `{asset_name}`: {err}"))?;
            zed::make_file_executable(&binary_path)?;

            // Drop every previously downloaded version.
            let entries =
                fs::read_dir(".").map_err(|err| format!("failed to list work directory: {err}"))?;
            for entry in entries {
                let entry =
                    entry.map_err(|err| format!("failed to read directory entry: {err}"))?;
                if entry.file_name().to_str() != Some(version_dir.as_str()) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }
        }

        Ok(self.cache(language_server_id, binary_path))
    }

    fn cache(&mut self, language_server_id: &LanguageServerId, binary_path: String) -> String {
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::None,
        );

        self.cached_binary_path = Some(binary_path.clone());
        binary_path
    }
}

/// The name the binary has inside a release archive.
fn binary_name(platform: zed::Os) -> &'static str {
    match platform {
        zed::Os::Windows => "sqruff.exe",
        zed::Os::Mac | zed::Os::Linux => "sqruff",
    }
}

/// Finds a binary left behind by an earlier download. Only the most recently
/// downloaded version is kept on disk, so at most one match exists.
fn downloaded_binary(binary_name: &str) -> Option<String> {
    fs::read_dir(".")
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let dir = entry.file_name().into_string().ok()?;
            dir.starts_with("sqruff-").then_some(dir)
        })
        .map(|dir| format!("{dir}/{binary_name}"))
        .find(|path| fs::metadata(path).is_ok_and(|stat| stat.is_file()))
}

/// Maps the current platform onto the release assets published by
/// `.github/workflows/release.yml`.
fn asset_name(platform: zed::Os, arch: zed::Architecture) -> Result<String> {
    let (slug, extension) = match (platform, arch) {
        (zed::Os::Mac, zed::Architecture::Aarch64) => ("darwin-aarch64", "tar.gz"),
        (zed::Os::Mac, zed::Architecture::X8664) => ("darwin-x86_64", "tar.gz"),
        (zed::Os::Linux, zed::Architecture::Aarch64) => ("linux-aarch64-musl", "tar.gz"),
        (zed::Os::Linux, zed::Architecture::X8664) => ("linux-x86_64-musl", "tar.gz"),
        (zed::Os::Windows, zed::Architecture::X8664) => ("windows-x86_64", "zip"),
        (platform, arch) => {
            return Err(format!(
                "sqruff publishes no binary for {os} {arch}; install sqruff manually \
                 and set `lsp.sqruff.binary.path` in your Zed settings",
                os = os_name(platform),
                arch = arch_name(arch),
            ));
        }
    };

    Ok(format!("sqruff-{slug}.{extension}"))
}

fn os_name(platform: zed::Os) -> &'static str {
    match platform {
        zed::Os::Mac => "macOS",
        zed::Os::Linux => "Linux",
        zed::Os::Windows => "Windows",
    }
}

fn arch_name(arch: zed::Architecture) -> &'static str {
    match arch {
        zed::Architecture::Aarch64 => "aarch64",
        zed::Architecture::X86 => "x86",
        zed::Architecture::X8664 => "x86_64",
    }
}

impl zed::Extension for SqruffExtension {
    fn new() -> Self {
        Self::default()
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        if language_server_id.as_ref() != SERVER_ID {
            return Err(format!(
                "unrecognized language server for sqruff: `{language_server_id}`"
            ));
        }

        let binary = LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|settings| settings.binary);
        let (configured_path, configured_args, configured_env) = match binary {
            Some(binary) => (binary.path, binary.arguments, binary.env),
            None => (None, None, None),
        };

        let command = self.binary_path(language_server_id, worktree, configured_path)?;

        Ok(zed::Command {
            command,
            args: configured_args.unwrap_or_else(|| vec!["lsp".to_string()]),
            env: configured_env.map(Vec::from_iter).unwrap_or_default(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree).ok();
        Ok(settings.and_then(|settings| settings.initialization_options))
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree).ok();
        Ok(settings.and_then(|settings| settings.settings))
    }
}

zed::register_extension!(SqruffExtension);
