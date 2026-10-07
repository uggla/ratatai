#!/bin/bash

# Script to build and run the ratatai Podman container
set -euo pipefail

# Configuration
IMAGE_NAME="ratatai"
CONTAINER_NAME="ratatai-container"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check that direnv has exported the variables needed by the application.
check_ai_environment() {
    local name
    for name in AI_PROVIDER GEMINI_API_KEY GEMINI_MODEL; do
        if [[ -z "${!name:-}" ]]; then
            print_error "$name is not set. Load .envrc with direnv before running the container."
            exit 1
        fi
    done
}

# Function to build Podman image
build_image() {
    print_status "Building Podman image: $IMAGE_NAME"
    
    podman build -t "$IMAGE_NAME" -f Containerfile .
    print_status "Podman image built successfully"
}

# Function to run container
run_container() {
    print_status "Starting container: $CONTAINER_NAME"
    
    # Create local logs directory if it doesn't exist
    mkdir -p ./logs
    print_status "Mounting current directory as container working directory"

    # Remove existing container if it exists
    if podman ps -a --format 'table {{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        print_warning "Removing existing container: $CONTAINER_NAME"
        podman rm -f "$CONTAINER_NAME"
    fi

    # Run the container with current directory mounted as working directory
    # Use --userns=keep-id to automatically map current user to container user
    podman run -it \
        --name "$CONTAINER_NAME" \
        --env AI_PROVIDER \
        --env GEMINI_API_KEY \
        --env GEMINI_MODEL \
        --env RUST_LOG="${RUST_LOG:-info}" \
        --volume "$(pwd):/app:Z" \
        --userns=keep-id \
        --rm \
        "$IMAGE_NAME"
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [build|run|build-and-run|help]"
    echo ""
    echo "Commands:"
    echo "  build         Build the Docker image only"
    echo "  run           Run the container (assumes image exists)"
    echo "  build-and-run Build image and run container (default)"
    echo "  help          Show this help message"
}

# Main script logic
main() {
    local command="${1:-build-and-run}"

    case "$command" in
    "build")
        build_image
        ;;
    "run")
        check_ai_environment
        run_container
        ;;
    "build-and-run")
        check_ai_environment
        build_image
        run_container
        ;;
    "help" | "-h" | "--help")
        show_usage
        ;;
    *)
        print_error "Unknown command: $command"
        show_usage
        exit 1
        ;;
    esac
}

# Check if Podman is available
if ! command -v podman &>/dev/null; then
    print_error "Podman is not installed or not in PATH"
    exit 1
fi

# Check if Podman is working
if ! podman info &>/dev/null; then
    print_error "Podman is not working properly"
    exit 1
fi

# Run main function with all arguments
main "$@"
