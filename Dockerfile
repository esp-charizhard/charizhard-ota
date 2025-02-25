# Stage 1: Use the CI to build the binary
FROM scratch

# Set the target architecture
ARG TARGET=x86_64-unknown-linux-musl

# Copy the binary from the CI build context
COPY ./target/$TARGET/release/charizhard-ota /ota

# Set the default command
CMD ["/ota"]
