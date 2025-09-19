#!/bin/bash

set -e -o pipefail

readonly GREEN="$(tput setaf 2 2>/dev/null || echo '')"
readonly CYAN="$(tput setaf 6 2>/dev/null || echo '')"
readonly NO_COLOR="$(tput sgr0 2>/dev/null || echo '')"

if ! command -v tar >/dev/null 2>&1; then
    echo "Error: tar is required to install sqruff."
    exit 1
fi

# Define the release information
RELEASE_URL="https://api.github.com/repos/quarylabs/sqruff/releases/latest"

# Determine the operating system
OS=$(uname -s)
if [ "$OS" = "Darwin" ]; then
    # Determine the CPU architecture
    ARCH=$(uname -m)
    if [ "$ARCH" = "arm64" ]; then
        ASSET_NAME="-darwin-aarch64.tar.gz"
    else
        ASSET_NAME="-darwin-x86_64.tar.gz"
    fi
elif [ "$OS" = "Linux" ]; then
    # Determine the CPU architecture
    ARCH=$(uname -m)
    if [ "$ARCH" = "aarch64" ]; then
        ASSET_NAME="-aarch64-musl.tar.gz"
    elif [ "$ARCH" = "x86_64" ]; then
        ASSET_NAME="-x86_64-musl.tar.gz"
    else
        echo "Unsupported architecture: $ARCH"
        exit 1
    fi
else
    echo "Unsupported operating system: $OS"
    exit 1
fi

# Retrieve the download URL for the desired asset
DOWNLOAD_URL=$(curl -sSL $RELEASE_URL | grep -o "browser_download_url.*$ASSET_NAME\"" | cut -d ' ' -f 2)

ASSET_NAME=$(basename $DOWNLOAD_URL)

# Define the installation directory
INSTALL_DIR="${1:-/usr/local/bin}"

DOWNLOAD_URL=`echo $DOWNLOAD_URL | tr -d '\"'`

# Download the asset
curl -SL $DOWNLOAD_URL -o /tmp/$ASSET_NAME

# Extract the asset
tar -xzf /tmp/$ASSET_NAME -C /tmp

# Set the correct permissions for the binary
chmod +x /tmp/sqruff

# Move the extracted binary to the installation directory
# use sudo if available
if command -v sudo >/dev/null 2>&1; then
    sudo mv /tmp/sqruff $INSTALL_DIR
else
    mv /tmp/sqruff $INSTALL_DIR
fi

# Clean up temporary files
rm /tmp/$ASSET_NAME

cat << EOF
${CYAN}

   ________  __  _____  ________
  / __/ __ \/ / / / _ \/ __/ __/
 _\ \/ /_/ / /_/ / , _/ _// _/  
/___/\___\_\____/_/|_/_/ /_/    
                                

${NO_COLOR}
A compact, high-speed ${CYAN}SQL linter,${NO_COLOR} engineered with ${CYAN}Rust efficiency${NO_COLOR}.

${GREEN}https://github.com/quarylabs/sqruff${NO_COLOR}

Please file an issue if you encounter any problems!

===============================================================================

Installation completed! 🎉

EOF
