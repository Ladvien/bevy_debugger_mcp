#!/bin/bash

# Bevy Debugger MCP Server - Service Management Script
# Provides convenient commands for managing the LaunchAgent service

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

# Configuration
SERVICE_NAME="bevy-debugger-mcp"
PLIST_NAME="com.${SERVICE_NAME}.plist"
BINARY_NAME="bevy-debugger-mcp"
CONFIG_DIR="${HOME}/.config/bevy-debugger"
SUPPORT_DIR="${HOME}/Library/Application Support/bevy-debugger"
LOG_DIR="${HOME}/Library/Logs/bevy-debugger"
LAUNCHAGENT_DIR="${HOME}/Library/LaunchAgents"
HEALTH_URL="http://localhost:3000/health"

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

print_status() {
    echo -e "${MAGENTA}STATUS: $1${NC}"
}

usage() {
    cat << EOF
Usage: $0 <command> [options]

Service management commands for ${SERVICE_NAME}

Commands:
    start       Start the service
    stop        Stop the service
    restart     Restart the service
    status      Show service status
    logs        Show service logs (tail -f)
    health      Check if MCP server is responding
    reload      Reload service configuration
    enable      Enable service to start at login
    disable     Disable service from starting at login
    pid         Show service PID
    config      Edit configuration file
    info        Show service information
    test        Test connection to Bevy game
    help        Show this help message

Options:
    -h, --help  Show help for specific command
    -f, --force Force operation without confirmation

Examples:
    $0 status           # Check if service is running
    $0 restart          # Restart the service
    $0 logs             # Watch service logs
    $0 test             # Test connection to Bevy game

EOF
}

check_service_installed() {
    if [[ ! -f "${LAUNCHAGENT_DIR}/${PLIST_NAME}" ]]; then
        print_error "Service not installed. Run ./scripts/install.sh first"
        exit 1
    fi
}

service_start() {
    print_info "Starting ${SERVICE_NAME}..."
    check_service_installed
    
    # Check if already running
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        print_warning "Service is already running"
        return 0
    fi
    
    # Load the service
    if launchctl load -w "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
        print_success "Service started successfully"
        sleep 2
        service_status
    else
        print_error "Failed to start service"
        print_info "Check logs: tail -f '${LOG_DIR}/stderr.log'"
        exit 1
    fi
}

service_stop() {
    print_info "Stopping ${SERVICE_NAME}..."
    check_service_installed
    
    # Check if running
    if ! launchctl list | grep -q "${SERVICE_NAME}"; then
        print_warning "Service is not running"
        return 0
    fi
    
    # Unload the service
    if launchctl unload "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
        print_success "Service stopped successfully"
    else
        print_error "Failed to stop service"
        exit 1
    fi
}

service_restart() {
    print_info "Restarting ${SERVICE_NAME}..."
    service_stop
    sleep 2
    service_start
}

service_status() {
    print_info "Checking ${SERVICE_NAME} status..."
    
    # Check LaunchAgent status
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        print_status "Service is RUNNING"
        
        # Get detailed status
        local status_line=$(launchctl list | grep "${SERVICE_NAME}")
        echo "  LaunchAgent: ${status_line}"
        
        # Get PID
        local pid=$(echo "${status_line}" | awk '{print $1}')
        if [[ "${pid}" != "-" ]]; then
            echo "  PID: ${pid}"
            
            # Get process info
            if ps -p "${pid}" > /dev/null 2>&1; then
                local proc_info=$(ps -p "${pid}" -o %cpu,%mem,etime,command | tail -1)
                echo "  Process: ${proc_info}"
            fi
        fi
        
        # Check if MCP port is listening
        if lsof -i :3000 > /dev/null 2>&1; then
            print_status "MCP Server: LISTENING on port 3000"
        else
            print_warning "MCP Server: NOT LISTENING on port 3000"
        fi
        
        # Check if Bevy game is available
        if nc -z localhost 15702 2>/dev/null; then
            print_status "Bevy Game: DETECTED on port 15702"
        else
            print_warning "Bevy Game: NOT DETECTED on port 15702"
        fi
        
    else
        print_status "Service is STOPPED"
        
        # Check if process is running anyway
        if pgrep -f "${BINARY_NAME}" > /dev/null; then
            print_warning "Process found running outside of LaunchAgent"
            local pids=$(pgrep -f "${BINARY_NAME}")
            echo "  PIDs: ${pids}"
        fi
    fi
}

service_logs() {
    print_info "Showing logs for ${SERVICE_NAME}..."
    
    local log_files=(
        "${LOG_DIR}/stderr.log"
        "${LOG_DIR}/stdout.log"
    )
    
    # Find existing log files
    local existing_logs=()
    for log in "${log_files[@]}"; do
        if [[ -f "${log}" ]]; then
            existing_logs+=("${log}")
        fi
    done
    
    if [[ ${#existing_logs[@]} -eq 0 ]]; then
        print_warning "No log files found"
        print_info "Log directory: ${LOG_DIR}"
        return 1
    fi
    
    print_info "Following log files:"
    printf '%s\n' "${existing_logs[@]}"
    echo
    echo "Press Ctrl+C to stop..."
    echo
    
    # Tail all existing logs
    tail -f "${existing_logs[@]}"
}

service_health() {
    print_info "Checking ${SERVICE_NAME} connectivity..."
    
    # Check if MCP server is listening
    if lsof -i :3000 > /dev/null 2>&1; then
        print_success "MCP Server is listening on port 3000"
    else
        print_error "MCP Server is NOT listening on port 3000"
        print_info "Service may not be running"
        return 1
    fi
    
    # Check if Bevy game is available
    if command -v nc &> /dev/null; then
        if nc -z localhost 15702 2>/dev/null; then
            print_success "Bevy game detected on port 15702"
        else
            print_warning "No Bevy game detected on port 15702"
            print_info "Make sure your Bevy game is running with RemotePlugin"
        fi
    fi
    
    # Try to connect to MCP server via netcat if available
    if command -v nc &> /dev/null; then
        print_info "Testing MCP server connection..."
        if echo '{"jsonrpc": "2.0", "method": "tools/list", "id": 1}' | nc localhost 3000 2>/dev/null | grep -q "jsonrpc"; then
            print_success "MCP Server is responding to requests"
        else
            print_warning "MCP Server may not be responding properly"
        fi
    fi
}

service_test() {
    print_info "Testing connection to Bevy game..."
    
    if ! command -v nc &> /dev/null; then
        print_warning "netcat (nc) not available, cannot test connection"
        print_info "Install with: brew install netcat"
        return 1
    fi
    
    # Test WebSocket connection to Bevy BRP
    print_info "Checking Bevy Remote Protocol connection..."
    if nc -z localhost 15702 2>/dev/null; then
        print_success "Bevy game is listening on port 15702"
        
        # Try to send a simple BRP request
        print_info "Testing BRP communication..."
        local test_msg='{"method": "bevy/list", "params": {}}'
        if echo "${test_msg}" | nc localhost 15702 2>/dev/null | head -1 | grep -q "entities"; then
            print_success "Bevy BRP is responding correctly"
        else
            print_warning "Bevy BRP connection established but response unclear"
        fi
    else
        print_error "No Bevy game found on port 15702"
        print_info "Make sure your Bevy game is running with:"
        print_info "  .add_plugins(RemotePlugin::default())"
    fi
}

service_reload() {
    print_info "Reloading ${SERVICE_NAME} configuration..."
    
    # Send SIGHUP to reload config
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        local status_line=$(launchctl list | grep "${SERVICE_NAME}")
        local pid=$(echo "${status_line}" | awk '{print $1}')
        
        if [[ "${pid}" != "-" ]] && [[ -n "${pid}" ]]; then
            print_info "Sending SIGHUP to PID ${pid}..."
            if kill -HUP "${pid}"; then
                print_success "Configuration reload signal sent"
                print_info "Check logs for reload status"
            else
                print_error "Failed to send reload signal"
                exit 1
            fi
        else
            print_warning "Could not find service PID"
            print_info "Performing restart instead..."
            service_restart
        fi
    else
        print_error "Service is not running"
        exit 1
    fi
}

service_enable() {
    print_info "Enabling ${SERVICE_NAME} to start at login..."
    check_service_installed
    
    # Enable the service
    if launchctl load -w "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
        print_success "Service enabled"
    else
        print_warning "Service may already be enabled"
    fi
}

service_disable() {
    print_info "Disabling ${SERVICE_NAME} from starting at login..."
    check_service_installed
    
    # Stop if running
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        service_stop
    fi
    
    # Disable the service
    if launchctl unload -w "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
        print_success "Service disabled"
    else
        print_warning "Service may already be disabled"
    fi
}

service_pid() {
    # Check LaunchAgent
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        local status_line=$(launchctl list | grep "${SERVICE_NAME}")
        local pid=$(echo "${status_line}" | awk '{print $1}')
        
        if [[ "${pid}" != "-" ]] && [[ -n "${pid}" ]]; then
            echo "${pid}"
            return 0
        fi
    fi
    
    # Check by process name
    local pids=$(pgrep -f "${BINARY_NAME}" 2>/dev/null || true)
    if [[ -n "${pids}" ]]; then
        echo "${pids}" | head -1
        return 0
    fi
    
    print_error "Service PID not found"
    return 1
}

service_config() {
    local config_file="${CONFIG_DIR}/config.toml"
    
    if [[ ! -f "${config_file}" ]]; then
        print_warning "Configuration file not found: ${config_file}"
        print_info "Creating default configuration..."
        mkdir -p "${CONFIG_DIR}"
        cat > "${config_file}" << 'EOF'
[connection]
bevy_host = "localhost"
bevy_port = 15702
mcp_port = 3000

[logging]
level = "info"
debug_mode = false

[features]
auto_reconnect = true
checkpoint_interval = 1000
max_recording_size = "100MB"
EOF
        print_success "Default configuration created"
    fi
    
    print_info "Opening configuration file: ${config_file}"
    
    # Use default editor or vi
    local editor="${EDITOR:-vi}"
    
    # Create backup
    local backup="${config_file}.backup.$(date +%Y%m%d_%H%M%S)"
    cp "${config_file}" "${backup}"
    print_info "Backup created: ${backup}"
    
    # Open editor
    "${editor}" "${config_file}"
    
    # Ask to reload
    read -p "Reload service configuration? (y/n): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        service_reload
    fi
}

service_info() {
    echo "========================================="
    echo "   ${SERVICE_NAME} Service Information"
    echo "========================================="
    echo
    echo "Service Name:      ${SERVICE_NAME}"
    echo "Binary:            /usr/local/bin/${BINARY_NAME}"
    echo "LaunchAgent:       ${LAUNCHAGENT_DIR}/${PLIST_NAME}"
    echo "Configuration:     ${CONFIG_DIR}/config.toml"
    echo "Logs:              ${LOG_DIR}/"
    echo "MCP Port:          3000"
    echo "Bevy BRP Port:     15702"
    echo
    
    # Show version if binary exists
    if [[ -f "/usr/local/bin/${BINARY_NAME}" ]]; then
        echo -n "Binary Version:    "
        /usr/local/bin/${BINARY_NAME} --version 2>/dev/null || echo "unknown"
    elif command -v "${BINARY_NAME}" &> /dev/null; then
        echo -n "Binary Version:    "
        ${BINARY_NAME} --version 2>/dev/null || echo "unknown"
    fi
    
    echo
    service_status
}

# Main command dispatcher
main() {
    if [[ $# -eq 0 ]]; then
        usage
        exit 0
    fi
    
    local command="$1"
    shift
    
    case "${command}" in
        start)
            service_start "$@"
            ;;
        stop)
            service_stop "$@"
            ;;
        restart)
            service_restart "$@"
            ;;
        status)
            service_status "$@"
            ;;
        logs|log)
            service_logs "$@"
            ;;
        health)
            service_health "$@"
            ;;
        test)
            service_test "$@"
            ;;
        reload)
            service_reload "$@"
            ;;
        enable)
            service_enable "$@"
            ;;
        disable)
            service_disable "$@"
            ;;
        pid)
            service_pid "$@"
            ;;
        config)
            service_config "$@"
            ;;
        info)
            service_info "$@"
            ;;
        help|-h|--help)
            usage
            ;;
        *)
            print_error "Unknown command: ${command}"
            echo
            usage
            exit 1
            ;;
    esac
}

# Run main function
main "$@"