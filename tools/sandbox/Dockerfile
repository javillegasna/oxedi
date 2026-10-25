# The image `make sandbox` builds and tests in. Every input is pinned: base image and uv by digest,
# rustup-init by version and SHA-256, the toolchain and both CPythons by full version. The build
# context is this folder only. The tag is oxedi-sandbox:<first 12 hex of sha256 of this file>.

# uv 0.13.5, one static binary.
FROM ghcr.io/astral-sh/uv:0.13.5@sha256:32a7bb39b496babbd997b0a39a4d66c126bdaf5661b302bf6f06abda8deca6ff AS uv

# Index digest of ubuntu:24.04 (amd64 and arm64).
FROM ubuntu:24.04@sha256:68ee22004a9109b44973313b02ff7b1de250f1de0f5ecb628d73f579aaff33ac
ARG TARGETARCH
ARG RUST_VERSION=1.99.0
ARG RUSTUP_VERSION=1.29.2
ARG RUSTUP_INIT_SHA256_AMD64=5be191e8c70a5007074254cb74d78dbfdd25cd5a61c27de7b8e5410122ec24f4
ARG RUSTUP_INIT_SHA256_ARM64=a9c168dbea81a84671c6b7a5c1795cb0eb699020110d107ddfca427bd797e697
ARG PYTHON_OLDEST=3.11.15
ARG PYTHON_NEWEST=3.13.13

# make: runs the repository's targets. gcc: the linker driver rustc calls, and the C compiler of
# build scripts. libc6-dev: C headers and libc for gcc. ca-certificates: TLS for the fetch phase.
RUN apt-get update \
 && apt-get install -y --no-install-recommends make gcc libc6-dev ca-certificates \
 && apt-get clean \
 && find /var/lib/apt/lists -type f -delete

COPY --from=uv /uv /usr/local/bin/uv

ENV RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    RUSTUP_TOOLCHAIN=$RUST_VERSION \
    PATH=/opt/cargo/bin:$PATH
# curl is installed only for this step and purged in it.
RUN set -eu; \
    case "$TARGETARCH" in \
      amd64) triple=x86_64-unknown-linux-gnu; sum=$RUSTUP_INIT_SHA256_AMD64 ;; \
      arm64) triple=aarch64-unknown-linux-gnu; sum=$RUSTUP_INIT_SHA256_ARM64 ;; \
      *) echo "unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    apt-get update; apt-get install -y --no-install-recommends curl; \
    curl --proto '=https' --tlsv1.2 -fsSL -o /tmp/rustup-init \
      "https://static.rust-lang.org/rustup/archive/$RUSTUP_VERSION/$triple/rustup-init"; \
    echo "$sum  /tmp/rustup-init" | sha256sum -c -; \
    chmod +x /tmp/rustup-init; \
    /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain "$RUST_VERSION" \
      -c clippy -c rustfmt; \
    rm /tmp/rustup-init; \
    apt-get purge -y --auto-remove curl; apt-get clean; \
    find /var/lib/apt/lists -type f -delete; \
    chmod -R a+rX /opt/rustup /opt/cargo

ENV UV_PYTHON_INSTALL_DIR=/opt/python
RUN set -eu; \
    uv python install "$PYTHON_OLDEST" "$PYTHON_NEWEST"; \
    ln -s "$(uv python find "$PYTHON_NEWEST")" /usr/local/bin/python3; \
    chmod -R a+rX /opt/python
ENV UV_PYTHON_DOWNLOADS=never

# The apt set is kept in /usr/share/oxedi-sandbox.manifest; `make sandbox-image` also copies it,
# with the Rust and CPython versions, into the image labels.
RUN dpkg-query -W > /usr/share/oxedi-sandbox.manifest
LABEL org.opencontainers.image.title="oxedi-sandbox" \
      oxedi.sandbox.rust="$RUST_VERSION" \
      oxedi.sandbox.python="$PYTHON_OLDEST $PYTHON_NEWEST"

WORKDIR /work
