#!/usr/bin/env bash
set -euo pipefail

# Function to extract version from Cargo.toml
get_version() {
    grep '^version = ' Cargo.toml | cut -d '"' -f2
}

# Function to check if gh cli is logged in
check_gh_auth() {
    if ! gh auth status &>/dev/null; then
        echo "Error: Not logged in to GitHub CLI. Please run 'gh auth login' first."
        exit 1
    fi
}

# Function to check if docker is running
check_docker() {
    if ! docker info &>/dev/null; then
        echo "Error: Docker is not running or not accessible."
        exit 1
    fi
}

# Main script
main() {
    # Check prerequisites
    check_gh_auth
    check_docker

    # Get version from Cargo.toml
    VERSION=$(get_version)
    echo "📦 Building version ${VERSION}"

    # Extract GitHub repository information
    GITHUB_REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
    if [ -z "$GITHUB_REPO" ]; then
        echo "Error: Could not determine GitHub repository."
        exit 1
    fi

    # Build with Nix
    echo "🔨 Building with Nix..."
    nix build .#docker

    # Load the image into Docker
    echo "🐳 Loading image into Docker..."
    docker load < result

    # Tag images
    echo "🏷️  Tagging images..."
    docker tag nats-web-ui:latest "ghcr.io/${GITHUB_REPO}:${VERSION}"
    docker tag nats-web-ui:latest "ghcr.io/${GITHUB_REPO}:latest"

    # Push images
    echo "⬆️  Pushing images to GitHub Container Registry..."
    docker push "ghcr.io/${GITHUB_REPO}:${VERSION}"
    docker push "ghcr.io/${GITHUB_REPO}:latest"

    echo "✨ Done! Images pushed:"
    echo "  - ghcr.io/${GITHUB_REPO}:${VERSION}"
    echo "  - ghcr.io/${GITHUB_REPO}:latest"
}

main "$@"
