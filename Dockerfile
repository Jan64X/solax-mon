# Runtime stage
FROM alpine:3.21 AS base

# Allow the architecture to be specified at runtime (default to amd64)
ARG TARGETARCH=amd64

# Install necessary runtime dependencies
RUN apk add --no-cache \
    sudo \
    curl \
    openssh \
    sshpass \
    perl

# Set working directory
WORKDIR /srv/solax-mon

# Create a stage for AMD64
FROM base AS amd64
COPY ./target/x86_64-unknown-linux-musl/release/solax-mon /srv/solax-mon/
COPY ./target/x86_64-unknown-linux-musl/release/ssh /srv/solax-mon/

# Create a stage for ARM64
FROM base AS arm64
COPY ./target/aarch64-unknown-linux-musl/release/solax-mon /srv/solax-mon/
COPY ./target/aarch64-unknown-linux-musl/release/ssh /srv/solax-mon/

# Final stage
FROM ${TARGETARCH}

# Copy the init script and make it executable
COPY init.sh .
RUN chmod +x init.sh

# Set the entrypoint
ENTRYPOINT ["/srv/solax-mon/init.sh"]