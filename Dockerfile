FROM rust:1.94-slim-bullseye

# System dependencies (ldd is included via libc-bin)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    patchelf \
    curl \
    build-essential \
    libffi-dev \
    libgmp-dev \
    libncurses-dev \
    libtinfo-dev \
    zlib1g-dev \
    libpcre3 \
    libpcre3-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

# Install ghcup, GHC 9.2.8, and cabal
ENV GHCUP_INSTALL_BASE_PREFIX=/opt
ENV PATH="/opt/.ghcup/bin:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://get-ghcup.haskell.org | \
    BOOTSTRAP_HASKELL_NONINTERACTIVE=1 \
    BOOTSTRAP_HASKELL_GHC_VERSION=9.2.8 \
    BOOTSTRAP_HASKELL_INSTALL_NO_STACK=1 \
    sh

RUN ghcup set ghc 9.2.8

WORKDIR /app

COPY . .

ARG CODEARTIFACT_URL
ARG CODEARTIFACT_TOKEN

RUN mkdir -p .cargo && make clean && \
    printf '[registries.my_registry]\nindex = "sparse+%s"\ntoken = "%s"\n\n[registry]\ndefault = "my_registry"\n\n[source.crates-io]\nreplace-with = "my_registry"\n' \
      "${CODEARTIFACT_URL}" "${CODEARTIFACT_TOKEN}" > .cargo/config.toml && \
    cargo publish --registry my_registry
