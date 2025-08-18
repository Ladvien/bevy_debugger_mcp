#!/bin/bash

# Bevy Debugger MCP Server - Uninstaller Script
# This script removes the bevy-debugger-mcp service and all related files

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SERVICE_NAME="bevy-debugger-mcp"
PLIST_NAME="com.${SERVICE_NAME}.plist"
BINARY_NAME="bevy-debugger-mcp"
CONFIG_DIR="${HOME}/.config/bevy-debugger"
SUPPORT_DIR="${HOME}/Library/Application Support/bevy-debugger"
LOG_DIR="${HOME}/Library/Logs/bevy-debugger"
LAUNCHAGENT_DIR="${HOME}/Library/LaunchAgents"

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

confirm_uninstall() {
    echo "This will remove the Bevy Debugger MCP Server and all related files:"
    echo "  • Service: ${SERVICE_NAME}"
    echo "  • Binary: /usr/local/bin/${BINARY_NAME}"
    echo "  • LaunchAgent: ${LAUNCHAGENT_DIR}/${PLIST_NAME}"
    echo "  • Configuration: ${CONFIG_DIR}"
    echo "  • Logs: ${LOG_DIR}"
    echo "  • Support files: ${SUPPORT_DIR}"
    echo
    
    read -p "Are you sure you want to continue? (y/N): " -n 1 -r
    echo
    
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_info "Uninstallation cancelled"
        exit 0
    fi
}

stop_service() {
    print_info "Stopping service..."
    
    # Check if service is running
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        print_info "Service is running, stopping it..."
        
        # Unload the service
        if launchctl unload "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
            print_success "Service stopped successfully"
        else
            print_warning "Failed to stop service gracefully"
        fi
        
        # Wait a moment for graceful shutdown
        sleep 2
        
        # Force kill if still running
        local pids=$(pgrep -f "${BINARY_NAME}" 2>/dev/null || true)
        if [[ -n "${pids}" ]]; then
            print_warning "Force stopping remaining processes..."
            echo "${pids}" | xargs kill -TERM 2>/dev/null || true
            sleep 2
            echo "${pids}" | xargs kill -KILL 2>/dev/null || true
        fi
    else
        print_info "Service is not running"
    fi
}

remove_launchagent() {
    print_info "Removing LaunchAgent..."
    
    local plist_path="${LAUNCHAGENT_DIR}/${PLIST_NAME}"
    
    if [[ -f "${plist_path}" ]]; then
        # Make sure it's unloaded
        launchctl unload "${plist_path}" 2>/dev/null || true
        
        # Remove the plist file
        rm -f "${plist_path}"
        print_success "LaunchAgent removed: ${plist_path}"
    else
        print_info "LaunchAgent not found: ${plist_path}"
    fi
}

remove_binary() {
    print_info "Removing binary..."
    
    local binary_path="/usr/local/bin/${BINARY_NAME}"
    
    if [[ -f "${binary_path}" ]]; then
        # Check if we need sudo
        if [[ -w "${binary_path}" ]]; then
            rm -f "${binary_path}"
        else
            print_info "Removing binary requires sudo permissions..."
            sudo rm -f "${binary_path}"
        fi
        print_success "Binary removed: ${binary_path}"
    else
        print_info "Binary not found: ${binary_path}"
    fi
}

remove_directories() {
    print_info "Removing directories and files..."
    
    # Ask about configuration and logs
    local remove_config=false
    local remove_logs=false
    
    if [[ -d "${CONFIG_DIR}" ]]; then
        read -p "Remove configuration directory? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            remove_config=true
        fi
    fi
    
    if [[ -d "${LOG_DIR}" ]]; then
        read -p "Remove log directory? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            remove_logs=true
        fi
    fi
    
    # Remove support directory (always)
    if [[ -d "${SUPPORT_DIR}" ]]; then
        rm -rf "${SUPPORT_DIR}"
        print_success "Support directory removed: ${SUPPORT_DIR}"
    fi
    
    # Remove configuration directory (if requested)
    if [[ "${remove_config}" == true ]] && [[ -d "${CONFIG_DIR}" ]]; then
        rm -rf "${CONFIG_DIR}"
        print_success "Configuration directory removed: ${CONFIG_DIR}"
    elif [[ -d "${CONFIG_DIR}" ]]; then
        print_info "Configuration directory preserved: ${CONFIG_DIR}"
    fi
    
    # Remove log directory (if requested)
    if [[ "${remove_logs}" == true ]] && [[ -d "${LOG_DIR}" ]]; then
        rm -rf "${LOG_DIR}"
        print_success "Log directory removed: ${LOG_DIR}"
    elif [[ -d "${LOG_DIR}" ]]; then
        print_info "Log directory preserved: ${LOG_DIR}"
    fi
}

remove_scripts() {
    print_info "Checking for installation scripts..."
    
    # Don't remove the scripts directory as user might want to reinstall
    # Just inform about their location
    if [[ -d "scripts" ]]; then
        print_info "Installation scripts preserved in: ./scripts/"
        print_info "Run ./scripts/install.sh to reinstall"
    fi
}

verify_removal() {
    print_info "Verifying removal..."
    
    local issues_found=false
    
    # Check if service is still running
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        print_warning "Service may still be loaded in LaunchAgent"
        issues_found=true
    fi
    
    # Check if binary still exists
    if [[ -f "/usr/local/bin/${BINARY_NAME}" ]]; then
        print_warning "Binary still exists: /usr/local/bin/${BINARY_NAME}"
        issues_found=true
    fi
    
    # Check if LaunchAgent plist still exists
    if [[ -f "${LAUNCHAGENT_DIR}/${PLIST_NAME}" ]]; then
        print_warning "LaunchAgent plist still exists: ${LAUNCHAGENT_DIR}/${PLIST_NAME}"
        issues_found=true
    fi
    
    # Check if processes are still running
    local pids=$(pgrep -f "${BINARY_NAME}" 2>/dev/null || true)
    if [[ -n "${pids}" ]]; then
        print_warning "Processes still running: ${pids}"
        issues_found=true
    fi
    
    if [[ "${issues_found}" == false ]]; then
        print_success "Removal verification passed"
    else
        print_warning "Some components may not have been fully removed"
        print_info "You may need to manually clean up remaining items"
    fi
}

show_completion() {
    echo
    echo "========================================="
    echo "  🗑️  Uninstallation Complete"
    echo "========================================="
    echo
    echo "The Bevy Debugger MCP Server has been uninstalled."
    echo
    
    if [[ -d "${CONFIG_DIR}" ]]; then
        echo "📁 Preserved directories:"
        echo "  • Configuration: ${CONFIG_DIR}"
    fi
    
    if [[ -d "${LOG_DIR}" ]]; then
        echo "  • Logs: ${LOG_DIR}"
    fi
    
    echo
    echo "🔄 To reinstall:"
    echo "  ./scripts/install.sh"
    echo
    echo "🧹 To remove preserved directories manually:"
    if [[ -d "${CONFIG_DIR}" ]]; then
        echo "  rm -rf '${CONFIG_DIR}'"
    fi
    if [[ -d "${LOG_DIR}" ]]; then
        echo "  rm -rf '${LOG_DIR}'"
    fi
    echo
}

# Handle force option
if [[ "${1:-}" == "--force" ]] || [[ "${1:-}" == "-f" ]]; then
    FORCE_UNINSTALL=true
else
    FORCE_UNINSTALL=false
fi

# Main uninstallation flow
main() {
    echo
    echo "╔══════════════════════════════════════════╗"
    echo "║   Bevy Debugger MCP Server Uninstaller  ║"
    echo "╚══════════════════════════════════════════╝"
    echo
    
    if [[ "${FORCE_UNINSTALL}" != true ]]; then
        confirm_uninstall
    fi
    
    stop_service
    remove_launchagent
    remove_binary
    remove_directories
    remove_scripts
    verify_removal
    show_completion
}

# Handle script arguments
case "${1:-}" in
    --help|-h)
        echo "Bevy Debugger MCP Server - Uninstaller Script"
        echo
        echo "Usage: $0 [options]"
        echo
        echo "Options:"
        echo "  --force, -f    Force uninstall without confirmation"
        echo "  --help, -h     Show this help"
        echo
        echo "This script will:"
        echo "  1. Stop the running service"
        echo "  2. Remove the LaunchAgent"
        echo "  3. Remove the binary from /usr/local/bin"
        echo "  4. Optionally remove configuration and logs"
        echo "  5. Verify complete removal"
        echo
        exit 0
        ;;
    --force|-f|"")
        # Valid options - proceed
        main
        ;;
    *)
        print_error "Unknown option: $1"
        echo "Use --help for usage information"
        exit 1
        ;;
esac