# Gateway Server

The Gateway Server holds a list of resources. There are two types of gateways: zone-level gateways, which hold all servers within their zone, and global gateways, which hold all zone gateways. Gateways act as routers, directing user requests to the best or destination node based on the request type and parameters.

### Inner Gateway

This gateway is used to route a request to the best node inside a zone.

### Global Gateway

This gateway is used to route a request to the closest zone to the user. To determine the user's location, we use the free maxmind-db geo-ip. You can download it by running the script `download-geodata.sh`.