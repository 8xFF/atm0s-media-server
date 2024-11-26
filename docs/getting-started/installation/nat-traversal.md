# NAT Traversal

Some cloud providers (like AWS) route all traffic through a NAT gateway, which means that we don't have a public IP address on the VM.

To work around this, we can use the `node_ip_alt` and `node_ip_alt_cloud` options to specify alternative IP addresses for the node.

## Using node_ip_alt

The `node_ip_alt` option takes a list of IP addresses as input, and the node will bind to each of them.

## Using node_ip_alt_cloud

The `node_ip_alt_cloud` option takes a cloud provider as input, and will automatically fetch the alternative IP address for the node.

| Cloud Provider | Fetch URL                                                                                       |
| -------------- | ----------------------------------------------------------------------------------------------- |
| AWS            | `http://169.254.169.254/latest/meta-data/local-ipv4`                                            |
| GCP            | `http://metadata/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip` |
| Azure          | `http://ipv4.icanhazip.com`                                                                     |
| Other          | `http://ipv4.icanhazip.com`                                                                     |
