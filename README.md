# network-tunnel

A service for easily starting a tunnel to a destination host through a bastion server, and port-forwarding specific ports, to be used by [Flow](https://github.com/estuary/flow) [connectors](https://github.com/estuary/connectors).

# Usage

```
flow-network-tunnel ssh --ssh-endpoint <SSH_ENDPOINT> --private-key <PRIVATE_KEY_FILE_PATH> --forward-host <FORWARD_HOST> --forward-port <FORWARD_PORT> --local-port <LOCAL_PORT> --log.level=debug
```
