# Gateway

The Gateway Server holds a list of resources. It will be used to route a request to the correct Media Server.

The route logic is described in bellow:

- If request in closest zone, then route to that best media server in current zone.
- If request in different zone, then route to other gateway which closest to user zone.

For implement above approach, each media server will broadcast its information to all same zone gateway, this is done by using atm0s-sdn PubSub feature.
In addition, each gateway also broadcast its information to all other gateway. This also is done by using atm0s-sdn PubSub feature.