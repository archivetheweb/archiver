FROM ubuntu:22.04
RUN apt-get update && apt-get upgrade -y
RUN apt-get install wget curl build-essential pkg-config libssl-dev python3.6 python3-pip -y 
RUN pip3 install pywb
RUN wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
RUN apt install ./google-chrome-stable_current_amd64.deb -y
RUN rm ./google-chrome-stable_current_amd64.deb
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- -y 


ENV USER=app
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"


RUN mkdir app
WORKDIR /app
COPY ./archivor/src ./archivor/src
COPY ./archivor/Cargo.lock ./archivor/Cargo.lock
COPY ./archivor/Cargo.toml ./archivor/Cargo.toml
COPY ./archivor/config.yaml ./archivor/config.yaml
COPY ./archivor/.secret ./archivor/.secret
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./warp-contracts-rust ./warp-contracts-rust 
COPY ./warp_dre ./warp_dre 

RUN /root/.cargo/bin/cargo build --release

USER app:app

# FROM scratch

# # Import from builder.
# COPY --from=builder /etc/passwd /etc/passwd
# COPY --from=builder /etc/group /etc/group

# WORKDIR /app

# # Copy our build
# COPY --from=builder /app/target/release/archivoor-v1 ./

# # Use an unprivileged user.
# USER app:app

# CMD ["/app/archivoor-v1"]



ENTRYPOINT ["tail", "-f", "/dev/null"]