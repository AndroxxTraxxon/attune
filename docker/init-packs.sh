#!/bin/sh
# Initialize builtin packs for Attune
# This script copies pack files to the shared volume and registers them in the database

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration from environment
DB_HOST="${DB_HOST:-postgres}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${DB_USER:-attune}"
DB_PASSWORD="${DB_PASSWORD:-attune}"
DB_NAME="${DB_NAME:-attune}"
DB_SCHEMA="${DB_SCHEMA:-public}"

# Pack directories
SOURCE_PACKS_DIR="${SOURCE_PACKS_DIR:-/source/packs}"
TARGET_PACKS_DIR="${TARGET_PACKS_DIR:-/opt/attune/packs}"

# Python loader script
LOADER_SCRIPT="${LOADER_SCRIPT:-/scripts/load_core_pack.py}"

echo ""
echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║    Attune Builtin Packs Initialization        ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}"
echo ""

# Install system dependencies
echo -e "${YELLOW}→${NC} Installing system dependencies..."
apk add --no-cache postgresql-client > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} System dependencies installed"
else
    echo -e "${RED}✗${NC} Failed to install system dependencies"
    exit 1
fi

# Install Python dependencies
echo -e "${YELLOW}→${NC} Installing Python dependencies..."
pip install --quiet --no-cache-dir psycopg2-binary pyyaml 2>/dev/null
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} Python dependencies installed"
else
    echo -e "${RED}✗${NC} Failed to install Python dependencies"
    exit 1
fi
echo ""

# Wait for database to be ready
echo -e "${YELLOW}→${NC} Waiting for database to be ready..."
export PGPASSWORD="$DB_PASSWORD"
until psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c '\q' 2>/dev/null; do
  echo -e "${YELLOW}  ...${NC} Database is unavailable - sleeping"
  sleep 2
done
echo -e "${GREEN}✓${NC} Database is ready"

# Create target packs directory if it doesn't exist
echo -e "${YELLOW}→${NC} Ensuring packs directory exists..."
mkdir -p "$TARGET_PACKS_DIR"
echo -e "${GREEN}✓${NC} Packs directory ready at: $TARGET_PACKS_DIR"

# Check if source packs directory exists
if [ ! -d "$SOURCE_PACKS_DIR" ]; then
    echo -e "${RED}✗${NC} Source packs directory not found: $SOURCE_PACKS_DIR"
    exit 1
fi

# Find all pack directories (directories with pack.yaml)
echo ""
echo -e "${BLUE}Discovering builtin packs...${NC}"
echo "----------------------------------------"

PACK_COUNT=0
COPIED_COUNT=0
LOADED_COUNT=0

for pack_dir in "$SOURCE_PACKS_DIR"/*; do
    if [ -d "$pack_dir" ]; then
        pack_name=$(basename "$pack_dir")
        pack_yaml="$pack_dir/pack.yaml"

        if [ -f "$pack_yaml" ]; then
            PACK_COUNT=$((PACK_COUNT + 1))
            echo -e "${BLUE}→${NC} Found pack: ${GREEN}$pack_name${NC}"

            # Check if pack already exists in target
            target_pack_dir="$TARGET_PACKS_DIR/$pack_name"

            if [ -d "$target_pack_dir" ]; then
                # Pack exists, update files to ensure we have latest (especially binaries)
                echo -e "${YELLOW}  ⟳${NC} Pack exists at: $target_pack_dir, updating files..."
                cp -rf "$pack_dir"/* "$target_pack_dir"/
                if [ $? -eq 0 ]; then
                    echo -e "${GREEN}  ✓${NC} Updated pack files at: $target_pack_dir"
                else
                    echo -e "${RED}  ✗${NC} Failed to update pack"
                    exit 1
                fi
            else
                # Copy pack to target directory
                echo -e "${YELLOW}  →${NC} Copying pack files..."
                cp -r "$pack_dir" "$target_pack_dir"

                if [ $? -eq 0 ]; then
                    COPIED_COUNT=$((COPIED_COUNT + 1))
                    echo -e "${GREEN}  ✓${NC} Copied to: $target_pack_dir"
                else
                    echo -e "${RED}  ✗${NC} Failed to copy pack"
                    exit 1
                fi
            fi
        fi
    fi
done

echo "----------------------------------------"
echo ""

if [ $PACK_COUNT -eq 0 ]; then
    echo -e "${YELLOW}⚠${NC} No builtin packs found in $SOURCE_PACKS_DIR"
    echo -e "${BLUE}ℹ${NC} This is OK if you're running with no packs"
    exit 0
fi

echo -e "${BLUE}Pack Discovery Summary:${NC}"
echo "  Total packs found: $PACK_COUNT"
echo "  Newly copied: $COPIED_COUNT"
echo "  Already present: $((PACK_COUNT - COPIED_COUNT))"
echo ""

# Load packs into database using Python loader
if [ -f "$LOADER_SCRIPT" ]; then
    echo -e "${BLUE}Loading packs into database...${NC}"
    echo "----------------------------------------"

    # Build database URL with schema support
    DATABASE_URL="postgresql://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME"

    # Set search_path for the Python script if not using default schema
    if [ "$DB_SCHEMA" != "public" ]; then
        export PGOPTIONS="-c search_path=$DB_SCHEMA,public"
    fi

    # Run the Python loader for each pack
    for pack_dir in "$TARGET_PACKS_DIR"/*; do
        if [ -d "$pack_dir" ]; then
            pack_name=$(basename "$pack_dir")
            pack_yaml="$pack_dir/pack.yaml"

            if [ -f "$pack_yaml" ]; then
                echo -e "${YELLOW}→${NC} Loading pack: ${GREEN}$pack_name${NC}"

                # Run Python loader
                if python3 "$LOADER_SCRIPT" \
                    --database-url "$DATABASE_URL" \
                    --pack-dir "$TARGET_PACKS_DIR" \
                    --schema "$DB_SCHEMA"; then
                    LOADED_COUNT=$((LOADED_COUNT + 1))
                    echo -e "${GREEN}✓${NC} Loaded pack: $pack_name"
                else
                    echo -e "${RED}✗${NC} Failed to load pack: $pack_name"
                    echo -e "${YELLOW}⚠${NC} Continuing with other packs..."
                fi
            fi
        fi
    done

    echo "----------------------------------------"
    echo ""
    echo -e "${BLUE}Database Loading Summary:${NC}"
    echo "  Successfully loaded: $LOADED_COUNT"
    echo "  Failed: $((PACK_COUNT - LOADED_COUNT))"
    echo ""
else
    echo -e "${YELLOW}⚠${NC} Pack loader script not found: $LOADER_SCRIPT"
    echo -e "${BLUE}ℹ${NC} Packs copied but not registered in database"
    echo -e "${BLUE}ℹ${NC} You can manually load them later"
fi

# Summary
echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  Builtin Packs Initialization Complete!       ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${BLUE}Packs Location:${NC} ${GREEN}$TARGET_PACKS_DIR${NC}"
echo -e "${BLUE}Packs Available:${NC}"

for pack_dir in "$TARGET_PACKS_DIR"/*; do
    if [ -d "$pack_dir" ]; then
        pack_name=$(basename "$pack_dir")
        pack_yaml="$pack_dir/pack.yaml"
        if [ -f "$pack_yaml" ]; then
            # Try to extract version from pack.yaml
            version=$(grep "^version:" "$pack_yaml" | head -1 | sed 's/version:[[:space:]]*//' | tr -d '"')
            echo -e "  • ${GREEN}$pack_name${NC} ${BLUE}($version)${NC}"
        fi
    fi
done

echo ""
echo -e "${BLUE}ℹ${NC} Pack files are accessible to all services via shared volume"
echo ""

exit 0
