#!/usr/bin/env bash
set -euo pipefail

# cc-boot installer — auto-installs Node.js LTS if missing, then runs cc-boot
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/wh000wh000/cc-boot/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/wh000wh000/cc-boot/main/install.sh | bash -s -- init --all -s -p 302ai -k "sk-xxx"
#   bash install.sh --node-version 20 init --all   # pin a specific Node.js major version

REQUIRED_NODE_MAJOR=18
PREFERRED_NODE_MAJOR=22  # Current LTS
CC_BOOT_PKG="@haibane/cc-boot"

# --- Parse --node-version before any other args ---
_PASSTHROUGH_ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --node-version)
      PREFERRED_NODE_MAJOR="$2"
      shift 2
      ;;
    --node-version=*)
      PREFERRED_NODE_MAJOR="${1#--node-version=}"
      shift
      ;;
    *)
      _PASSTHROUGH_ARGS+=("$1")
      shift
      ;;
  esac
done
set -- "${_PASSTHROUGH_ARGS[@]+"${_PASSTHROUGH_ARGS[@]}"}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { printf "${CYAN}[cc-boot]${NC} %s\n" "$*"; }
ok()    { printf "${GREEN}[cc-boot]${NC} %s\n" "$*"; }
warn()  { printf "${YELLOW}[cc-boot]${NC} %s\n" "$*"; }
fail()  { printf "${RED}[cc-boot]${NC} %s\n" "$*"; exit 1; }
banner() {
  printf "\n${BOLD}${CYAN}"
  printf "   ╔══════════════════════════════════╗\n"
  printf "   ║         cc-boot installer        ║\n"
  printf "   ║   One-command AI coding setup    ║\n"
  printf "   ╚══════════════════════════════════╝\n"
  printf "${NC}\n"
}

# Detect OS and architecture
detect_os() {
  case "$(uname -s)" in
    Darwin*)  echo "macos" ;;
    Linux*)   echo "linux" ;;
    MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
    *)        echo "unknown" ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64)  echo "x64" ;;
    aarch64|arm64) echo "arm64" ;;
    armv7l)        echo "armv7l" ;;
    *)             echo "$(uname -m)" ;;
  esac
}

# Check if a command exists
has() { command -v "$1" &>/dev/null; }

# Get Node.js major version
node_major() {
  node --version 2>/dev/null | sed 's/^v//' | cut -d. -f1
}

# Install Node.js via the best available method
# Priority: fnm > nvm > brew (macOS) > official binary
install_node() {
  local os="$1"
  local target_major="${PREFERRED_NODE_MAJOR}"
  info "Installing Node.js v${target_major} LTS..."

  # Try fnm first (fast, cross-platform)
  if has fnm; then
    info "Using fnm to install Node.js ${target_major}..."
    fnm install "${target_major}" --lts
    fnm use "${target_major}"
    eval "$(fnm env --shell bash)"
    return 0
  fi

  # Try nvm
  if [ -s "${NVM_DIR:-$HOME/.nvm}/nvm.sh" ]; then
    info "Using nvm to install Node.js ${target_major}..."
    # shellcheck disable=SC1091
    . "${NVM_DIR:-$HOME/.nvm}/nvm.sh"
    nvm install "${target_major}"
    nvm use "${target_major}"
    return 0
  fi

  # OS-specific methods
  case "$os" in
    macos)
      if has brew; then
        info "Using Homebrew to install Node.js ${target_major}..."
        brew install "node@${target_major}"
        # Link if not already linked
        brew link --overwrite "node@${target_major}" 2>/dev/null || true
        return 0
      fi
      # Install fnm then use it
      info "Installing fnm (Fast Node Manager)..."
      curl -fsSL https://fnm.vercel.app/install | bash -s -- --skip-shell
      export PATH="$HOME/.local/share/fnm:$PATH"
      eval "$(fnm env --shell bash)"
      fnm install "${target_major}" --lts
      fnm use "${target_major}"
      ok "fnm installed — add 'eval \"\$(fnm env)\"' to your shell profile"
      return 0
      ;;
    linux)
      # Try direct binary install first (fastest, no deps)
      local arch
      arch="$(detect_arch)"
      local node_url="https://nodejs.org/dist/latest-v${target_major}.x/"

      # Get exact version from nodejs.org
      local version_line
      version_line=$(curl -fsSL "${node_url}" 2>/dev/null | grep -oP "node-v${target_major}\.\d+\.\d+" | head -1 || true)

      if [ -n "$version_line" ]; then
        local tarball="${version_line}-linux-${arch}.tar.xz"
        local download_url="${node_url}${tarball}"
        info "Downloading ${tarball}..."

        if curl -fsSL "${download_url}" -o "/tmp/${tarball}" 2>/dev/null; then
          local install_dir="/usr/local"
          if [ ! -w "$install_dir" ]; then
            install_dir="$HOME/.local"
            mkdir -p "$install_dir"
          fi

          info "Extracting to ${install_dir}..."
          tar -xJf "/tmp/${tarball}" -C "$install_dir" --strip-components=1
          rm -f "/tmp/${tarball}"

          # Ensure PATH
          if [[ ":$PATH:" != *":${install_dir}/bin:"* ]]; then
            export PATH="${install_dir}/bin:$PATH"
          fi

          if has node; then
            return 0
          fi
        fi
      fi

      # Fallback: install fnm
      info "Falling back to fnm..."
      curl -fsSL https://fnm.vercel.app/install | bash -s -- --skip-shell
      export PATH="$HOME/.local/share/fnm:$PATH"
      eval "$(fnm env --shell bash)"
      fnm install "${target_major}" --lts
      fnm use "${target_major}"
      ok "fnm installed — add 'eval \"\$(fnm env)\"' to your shell profile"
      return 0
      ;;
    *)
      fail "Unsupported OS: ${os}. Please install Node.js >= ${REQUIRED_NODE_MAJOR} manually: https://nodejs.org"
      ;;
  esac
}

main() {
  banner

  local os arch
  os="$(detect_os)"
  arch="$(detect_arch)"
  info "Platform: ${os}/${arch}"
  info "Target Node.js: v${PREFERRED_NODE_MAJOR}.x"

  # Check Node.js
  if has node; then
    local major
    major="$(node_major)"
    if [ "$major" -ge "$PREFERRED_NODE_MAJOR" ]; then
      ok "Node.js v$(node --version | sed 's/^v//') ✓ (meets target v${PREFERRED_NODE_MAJOR})"
    elif [ "$major" -ge "$REQUIRED_NODE_MAJOR" ]; then
      warn "Node.js v$(node --version | sed 's/^v//') is below target v${PREFERRED_NODE_MAJOR} — installing..."
      install_node "$os"
    else
      warn "Node.js v$(node --version | sed 's/^v//') is too old (need >= ${REQUIRED_NODE_MAJOR})"
      install_node "$os"
    fi
  else
    install_node "$os"
  fi

  # Verify node is available
  if ! has node; then
    fail "Node.js installation failed. Please install manually: https://nodejs.org"
  fi

  ok "Node.js v$(node --version | sed 's/^v//') ready"

  # Verify npm is available
  if ! has npm; then
    fail "npm not found. Please reinstall Node.js: https://nodejs.org"
  fi

  # Run cc-boot
  info "Launching cc-boot..."
  echo
  npx "${CC_BOOT_PKG}@latest" "$@"
}

main "$@"
