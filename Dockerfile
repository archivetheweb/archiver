FROM ubuntu:latest
RUN apt-get update && apt-get upgrade -y
RUN apt-get install wget curl build-essential pkg-config libssl-dev python3.6 -y
RUN wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
RUN apt install ./google-chrome-stable_current_amd64.deb -y
RUN rm ./google-chrome-stable_current_amd64.deb
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- -y 
RUN mkdir app
WORKDIR /app
COPY ./src ./src
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./config.yaml ./config.yaml
COPY ./res ./res
RUN /root/.cargo/bin/cargo build
ENTRYPOINT ["tail", "-f", "/dev/null"]