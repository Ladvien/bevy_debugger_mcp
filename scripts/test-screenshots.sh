#!/bin/bash
# Script to run screenshot E2E tests locally

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔧 Setting up screenshot test environment...${NC}"

# Export test environment variables
export CI=false
export RUST_LOG=debug
export RUST_BACKTRACE=1

# Create test directories
mkdir -p test_output reference_screenshots diffs

echo -e "${BLUE}📋 Running screenshot utility tests...${NC}"
cargo test screenshot_test_utils --lib -- --nocapture

echo -e "${BLUE}🧪 Running E2E screenshot tests...${NC}"

# Run individual test categories
echo -e "${GREEN}Testing basic screenshot functionality...${NC}"
cargo test test_basic_screenshot_functionality -- --exact --nocapture

echo -e "${GREEN}Testing parameter validation...${NC}"
cargo test test_screenshot_parameter_validation -- --exact --nocapture

echo -e "${GREEN}Testing file validation...${NC}"
cargo test test_screenshot_file_validation -- --exact --nocapture

echo -e "${GREEN}Testing timing parameters...${NC}"
cargo test test_screenshot_timing_parameters -- --exact --nocapture

echo -e "${GREEN}Testing directory structure...${NC}"
cargo test test_screenshot_directory_structure -- --exact --nocapture

echo -e "${GREEN}Testing tool schema validation...${NC}"
cargo test test_screenshot_tool_schema_validation -- --exact --nocapture

echo -e "${GREEN}Testing timeout behavior...${NC}"
cargo test test_screenshot_wait_timeout -- --exact --nocapture

echo -e "${GREEN}Testing MCP schema integration...${NC}"
cargo test test_screenshot_tool_in_mcp_schema -- --exact --nocapture

echo -e "${BLUE}🎯 Checking compilation of examples and fixtures...${NC}"
cargo check --example screenshot_setup
cargo check tests/fixtures/static_test_game.rs
cargo check tests/fixtures/animated_test_game.rs

echo -e "${GREEN}✅ All screenshot tests completed successfully!${NC}"

echo -e "${BLUE}📁 Test artifacts created in:${NC}"
echo "  - test_output/ (screenshot outputs)"
echo "  - reference_screenshots/ (reference images)"  
echo "  - diffs/ (comparison diffs)"

echo -e "${BLUE}🚀 To test with a real Bevy game:${NC}"
echo "  1. Run: cargo run --example screenshot_setup"
echo "  2. In another terminal: cargo run --bin bevy-debugger-mcp"
echo "  3. Test screenshot functionality via Claude Code"

echo -e "${BLUE}📖 For more info, see:${NC}"
echo "  - book/SCREENSHOT_SETUP.md"
echo "  - examples/screenshot_setup.rs"
echo "  - tests/screenshot_e2e_tests.rs"