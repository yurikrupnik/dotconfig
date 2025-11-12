#!/usr/bin/env bash
# Test installation in Docker environment
set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}=== Dotconfig Docker Test ===${NC}"
echo ""

# Check if Docker is available
if ! command -v docker &>/dev/null; then
    echo -e "${YELLOW}Docker is not installed. Please install Docker first.${NC}"
    exit 1
fi

# Build test image
echo -e "${GREEN}Building test Docker image...${NC}"
docker build -f Dockerfile.test -t dotconfig-test .

echo ""
echo -e "${GREEN}Docker image built successfully!${NC}"
echo ""
echo "Options:"
echo "  1. Run validation tests (safe, no installation)"
echo "  2. Run full installation in container"
echo "  3. Start interactive shell in container"
echo ""
read -p "Choose option (1-3): " choice

case $choice in
    1)
        echo -e "${BLUE}Running validation tests...${NC}"
        docker run -it --rm dotconfig-test ./test-install.sh
        ;;
    2)
        echo -e "${BLUE}Running full installation...${NC}"
        docker run -it --rm dotconfig-test /bin/bash -c "./install.sh"
        ;;
    3)
        echo -e "${BLUE}Starting interactive shell...${NC}"
        echo "Run './test-install.sh' to validate or './install.sh' to install"
        docker run -it --rm dotconfig-test /bin/bash
        ;;
    *)
        echo "Invalid option"
        exit 1
        ;;
esac

echo ""
echo -e "${GREEN}Test complete!${NC}"
