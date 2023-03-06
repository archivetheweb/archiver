# Archiver

![](https://github.com/archivetheweb/.github/blob/main/profile/library.png?raw=true)

Archive the Web is an open-source website archiving tool that allows you to set up automated archiving stored on Arweave. Our mission at Archive the Web is to create a decentralized backup of the world wide web together.

Website can be found [here](https://archivetheweb.com).

## How it works

In its basic form, this application crawls a website up to a specific depth, saves all interactions with the website's servers and resources loaded in a WARC format and uploads it all to the Arweave network.

### Archive format

[WARC 1.1](http://iipc.github.io/warc-specifications/specifications/warc-format/warc-1.1/) is the format chosen for this application. It an international standard used by many archives and thus allows for composability.

We rely heavily on [Webrecorder](https://webrecorder.net/)'s [pywb](https://github.com/webrecorder/pywb) toolkit to capture all requests between our browser and the website's servers to output a WARC file.

### Arweave

#### The permaweb

Data added to Arweave is replicated amongst hundreds or thousands of computers or "miners" making it resilient and easily retrievable. To permanently save data, the Arweave network charges an upfront fee or an "endowment fee". The cost is estimated to incentivize these miners to continue to store the data for at least 200 years. The cost is calculated based on conservative estimates around price reductions for storage over time. For more information please check their [yellow paper](https://yellow-paper.arweave.dev/)

#### Warp Contract

A [Warp contract](https://warp.cc/) (smart contract on Arweave) is used to update the current state of the archive. Currently it is where an archiver can register, and anyone can create an "Archiving Request" that will be fulfilled by an archiver.

Warp contract address: `dD1DuvgM_Vigtnv4vl2H1IYn9CgLvYuhbEWPOL-_4Mw`

## How to run

First ensure you have an Arweave wallet with AR in it. Also, make sure you fund you Bundlr account with sufficient AR on the Bundlr node of your choice (default is [node1](https://node1.bundlr.network)).

Make sure that the file is stored at the path `./archiver/.secret/wallet.json`.

Third, make sure to register as an archiver. More info to come.

### Vanilla

1. Run `git submodule update`

2. Ensure you have [redis](https://redis.io/docs/getting-started/) running on port 6379

3. Install [Google Chrome](https://www.google.com/chrome/?brand=YTUH&gclid=Cj0KCQiA0oagBhDHARIsAI-Bbgelk0ka9FqCJSvfUTauwL89oRMgQhUg0cldJVJyPvBCvFJGEF_JhZMaAoH4EALw_wcB&gclsrc=aw.ds) (latest stable release)

4. Install [pywb](https://pywb.readthedocs.io/en/latest/manual/usage.html#getting-started) by running `pip3 install pywb`

5. Run `cd archiver && cargo run`. If you want to get the debug output, make sure to add `RUST_LOG=debug` to your environment variables

### Using Docker

1. Run `git submodule update`

2. Run `docker-compose up`
