"""Module extension for downloading platform-specific tool binaries."""

# Tool configurations with platform-specific URLs and checksums
_RATCHET_VERSION = "0.11.4"
_RATCHET_PLATFORMS = {
    "linux_amd64": {
        "url": "https://github.com/sethvargo/ratchet/releases/download/v{version}/ratchet_{version}_linux_amd64.tar.gz",
        "sha256": "7141236c5500dce440bb764a964c9d9d8130a3a421604c75b7f7fbaa55cf89f5",
    },
    "linux_arm64": {
        "url": "https://github.com/sethvargo/ratchet/releases/download/v{version}/ratchet_{version}_linux_arm64.tar.gz",
        "sha256": "11050a91f2531d65d76d463e710263270a33b5d8b4cc3ec258c58a835a2bb58c",
    },
    "darwin_amd64": {
        "url": "https://github.com/sethvargo/ratchet/releases/download/v{version}/ratchet_{version}_darwin_amd64.tar.gz",
        "sha256": "78756b000dee07e4d32e3c2bf518e81e971a6cef56627d9eafc25afef7644d57",
    },
    "darwin_arm64": {
        "url": "https://github.com/sethvargo/ratchet/releases/download/v{version}/ratchet_{version}_darwin_arm64.tar.gz",
        "sha256": "319f4c35b818f8d0f42467960e50fbd9d62032ae3bb170aa5aec00985e613336",
    },
}

_MACHETE_VERSION = "0.9.1"
_MACHETE_PLATFORMS = {
    "linux_amd64": {
        "url": "https://github.com/bnjbvr/cargo-machete/releases/download/v{version}/cargo-machete-v{version}-x86_64-unknown-linux-musl.tar.gz",
        "sha256": "640b0814480b401e4e72201e52a13e1311b8eb8d7c27faa08bbe9886f252f26d",
        "strip_prefix": "cargo-machete-v{version}-x86_64-unknown-linux-musl",
    },
    "linux_arm64": {
        "url": "https://github.com/bnjbvr/cargo-machete/releases/download/v{version}/cargo-machete-v{version}-aarch64-unknown-linux-musl.tar.gz",
        "sha256": "0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5",
        "strip_prefix": "cargo-machete-v{version}-aarch64-unknown-linux-musl",
    },
    "darwin_amd64": {
        "url": "https://github.com/bnjbvr/cargo-machete/releases/download/v{version}/cargo-machete-v{version}-x86_64-apple-darwin.tar.gz",
        "sha256": "fd0c0dbcc9db0c1b8745fe9dc4f273d838b04391c6e487d1146957bf687a9703",
        "strip_prefix": "cargo-machete-v{version}-x86_64-apple-darwin",
    },
    "darwin_arm64": {
        "url": "https://github.com/bnjbvr/cargo-machete/releases/download/v{version}/cargo-machete-v{version}-aarch64-apple-darwin.tar.gz",
        "sha256": "72355383848acb154060e6fea2d5b7d58a825ed49fef157b46a6fe25180f8638",
        "strip_prefix": "cargo-machete-v{version}-aarch64-apple-darwin",
    },
}

_UV_VERSION = "0.9.22"
_UV_PLATFORMS = {
    "linux_amd64": {
        "url": "https://github.com/astral-sh/uv/releases/download/{version}/uv-x86_64-unknown-linux-gnu.tar.gz",
        "sha256": "e170aed70ac0225feee612e855d3a57ae73c61ffb22c7e52c3fd33b87c286508",
        "strip_prefix": "uv-x86_64-unknown-linux-gnu",
    },
    "linux_arm64": {
        "url": "https://github.com/astral-sh/uv/releases/download/{version}/uv-aarch64-unknown-linux-gnu.tar.gz",
        "sha256": "2f8716c407d5da21b8a3e8609ed358147216aaab28b96b1d6d7f48e9bcc6254e",
        "strip_prefix": "uv-aarch64-unknown-linux-gnu",
    },
    "darwin_amd64": {
        "url": "https://github.com/astral-sh/uv/releases/download/{version}/uv-x86_64-apple-darwin.tar.gz",
        "sha256": "c0057ad78b475f343739b1bbe223361c1054524c9edf310ee1dc85a050207f86",
        "strip_prefix": "uv-x86_64-apple-darwin",
    },
    "darwin_arm64": {
        "url": "https://github.com/astral-sh/uv/releases/download/{version}/uv-aarch64-apple-darwin.tar.gz",
        "sha256": "4bfc6dacc9bcc9e433a9214a658495ca082b94fd607949b6745a955f34ccbc3c",
        "strip_prefix": "uv-aarch64-apple-darwin",
    },
}

def _get_platform(repository_ctx):
    """Detect the current platform."""
    os_name = repository_ctx.os.name.lower()
    arch = repository_ctx.os.arch

    if "mac" in os_name or "darwin" in os_name:
        os_key = "darwin"
    elif "linux" in os_name:
        os_key = "linux"
    else:
        fail("Unsupported OS: {}".format(os_name))

    if arch == "aarch64" or arch == "arm64":
        arch_key = "arm64"
    elif arch == "x86_64" or arch == "amd64":
        arch_key = "amd64"
    else:
        fail("Unsupported architecture: {}".format(arch))

    return "{}_{}".format(os_key, arch_key)

def _ratchet_repo_impl(repository_ctx):
    platform = _get_platform(repository_ctx)
    config = _RATCHET_PLATFORMS.get(platform)
    if not config:
        fail("Unsupported platform for ratchet: {}".format(platform))

    url = config["url"].format(version = _RATCHET_VERSION)

    repository_ctx.download_and_extract(
        url = url,
        sha256 = config["sha256"],
    )
    repository_ctx.file("BUILD.bazel", 'exports_files(["ratchet"])')

_ratchet_repo = repository_rule(
    implementation = _ratchet_repo_impl,
    attrs = {},
)

def _cargo_machete_repo_impl(repository_ctx):
    platform = _get_platform(repository_ctx)
    config = _MACHETE_PLATFORMS.get(platform)
    if not config:
        fail("Unsupported platform for cargo-machete: {}".format(platform))

    url = config["url"].format(version = _MACHETE_VERSION)
    strip_prefix = config["strip_prefix"].format(version = _MACHETE_VERSION)

    repository_ctx.download_and_extract(
        url = url,
        sha256 = config["sha256"],
        stripPrefix = strip_prefix,
    )
    repository_ctx.file("BUILD.bazel", 'exports_files(["cargo-machete"])')

_cargo_machete_repo = repository_rule(
    implementation = _cargo_machete_repo_impl,
    attrs = {},
)

def _uv_repo_impl(repository_ctx):
    platform = _get_platform(repository_ctx)
    config = _UV_PLATFORMS.get(platform)
    if not config:
        fail("Unsupported platform for uv: {}".format(platform))

    url = config["url"].format(version = _UV_VERSION)
    strip_prefix = config["strip_prefix"]

    repository_ctx.download_and_extract(
        url = url,
        sha256 = config["sha256"],
        stripPrefix = strip_prefix,
    )
    repository_ctx.file("BUILD.bazel", 'exports_files(["uv"])')

_uv_repo = repository_rule(
    implementation = _uv_repo_impl,
    attrs = {},
)

def _tools_impl(module_ctx):
    _ratchet_repo(name = "ratchet")
    _cargo_machete_repo(name = "cargo_machete")
    _uv_repo(name = "uv")

tools = module_extension(
    implementation = _tools_impl,
)
