# network-tunnel

A service for easily starting a tunnel to a destination host through a bastion server, and port-forwarding specific ports, to be used by [Flow](https://github.com/estuary/flow) [connectors](https://github.com/estuary/connectors).

# Usage

```
flow-network-tunnel ssh --ssh-endpoint <SSH_ENDPOINT> \
  --private-key <PRIVATE_KEY_FILE_PATH> \
  --forward-host <FORWARD_HOST> \
  --forward-port <FORWARD_PORT> \
  --local-port <LOCAL_PORT> \
  [--known-hosts <KNOWN_HOSTS_FILE_PATH>] \
  --log.level=debug
```

With `--known-hosts`, ssh only connects to a server whose host key is in that file. Without it, ssh doesn't check the server's host key.
