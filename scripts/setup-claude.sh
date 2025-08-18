#!/bin/bash

# Setup Claude Code MCP Configuration Script
# This script adds the bevy-debugger-mcp server to your Claude Code configuration

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CLAUDE_CONFIG_FILE="${HOME}/.claude.json"
BACKUP_FILE="${CLAUDE_CONFIG_FILE}.backup.$(date +%Y%m%d_%H%M%S)"

# Functions
print_error() {
    echo -e "${RED}ERROR: $1${NC}" >&2
}

print_success() {
    echo -e "${GREEN}SUCCESS: $1${NC}"
}

print_info() {
    echo -e "${BLUE}INFO: $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}WARNING: $1${NC}"
}

check_requirements() {
    print_info "Checking requirements..."
    
    # Check if Claude config file exists
    if [[ ! -f "${CLAUDE_CONFIG_FILE}" ]]; then
        print_error "Claude Code configuration file not found: ${CLAUDE_CONFIG_FILE}"
        print_info "Please run Claude Code first to create the configuration file"
        exit 1
    fi
    
    # Check if jq is available
    if ! command -v jq &> /dev/null; then
        print_error "jq is required but not installed. Please install jq first:"
        print_info "  brew install jq"
        exit 1
    fi
    
    # Check if bevy-debugger-mcp binary exists
    if ! command -v bevy-debugger-mcp &> /dev/null; then
        print_error "bevy-debugger-mcp not found in PATH"
        print_info "Please install bevy-debugger-mcp first or run: ./scripts/install.sh"
        exit 1
    fi
    
    print_success "Requirements check passed"
}

backup_config() {
    print_info "Creating backup of Claude configuration..."
    cp "${CLAUDE_CONFIG_FILE}" "${BACKUP_FILE}"
    print_success "Backup created: ${BACKUP_FILE}"
}

add_mcp_server() {
    print_info "Adding bevy-debugger-mcp server to Claude Code configuration..."
    
    # Get the binary path
    local binary_path=$(which bevy-debugger-mcp)
    print_info "Binary path: ${binary_path}"
    
    # Check if MCP server already exists
    if jq -e '.mcpServers["bevy-debugger-mcp"]' "${CLAUDE_CONFIG_FILE}" &> /dev/null; then
        print_warning "bevy-debugger-mcp already exists in configuration"
        read -p "Do you want to update it? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            print_info "Configuration update cancelled"
            return 0
        fi
    fi
    
    # Create the MCP server configuration
    local mcp_config=$(cat <<EOF
{
  "command": "${binary_path}",
  "args": ["serve"],
  "env": {
    "BEVY_BRP_HOST": "localhost",
    "BEVY_BRP_PORT": "15702",
    "MCP_PORT": "3000",
    "RUST_LOG": "info"
  }
}
EOF
)
    
    # Add the configuration using jq
    local temp_file=$(mktemp)
    jq --argjson config "${mcp_config}" '.mcpServers["bevy-debugger-mcp"] = $config' "${CLAUDE_CONFIG_FILE}" > "${temp_file}"
    
    # Verify the JSON is valid
    if jq empty "${temp_file}" 2>/dev/null; then
        mv "${temp_file}" "${CLAUDE_CONFIG_FILE}"
        print_success "MCP server configuration added successfully"
    else
        print_error "Failed to add MCP server configuration (invalid JSON)"
        rm -f "${temp_file}"
        exit 1
    fi
}

verify_configuration() {
    print_info "Verifying configuration..."
    
    # Check if the configuration was added
    if jq -e '.mcpServers["bevy-debugger-mcp"]' "${CLAUDE_CONFIG_FILE}" &> /dev/null; then
        print_success "Configuration verified successfully"
        
        # Show the configuration
        print_info "MCP Server Configuration:"
        jq '.mcpServers["bevy-debugger-mcp"]' "${CLAUDE_CONFIG_FILE}"
    else
        print_error "Configuration verification failed"
        exit 1
    fi
}

show_next_steps() {
    echo
    echo "========================================="
    echo "  🎉 Claude Code Setup Complete!"
    echo "========================================="
    echo
    echo "The bevy-debugger-mcp server has been added to your Claude Code configuration."
    echo
    echo "📋 Next Steps:"
    echo "  1. Restart Claude Code if it's running"
    echo "  2. Start your Bevy game with RemotePlugin:"
    echo "     cargo run"
    echo "  3. The MCP server should connect automatically"
    echo "  4. Start debugging with Claude Code!"
    echo
    echo "🎮 Example Commands:"
    echo "  • \"Show me all entities in the game\""
    echo "  • \"Monitor the player's health\""
    echo "  • \"Start recording this session\""
    echo "  • \"Find what's causing FPS drops\""
    echo
    echo "🔧 Service Management:"
    echo "  ./scripts/service.sh start    # Start background service"
    echo "  ./scripts/service.sh stop     # Stop background service"
    echo "  ./scripts/service.sh status   # Check service status"
    echo
    echo "📖 Documentation:"
    echo "  • README.md - Full documentation"
    echo "  • docs/USAGE_GUIDE.md - Advanced usage"
    echo "  • docs/CLAUDE_SUBAGENT_GUIDE.md - Prompt guide"
    echo
}

# Main setup flow
main() {
    echo
    echo "╔══════════════════════════════════════════╗"
    echo "║     Claude Code MCP Setup Script        ║"
    echo "║         bevy-debugger-mcp                ║"
    echo "╚══════════════════════════════════════════╝"
    echo
    
    check_requirements
    backup_config
    add_mcp_server
    verify_configuration
    show_next_steps
}

# Handle script arguments
case "${1:-}" in
    --help|-h)
        echo "Claude Code MCP Setup Script - bevy-debugger-mcp"
        echo
        echo "Usage: $0 [options]"
        echo
        echo "Options:"
        echo "  --help, -h        Show this help"
        echo
        echo "This script will:"
        echo "  1. Check if Claude Code configuration exists"
        echo "  2. Backup existing configuration"
        echo "  3. Add bevy-debugger-mcp server to mcpServers"
        echo "  4. Verify the configuration"
        echo
        echo "Requirements:"
        echo "  • Claude Code installed and run at least once"
        echo "  • jq installed (brew install jq)"
        echo "  • bevy-debugger-mcp installed"
        echo
        exit 0
        ;;
    "")
        # No arguments - proceed with setup
        main
        ;;
    *)
        print_error "Unknown option: $1"
        echo "Use --help for usage information"
        exit 1
        ;;
esac