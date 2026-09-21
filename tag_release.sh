#!/bin/bash

git switch main
git pull

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🚀 Copycraft Release Script${NC}"

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "//' | sed 's/"//')

echo -e "${YELLOW}Current version: ${GREEN}$CURRENT_VERSION${NC}"

# Parse version parts
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

# Bump patch version
NEW_PATCH=$((PATCH + 1))
NEW_VERSION="$MAJOR.$MINOR.$NEW_PATCH"

echo -e "${YELLOW}New version: ${GREEN}$NEW_VERSION${NC}"

# Confirm
read -p "Continue with version $NEW_VERSION? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${RED}Cancelled${NC}"
    exit 1
fi

# Update Cargo.toml
echo -e "${YELLOW}Updating Cargo.toml...${NC}"
sed -i '' "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml

# Also update version in bundle metadata if present
sed -i '' "s/version = \"$CURRENT_VERSION\" # bundle/version = \"$NEW_VERSION\" # bundle/" Cargo.toml

# Check if changes were made
if ! grep -q "version = \"$NEW_VERSION\"" Cargo.toml; then
    echo -e "${RED}Failed to update version${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Cargo.toml updated${NC}"

# Git commit
echo -e "${YELLOW}Creating git commit...${NC}"
git add Cargo.toml
git commit -m "Bump version to $NEW_VERSION"

if [ $? -ne 0 ]; then
    echo -e "${RED}Failed to commit${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Git commit created${NC}"

# Git tag
echo -e "${YELLOW}Creating git tag...${NC}"
git tag "v$NEW_VERSION"

if [ $? -ne 0 ]; then
    echo -e "${RED}Failed to create tag${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Git tag created${NC}"

# Git push
echo -e "${YELLOW}Pushing to GitHub...${NC}"
git push origin main
git push origin "v$NEW_VERSION"

if [ $? -ne 0 ]; then
    echo -e "${RED}Failed to push${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Pushed to GitHub${NC}"

echo -e "${GREEN}✅ Release v$NEW_VERSION created successfully!${NC}"
echo -e "${YELLOW}GitHub Actions will now build and release the DMG.${NC}"

git add Cargo.lock
git commit -m "Update cargo lock with version $NEW_VERSION"
git push
