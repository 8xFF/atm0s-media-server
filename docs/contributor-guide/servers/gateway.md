# Gateway

The Gateway Server holds a list of media servers and is used to route requests to the correct media server.

The routing logic is described below:

- If the request is in the closest zone, then route it to the best media server in the current zone.
- If the request is in a different zone, then route it to the other gateway that is closest to the user's zone.

To implement the above approach, each media server will broadcast its information to all gateways in the same zone. This is done using the atm0s-sdn PubSub feature.
Additionally, each gateway will also broadcast its information to all other gateways. This is also done using the atm0s-sdn PubSub feature.
