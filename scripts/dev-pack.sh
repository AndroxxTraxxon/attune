#!/bin/bash
set -e

# Helper script for managing development packs

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PACKS_DEV_DIR="$PROJECT_ROOT/packs.dev"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

function print_usage() {
    cat << USAGE
Development Pack Management Script

Usage: $0 <command> [arguments]

Commands:
  create <pack-ref>              Create a new pack structure
  list                           List all dev packs
  validate <pack-ref>            Validate a pack structure
  register <pack-ref>            Register pack in Docker environment
  clean                          Remove all non-example packs
  help                           Show this help message

Examples:
  # Create a new pack
  $0 create my-awesome-pack

  # List all packs
  $0 list

  # Register pack in database
  $0 register my-awesome-pack

Environment Variables:
  ATTUNE_API_URL    API URL (default: http://localhost:8080)
  ATTUNE_TOKEN      Authentication token (required for register)

USAGE
}

function create_pack() {
    local pack_ref="$1"

    if [ -z "$pack_ref" ]; then
        echo -e "${RED}Error: Pack reference is required${NC}"
        echo "Usage: $0 create <pack-ref>"
        exit 1
    fi

    local pack_dir="$PACKS_DEV_DIR/$pack_ref"

    if [ -d "$pack_dir" ]; then
        echo -e "${RED}Error: Pack '$pack_ref' already exists${NC}"
        exit 1
    fi

    echo -e "${BLUE}Creating pack structure for '$pack_ref'...${NC}"

    # Create directories
    mkdir -p "$pack_dir/actions"
    mkdir -p "$pack_dir/triggers"
    mkdir -p "$pack_dir/sensors"
    mkdir -p "$pack_dir/workflows"

    # Create pack.yaml
    cat > "$pack_dir/pack.yaml" << YAML
ref: $pack_ref
label: "$(echo $pack_ref | sed 's/-/ /g' | awk '{for(i=1;i<=NF;i++) $i=toupper(substr($i,1,1)) tolower(substr($i,2));}1')"
description: "Custom pack: $pack_ref"
version: "1.0.0"
author: "Developer"
email: "dev@example.com"

system: false
enabled: true

tags:
  - custom
  - development
YAML

    # Create example action
    cat > "$pack_dir/actions/example.yaml" << YAML
name: example
ref: ${pack_ref}.example
description: "Example action"
runner_type: shell
enabled: true
entry_point: example.sh

parameters:
  type: object
  properties:
    message:
      type: string
      description: "Message to process"
      default: "Hello from $pack_ref"
  required: []

output:
  type: object
  properties:
    result:
      type: string
      description: "Processing result"

tags:
  - example
YAML

    cat > "$pack_dir/actions/example.sh" << 'BASH'
#!/bin/bash
set -e

MESSAGE="${ATTUNE_ACTION_message:-Hello}"
echo "{\"result\": \"Processed: $MESSAGE\"}"
BASH

    chmod +x "$pack_dir/actions/example.sh"

    # Create README
    cat > "$pack_dir/README.md" << README
# $pack_ref

Custom development pack.

## Actions

- \`${pack_ref}.example\` - Example action

## Usage

\`\`\`bash
# Register the pack
./scripts/dev-pack.sh register $pack_ref

# Validate the pack
./scripts/dev-pack.sh validate $pack_ref
\`\`\`

## Development

Edit files in \`packs.dev/$pack_ref/\` and they will be immediately available in Docker containers.

README

    echo -e "${GREEN}✓ Pack created successfully${NC}"
    echo -e "${BLUE}Location: $pack_dir${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Edit $pack_dir/pack.yaml"
    echo "  2. Add actions in $pack_dir/actions/"
    echo "  3. Register pack: $0 register $pack_ref"
}

function list_packs() {
    echo -e "${BLUE}Development Packs:${NC}"
    echo ""

    local count=0
    for pack_dir in "$PACKS_DEV_DIR"/*; do
        if [ -d "$pack_dir" ] && [ -f "$pack_dir/pack.yaml" ]; then
            local pack_ref=$(basename "$pack_dir")
            local label=$(grep "^label:" "$pack_dir/pack.yaml" | cut -d'"' -f2)
            local version=$(grep "^version:" "$pack_dir/pack.yaml" | cut -d'"' -f2)

            echo -e "  ${GREEN}$pack_ref${NC}"
            echo -e "    Label: $label"
            echo -e "    Version: $version"
            echo ""

            ((count++))
        fi
    done

    if [ $count -eq 0 ]; then
        echo -e "  ${YELLOW}No packs found${NC}"
        echo ""
        echo "Create a pack with: $0 create <pack-ref>"
    else
        echo -e "Total: $count pack(s)"
    fi
}

function validate_pack() {
    local pack_ref="$1"

    if [ -z "$pack_ref" ]; then
        echo -e "${RED}Error: Pack reference is required${NC}"
        exit 1
    fi

    local pack_dir="$PACKS_DEV_DIR/$pack_ref"

    if [ ! -d "$pack_dir" ]; then
        echo -e "${RED}Error: Pack '$pack_ref' not found${NC}"
        exit 1
    fi

    echo -e "${BLUE}Validating pack '$pack_ref'...${NC}"

    # Check pack.yaml
    if [ ! -f "$pack_dir/pack.yaml" ]; then
        echo -e "${RED}✗ pack.yaml not found${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ pack.yaml exists${NC}"

    # Check for actions
    local action_count=$(find "$pack_dir/actions" -name "*.yaml" 2>/dev/null | wc -l)
    echo -e "${GREEN}✓ Found $action_count action(s)${NC}"

    # Check action scripts
    for action_yaml in "$pack_dir/actions"/*.yaml; do
        if [ -f "$action_yaml" ]; then
            local entry_point=$(grep "entry_point:" "$action_yaml" | awk '{print $2}')
            local script_path="$pack_dir/actions/$entry_point"

            if [ ! -f "$script_path" ]; then
                echo -e "${RED}✗ Script not found: $entry_point${NC}"
            elif [ ! -x "$script_path" ]; then
                echo -e "${YELLOW}⚠ Script not executable: $entry_point${NC}"
            else
                echo -e "${GREEN}✓ Script OK: $entry_point${NC}"
            fi
        fi
    done

    echo -e "${GREEN}Validation complete${NC}"
}

function register_pack() {
    local pack_ref="$1"

    if [ -z "$pack_ref" ]; then
        echo -e "${RED}Error: Pack reference is required${NC}"
        exit 1
    fi

    local pack_dir="$PACKS_DEV_DIR/$pack_ref"

    if [ ! -d "$pack_dir" ]; then
        echo -e "${RED}Error: Pack '$pack_ref' not found${NC}"
        exit 1
    fi

    echo -e "${BLUE}Registering pack '$pack_ref' in Docker environment...${NC}"

    # Extract pack metadata
    local label=$(grep "^label:" "$pack_dir/pack.yaml" | cut -d'"' -f2)
    local version=$(grep "^version:" "$pack_dir/pack.yaml" | cut -d'"' -f2)
    local description=$(grep "^description:" "$pack_dir/pack.yaml" | cut -d'"' -f2)

    echo -e "${YELLOW}Note: Manual registration required via API${NC}"
    echo ""
    echo "Run the following command to register the pack:"
    echo ""
    echo "curl -X POST http://localhost:8080/api/v1/packs \\"
    echo "  -H \"Authorization: Bearer \$ATTUNE_TOKEN\" \\"
    echo "  -H \"Content-Type: application/json\" \\"
    echo "  -d '{"
    echo "    \"ref\": \"$pack_ref\","
    echo "    \"label\": \"${label:-Custom Pack}\","
    echo "    \"description\": \"${description:-Development pack}\","
    echo "    \"version\": \"${version:-1.0.0}\","
    echo "    \"system\": false,"
    echo "    \"enabled\": true"
    echo "  }'"
    echo ""
    echo "The pack files are available at: /opt/attune/packs.dev/$pack_ref"
}

function clean_packs() {
    echo -e "${YELLOW}This will remove all non-example packs from packs.dev/${NC}"
    echo -e "${RED}This action cannot be undone!${NC}"
    read -p "Are you sure? (yes/no): " confirm

    if [ "$confirm" != "yes" ]; then
        echo "Cancelled"
        exit 0
    fi

    local count=0
    for pack_dir in "$PACKS_DEV_DIR"/*; do
        if [ -d "$pack_dir" ]; then
            local pack_name=$(basename "$pack_dir")
            if [ "$pack_name" != "examples" ] && [ "$pack_name" != "README.md" ]; then
                echo "Removing: $pack_name"
                rm -rf "$pack_dir"
                ((count++))
            fi
        fi
    done

    echo -e "${GREEN}Removed $count pack(s)${NC}"
}

# Main command dispatch
case "${1:-}" in
    create)
        create_pack "$2"
        ;;
    list)
        list_packs
        ;;
    validate)
        validate_pack "$2"
        ;;
    register)
        register_pack "$2"
        ;;
    clean)
        clean_packs
        ;;
    help|--help|-h)
        print_usage
        ;;
    *)
        print_usage
        exit 1
        ;;
esac
