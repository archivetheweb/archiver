FROM rust:1.67 as builder

# Copies overthe source code
RUN mkdir app
WORKDIR /app
COPY ./archiver/src ./archiver/src
COPY ./archiver/Cargo.lock ./archiver/Cargo.lock
COPY ./archiver/Cargo.toml ./archiver/Cargo.toml
COPY ./archiver/config.yaml ./archiver/config.yaml
COPY ./archiver/.secret ./archiver/.secret
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

COPY ./warp-contracts-rust/shared ./warp-contracts-rust/shared 

COPY ./warp-contracts-rust/atw/Cargo.lock ./warp-contracts-rust/atw/Cargo.lock 
COPY ./warp-contracts-rust/atw/Cargo.toml ./warp-contracts-rust/atw/Cargo.toml 

COPY ./warp-contracts-rust/atw/contract/definition/src ./warp-contracts-rust/atw/contract/definition/src 
COPY ./warp-contracts-rust/atw/contract/definition/Cargo.toml ./warp-contracts-rust/atw/contract/definition/Cargo.toml 

COPY ./warp-contracts-rust/atw/contract/implementation/src ./warp-contracts-rust/atw/contract/implementation/src 
COPY ./warp-contracts-rust/atw/contract/implementation/Cargo.toml ./warp-contracts-rust/atw/contract/implementation/Cargo.toml 

COPY ./warp_dre ./warp_dre 

# builds the release
RUN cargo build --release


FROM ubuntu:22.04

# Gets the dependencies
RUN echo "deb http://security.ubuntu.com/ubuntu focal-security main" | tee /etc/apt/sources.list.d/focal-security.list
RUN apt-get update && apt-get install wget curl build-essential pkg-config libssl-dev python3.6 python3-pip libssl1.1 -y 
RUN dpkg -L libssl1.1
RUN pip3 install pywb
RUN wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb && \ 
    apt install ./google-chrome-stable_current_amd64.deb -y && \
    rm ./google-chrome-stable_current_amd64.deb

RUN rm -rf /var/lib/apt/lists/*

####################################################
# COPY  ./target/debug/archiver-v1 ./
# COPY  ./archiver/.secret/wallet.json ./.secret/
####################################################

# Copies over the release
COPY --from=builder /app/target/release/archiver-v1 ./
COPY --from=builder /app/archiver/.secret/wallet.json ./.secret/

# Sets env flags
ENV RUST_LOG=debug
ENV IN_DOCKER=true

ENTRYPOINT ["./archiver-v1"]
# ENTRYPOINT ["tail", "-f", "/dev/null"]

