FROM rust:1.67 as builder

RUN mkdir app
WORKDIR /app
COPY ./archivor/src ./archivor/src
COPY ./archivor/Cargo.lock ./archivor/Cargo.lock
COPY ./archivor/Cargo.toml ./archivor/Cargo.toml
COPY ./archivor/config.yaml ./archivor/config.yaml
COPY ./archivor/.secret ./archivor/.secret
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

COPY ./warp-contracts-rust/shared ./warp-contracts-rust/shared 

COPY ./warp-contracts-rust/awt/Cargo.lock ./warp-contracts-rust/awt/Cargo.lock 
COPY ./warp-contracts-rust/awt/Cargo.toml ./warp-contracts-rust/awt/Cargo.toml 

COPY ./warp-contracts-rust/awt/contract/definition/src ./warp-contracts-rust/awt/contract/definition/src 
COPY ./warp-contracts-rust/awt/contract/definition/Cargo.toml ./warp-contracts-rust/awt/contract/definition/Cargo.toml 

COPY ./warp-contracts-rust/awt/contract/implementation/src ./warp-contracts-rust/awt/contract/implementation/src 
COPY ./warp-contracts-rust/awt/contract/implementation/Cargo.toml ./warp-contracts-rust/awt/contract/implementation/Cargo.toml 

COPY ./warp_dre ./warp_dre 

RUN cargo build --release


FROM ubuntu:22.04

RUN echo "deb http://security.ubuntu.com/ubuntu focal-security main" | tee /etc/apt/sources.list.d/focal-security.list
RUN apt-get update && apt-get install wget curl build-essential pkg-config libssl-dev python3.6 python3-pip libssl1.1 -y 
RUN dpkg -L libssl1.1
RUN pip3 install pywb
RUN wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb && \ 
    apt install ./google-chrome-stable_current_amd64.deb -y && \
    rm ./google-chrome-stable_current_amd64.deb

RUN rm -rf /var/lib/apt/lists/*

# COPY  ./target/debug/archiver-v1 ./
# COPY  ./archivor/.secret/test_wallet.json ./.secret/
COPY --from=builder /app/target/release/archiver-v1 ./
COPY --from=builder /app/archivor/.secret/test_wallet.json ./.secret/

ENV RUST_LOG=debug

ENV IN_DOCKER=true

ENTRYPOINT ["./archiver-v1"]
# ENTRYPOINT ["tail", "-f", "/dev/null"]

