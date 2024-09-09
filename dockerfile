FROM rust:buster as builder
RUN apt-get update --fix-missing
RUN apt-get install -y git && apt-get install -y curl && apt-get install -y build-essential && apt-get install -y clang && apt-get install -y jq && apt-get install -y cmake
RUN git clone -b main https://github.com/Abstracted-Labs/InvArch
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
    export PATH="$PATH:$HOME/.cargo/bin" && \
    cd InvArch && \
    rustup toolchain install $(cat rust-toolchain.toml | grep -o -P '(?<=").*(?=")') && \
    rustup default stable && \
    rustup target add wasm32-unknown-unknown --toolchain $(cat rust-toolchain.toml | grep -o -P '(?<=").*(?=")') && \
    cargo build --release

# ↑ Build Stage | Final Stage ↓
FROM docker.io/library/ubuntu:latest
COPY --from=builder /InvArch/target/release/invarch-collator /usr/local/bin
COPY --from=builder /etc/ssl/certs/ /etc/ssl/certs/
COPY --from=builder /InvArch/node/res/tinker-spec-raw.json /data/tinker-spec-raw.json
COPY --from=builder /InvArch/node/res/rococo.json /data/rococo.json


RUN useradd -m -u 1000 -U -s /bin/sh -d /invarch-collator invarch-collator && \
    mkdir -p /invarch-collator/.local/share && \
    chown -R invarch-collator:invarch-collator /data && \
    ln -s /data /invarch-collator/.local/share/invarch-collator && \
    rm -rf /usr/bin /usr/sbin

USER invarch-collator
EXPOSE 30333 9933 9944
VOLUME ["/data"]

EXPOSE 30333 9933 9944

ENTRYPOINT ["/usr/local/bin/invarch-collator"]
