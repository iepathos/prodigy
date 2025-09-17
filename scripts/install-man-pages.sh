#!/usr/bin/env bash
# Install man pages for Prodigy

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Detect OS
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    OS="linux"
    MAN_DIR="/usr/local/share/man/man1"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    OS="macos"
    MAN_DIR="/usr/local/share/man/man1"
elif [[ "$OSTYPE" == "freebsd"* ]]; then
    OS="freebsd"
    MAN_DIR="/usr/local/man/man1"
else
    echo -e "${RED}Unsupported operating system: $OSTYPE${NC}"
    exit 1
fi

# Check if running as root (for system-wide installation)
if [[ $EUID -eq 0 ]]; then
   INSTALL_DIR="$MAN_DIR"
else
   # Install to user directory if not root
   INSTALL_DIR="$HOME/.local/share/man/man1"
   echo -e "${YELLOW}Not running as root, installing to user directory: $INSTALL_DIR${NC}"
fi

# Create installation directory if it doesn't exist
mkdir -p "$INSTALL_DIR"

# Find the man pages (they should be in target/man after building)
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
MAN_SOURCE_DIR="$PROJECT_ROOT/target/man"

# Build the project if man pages don't exist
if [[ ! -d "$MAN_SOURCE_DIR" ]] || [[ -z "$(ls -A "$MAN_SOURCE_DIR" 2>/dev/null)" ]]; then
    echo -e "${YELLOW}Man pages not found. Building project...${NC}"
    cd "$PROJECT_ROOT"
    cargo build --release

    # Check again
    if [[ ! -d "$MAN_SOURCE_DIR" ]] || [[ -z "$(ls -A "$MAN_SOURCE_DIR" 2>/dev/null)" ]]; then
        echo -e "${RED}Failed to generate man pages during build.${NC}"
        echo "Please ensure you have the latest version and try again."
        exit 1
    fi
fi

# Copy man pages
echo "Installing man pages to $INSTALL_DIR..."
cp -f "$MAN_SOURCE_DIR"/*.1 "$INSTALL_DIR/" 2>/dev/null || true
cp -f "$MAN_SOURCE_DIR"/*.1.gz "$INSTALL_DIR/" 2>/dev/null || true

# Count installed pages
INSTALLED_COUNT=$(ls -1 "$INSTALL_DIR"/prodigy*.1* 2>/dev/null | wc -l)

if [[ $INSTALLED_COUNT -gt 0 ]]; then
    echo -e "${GREEN}Successfully installed $INSTALLED_COUNT man pages.${NC}"

    # Update man database if makewhatis/mandb is available
    if command -v makewhatis &> /dev/null; then
        echo "Updating man database..."
        makewhatis "$INSTALL_DIR" 2>/dev/null || true
    elif command -v mandb &> /dev/null; then
        echo "Updating man database..."
        mandb 2>/dev/null || true
    fi

    # Add to MANPATH if installing to user directory
    if [[ "$INSTALL_DIR" == "$HOME/.local/share/man/man1" ]]; then
        echo ""
        echo -e "${YELLOW}To use the man pages, add this to your shell configuration:${NC}"
        echo "export MANPATH=\"\$HOME/.local/share/man:\$MANPATH\""
        echo ""
        echo "Then reload your shell or run: source ~/.bashrc (or ~/.zshrc)"
    fi

    echo ""
    echo "You can now use commands like:"
    echo "  man prodigy"
    echo "  man prodigy-run"
    echo "  man prodigy-exec"
    echo ""
else
    echo -e "${RED}Failed to install man pages.${NC}"
    exit 1
fi