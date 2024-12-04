# Use a Rust base image
FROM rust:1.78 as builder

# Create a new empty shell project
WORKDIR /usr/src/app

# Install cargo-watch for development hot-reloading
RUN cargo install cargo-watch

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# This build step will cache dependencies
RUN mkdir src && \
    echo "fn main() {println!(\"dummy\")}" > src/main.rs && \
    cargo build && \
    rm -rf src

# Copy the source code
COPY . .

# Use cargo watch to rebuild on changes
CMD ["cargo", "watch", "-x", "run"]
