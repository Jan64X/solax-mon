
# Solax X3 Hybrid Gen4 Monitoring Program

A monitoring program written in Rust for the Solax X3 Hybrid Gen4 inverter. Also features an ssh client to automatically turn off your server(s) if the power conditions aren't met!

## TODO

- Rewrite the code to fix warnings
- Do more testing

## Building Containers

### AMD64

```bash
cross build --release --target x86_64-unknown-linux-musl
docker build --build-arg TARGETARCH=amd64 -t solax-mon:amd64 .
```

### ARM64

```bash
cross build --release --target aarch64-unknown-linux-musl
docker build --build-arg TARGETARCH=arm64 -t solax-mon:arm64 .
```

## Running the Container

Use one of the following commands depending on your architecture:

```bash
docker run -v /srv/solax-mon/data:/srv/solax-mon/data -p 3000:3000 solax-mon:amd64
# or
docker run -v /srv/solax-mon/data:/srv/solax-mon/data -p 3000:3000 solax-mon:arm64
```

## Configuration

User data should be stored in `/srv/solax-mon/data`

### Required Configuration

Add an ssh key to be used to ssh into the servers (/srv/solax-mon/data)
Create a `secrets.txt` file with the following attributes (in /srv/solax-mon/data):

```plaintext
INVERTER_IP=10.0.0.69
SERIAL=someserial
DISCORD_WEBHOOK=https://discord.com/api/webhooks/...
SERVER=user@10.0.0.70
HAVE_IDRAC=true/false
IDRAC_SERVER=10.0.0.6,root,password
SSH=true/false
```
