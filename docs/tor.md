# Tor

> **Warning:** Tor support in LDK Server applies **only when connecting outbound to
> `.onion` Lightning peers**. Connections to clearnet peers are not routed through
> Tor exit nodes. All other connections, including Electrum servers, Esplora endpoints, and
> Rapid Gossip Sync (RGS) servers, are also **not** routed through Tor and will use your
> normal network connection.
> If you require full network privacy, you should use a local bitcoind node as your chain
> source. Support for routing these connections through Tor may be added in the future.

LDK Server supports connecting to peers over Tor. This guide covers both outbound connections
(connecting to `.onion` peers) and inbound connections (making your node reachable as a hidden
service).

## Installing Tor

Follow tor's official installation instructions for your platform: https://support.torproject.org/little-t-tor/getting-started/installing/

The Tor daemon listens on `127.0.0.1:9050` by default.

## Outbound Connections

The `[tor]` section in the config sets a SOCKS proxy for outbound connections to OnionV3 peers:

```toml
[tor]
proxy_address = "127.0.0.1:9050"
```

This requires a running Tor daemon with a SOCKS port. Only connections to `.onion` peers use
the proxy. Connections to IPv4/IPv6 peers, Electrum servers, and Esplora endpoints are **not**
routed through Tor.

## Inbound Connections

To make your node reachable as a Tor hidden service, you need to configure Tor itself. LDK
Server does not manage this automatically.

### 1. Configure the Hidden Service

Edit your `torrc` file (typically `/etc/tor/torrc`):

```
HiddenServiceDir /var/lib/tor/ldk-server/
HiddenServicePort 9735 127.0.0.1:9735
```

This tells Tor to forward incoming connections on port 9735 of the hidden service to your
node's local Lightning listening port. Adjust the local port to match your
`node.listening_addresses` config.

### 2. Restart Tor

```bash
sudo systemctl restart tor
```

### 3. Get Your Onion Address

After restarting, Tor generates your `.onion` address:

```bash
sudo cat /var/lib/tor/ldk-server/hostname
```

This outputs something like `abcdef...xyz.onion`.

### 4. Configure LDK Server

Set the onion address as an announcement address so other nodes can find you:

```toml
[node]
listening_addresses = ["localhost:9735"]
announcement_addresses = ["abcdef...xyz.onion:9735"]

[tor]
proxy_address = "127.0.0.1:9050"
```

- `listening_addresses`: the local address your node actually listens on
- `announcement_addresses`: the public address announced to the network (your `.onion` address)
- `proxy_address`: needed so your node can also connect outbound to other `.onion` peers

### 5. Verify

After starting LDK Server, confirm your onion address appears in the node info:

```bash
ldk-server-cli get-node-info
```

The `node_uris` field should include `<node_id>@abcdef...xyz.onion:9735`. Other nodes can
now connect to you over Tor using this URI.

## Dual-Stack (Clearnet + Tor)

You can announce both a clearnet address and an onion address:

```toml
[node]
listening_addresses = ["0.0.0.0:9735"]
announcement_addresses = ["203.0.113.1:9735", "abcdef...xyz.onion:9735"]
```

This makes your node reachable over both the public internet and Tor.
