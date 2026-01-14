#!/bin/bash
# Script for running tabs tests in the agent_ui crate

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print usage
usage() {
    echo "Usage: $0 [OPTIONS] [TEST_NAME]"
    echo ""
    echo "Options:"
    echo "  -a, --all         Run all tab tests"
    echo "  -l, --list        List all tab tests"
    echo "  -o, --output      Show test output"
    echo "  -f, --failed      Run only failed tests"
    echo "  -h, --help        Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 -a                    # Run all tab tests"
    echo "  $0 test_create_single_tab # Run specific test"
    echo "  $0 -a -o                 # Run all tests with output"
    echo "  $0 -l                    # List all tests"
    exit 0
}

# Default values
SHOW_OUTPUT=""
TEST_PATTERN="tabs::tests"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -a|--all)
            TEST_PATTERN="tabs::tests"
            shift
            ;;
        -l|--list)
            LIST_TESTS=true
            shift
            ;;
        -o|--output)
            SHOW_OUTPUT="--nocapture"
            shift
            ;;
        -f|--failed)
            FAILED_ONLY="-- --failed"
            shift
            ;;
        -h|--help)
            usage
            ;;
        test_*)
            TEST_PATTERN="tabs::tests::$1"
            shift
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            ;;
    esac
done

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$PROJECT_DIR"

# List tests if requested
if [ "$LIST_TESTS" = true ]; then
    echo -e "${BLUE}Available tab tests:${NC}"
    echo ""
    cargo test -p agent_ui --lib -- --list 2>/dev/null | grep "tabs::tests::" | sed 's/^    /  /' || echo "  No tests found"
    exit 0
fi

# Print header
echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}  Running AgentPanel Tabs Tests${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""
echo -e "Test Pattern: ${YELLOW}$TEST_PATTERN${NC}"
echo ""

# Build the command
TEST_CMD="cargo test -p agent_ui --lib $TEST_PATTERN $SHOW_OUTPUT $FAILED_ONLY"

# Run tests
echo -e "${GREEN}Executing:${NC} $TEST_CMD"
echo ""

if eval "$TEST_CMD"; then
    echo ""
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    EXIT_CODE=$?
    echo ""
    echo -e "${RED}✗ Tests failed with exit code $EXIT_CODE${NC}"
    exit $EXIT_CODE
fi
