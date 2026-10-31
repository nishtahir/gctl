#!/bin/bash
# shellcheck disable=SC2016

set -e

help() {
  echo "Usage: install.sh [--release <release>] [--install-dir <dir>] [--skip-shell]"
  echo ""
  echo "Options:"
  echo "  --release <release>    The release to install, e.g. v0.1.0. Defaults to latest."
  echo "  --install-dir <dir>    The directory to install gctl to. Defaults to ~/.gctl/bin."
  echo "  --skip-shell           Skip adding the install directory to your shell profile."
  echo "  --help                 Print this help message."
}

INSTALL_DIR="$HOME/.gctl/bin"
RELEASE="latest"
SKIP_SHELL=0

OS="$(uname -s)"
ARCH="$(uname -m)"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release)
      RELEASE="$2"
      shift
      shift
      ;;
    --install-dir)
      INSTALL_DIR="$2"
      shift
      shift
      ;;
    --skip-shell)
      SKIP_SHELL=1
      shift
      ;;
    --help)
      help
      exit 0
      ;;
    *)
      echo "Unknown argument: $1"
      help
      exit 1
      ;;
  esac
done

if [[ "$OS" == "Darwin" ]]; then
  OS="darwin"
elif [[ "$OS" == "Linux" ]]; then
  OS="linux"
else
  echo "Unsupported OS: $OS"
  exit 1
fi

if [[ "$ARCH" == "x86_64" ]]; then
  ARCH="amd64"
elif [[ "$ARCH" == "arm64" || "$ARCH" == "aarch64" ]]; then
  ARCH="arm64"
else
  echo "Unsupported architecture: $ARCH"
  exit 1
fi

FILENAME="gctl-$OS-$ARCH"
if [ -n "$GCTL_DOWNLOAD_URL" ]; then
  URL="$GCTL_DOWNLOAD_URL"
elif [ "$RELEASE" = "latest" ]; then
  URL="https://github.com/nishtahir/gctl/releases/latest/download/$FILENAME"
else
  URL="https://github.com/nishtahir/gctl/releases/download/$RELEASE/$FILENAME"
fi

echo "Downloading $URL..."

DOWNLOAD_DIR=$(mktemp -d)
trap 'rm -rf "$DOWNLOAD_DIR"' EXIT
mkdir -p "$INSTALL_DIR"

if ! curl --progress-bar --fail -L "$URL" -o "$DOWNLOAD_DIR/$FILENAME"; then
  echo "Download failed. Check that the release/filename are correct."
  exit 1
fi

# Verify the download when the release publishes a checksum for it.
if curl --silent --fail -L "$URL.sha256" -o "$DOWNLOAD_DIR/$FILENAME.sha256"; then
  EXPECTED="$(awk '{print $1}' "$DOWNLOAD_DIR/$FILENAME.sha256")"
  ACTUAL="$(shasum -a 256 "$DOWNLOAD_DIR/$FILENAME" | awk '{print $1}')"
  if [ "$EXPECTED" != "$ACTUAL" ]; then
    echo "Checksum mismatch for $FILENAME."
    exit 1
  fi
fi

mv "$DOWNLOAD_DIR/$FILENAME" "$INSTALL_DIR/gctl"
chmod u+x "$INSTALL_DIR/gctl"
echo "Installed gctl to $INSTALL_DIR/gctl"

if [[ "$SKIP_SHELL" == "1" ]]; then
  echo "Skipping shell profile setup."
  exit 0
fi

CURRENT_SHELL="$(basename "$SHELL")"
MARKER="# gctl https://github.com/nishtahir/gctl/"

# Bash reads ~/.bashrc for interactive non-login shells (most Linux terminals) but only the
# first of ~/.bash_profile, ~/.bash_login and ~/.profile for login shells (every macOS
# terminal). Use whichever file the shell is going to read.
bash_conf_file() {
  if [ "$OS" = "darwin" ]; then
    for candidate in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
      if [ -f "$candidate" ]; then
        echo "$candidate"
        return
      fi
    done
    echo "$HOME/.bash_profile"
  else
    echo "$HOME/.bashrc"
  fi
}

# append_block <conf file> <shell name> <line>...
# Does nothing if the file already has a gctl block, so running the installer again
# (e.g. to upgrade) doesn't stack up duplicates.
append_block() {
  local CONF_FILE="$1" SHELL_NAME="$2"
  shift 2

  mkdir -p "$(dirname "$CONF_FILE")"
  if [ -f "$CONF_FILE" ] && grep -qF "$MARKER" "$CONF_FILE"; then
    echo "gctl is already set up for $SHELL_NAME in $CONF_FILE. Skipping."
    return
  fi

  echo "Appending the following to $CONF_FILE:"
  echo ''
  {
    echo ''
    echo "$MARKER"
    for line in "$@"; do
      echo "$line"
    done
  } | tee -a "$CONF_FILE" | sed 's/^/  /'
}

if [ "$CURRENT_SHELL" = "zsh" ]; then
  append_block "${ZDOTDIR:-$HOME}/.zshrc" "Zsh" \
    'export PATH="'"$INSTALL_DIR"':$PATH"'

elif [ "$CURRENT_SHELL" = "fish" ]; then
  append_block "$HOME/.config/fish/conf.d/gctl.fish" "Fish" \
    'set -gx PATH "'"$INSTALL_DIR"'" $PATH'

elif [ "$CURRENT_SHELL" = "bash" ]; then
  append_block "$(bash_conf_file)" "Bash" \
    'export PATH="'"$INSTALL_DIR"':$PATH"'

else
  echo "Unsupported shell '$CURRENT_SHELL'. Add the following to your shell profile:"
  echo ''
  echo '  export PATH="'"$INSTALL_DIR"':$PATH"'
fi

echo ''
echo "Restart your shell, then run \`gctl auth login\` to get started."
