# Console API Reference

## Login

<details>
 <summary>
  <code>POST</code> 
  <code><b>/api/login</b></code> 
  <code>Login</code>
 </summary>

### Body

| name   | required           | type   | Description |
| ------ | ------------------ | ------ | ----------- |
| secret | :white_check_mark: | string | N/A         |

### Response

| status code | content-type     | response                                                          |
| ----------- | ---------------- | ----------------------------------------------------------------- |
| 200         | application/json | {access_token: string, refresh_token: string, expired_at: number} |
| 401         | application/json | {code: string, message: string}                                   |
| 404         | application/json | {code: string, message: string}                                   |

</details>

## Node

<details>
  <summary>
    <code>GET</code> 
    <code><b>/api/nodes</b></code> 
    <code>List Node</code>
 </summary>

### Header

| name         | required           | type       | Description  |
| ------------ | ------------------ | ---------- | ------------ |
| Authoriation | :white_check_mark: | Bear token | access token |

### Query Params

| name     | required             | type        | Description          |
| -------- | -------------------- | ----------- | -------------------- |
| zone_ids | :white_large_square: | Vec<int>    | Filter by zones      |
| types    | :white_large_square: | Vec<string> | Filter by node type  |
| fields   | :white_large_square: | Vec<string> | Select field to show |

### Response

#### Node connection object

| name        | required           | type | Description                    |
| ----------- | ------------------ | ---- | ------------------------------ |
| id          | :white_check_mark: | int  |                                |
| uptime      | :white_check_mark: | int  | connection up time             |
| throughput  | :white_check_mark: | int  | throughput between connection  |
| packet_loss | :white_check_mark: | int  | loss packet between connection |

#### Node Object

| name        | required             | type                                                | Description                                       |
| ----------- | -------------------- | --------------------------------------------------- | ------------------------------------------------- |
| zone_id     | :white_check_mark:   | int                                                 | N/A                                               |
| type        | :white_check_mark:   | string                                              | type of node (connector, console, gateway, media) |
| ip          | :white_check_mark:   | string                                              | IP of node                                        |
| id          | :white_check_mark:   | int                                                 | ID of node                                        |
| status      | :white_large_square: | enum                                                | status of node: ONLINE / OFFLINE                  |
| uptime      | :white_large_square: | int                                                 | N/A                                               |
| current_cpu | :white_large_square: | float                                               | N/A                                               |
| current_ram | :white_large_square: | float                                               | N/A                                               |
| connections | :white_large_square: | Array<[Connection Object](#node-connection-object)> | list connection of node                           |

#### Response Payload

| status code | content-type     | response                                    |
| ----------- | ---------------- | ------------------------------------------- |
| 200         | application/json | { data: Array<[Node Object](#node-object)>} |
| 401         | application/json | {code: string, message: string}             |

</details>

<details>
  <summary>
    <code>GET</code> 
    <code><b>/api/nodes/:id</b></code> 
    <code>Detail Node</code>
 </summary>

### Header

| name         | required           | type       | Description  |
| ------------ | ------------------ | ---------- | ------------ |
| Authoriation | :white_check_mark: | Bear token | access token |

### Response

| status code | content-type     | response                             |
| ----------- | ---------------- | ------------------------------------ |
| 200         | application/json | { data: [Node Object](#node-object)} |
| 401         | application/json | {code: string, message: string}      |

</details>

## Event

<details>
  <summary>
    <code>GET</code> 
    <code><b>/api/events</b></code> 
    <code>List Event</code>
 </summary>

### Header

| name         | required           | type       | Description  |
| ------------ | ------------------ | ---------- | ------------ |
| Authoriation | :white_check_mark: | Bear token | access token |

### Query Params

| name           | required             | type        | Description                                                                          |
| -------------- | -------------------- | ----------- | ------------------------------------------------------------------------------------ |
| zone_ids       | :white_large_square: | Vec<int>    | Filter by zones                                                                      |
| node_ids       | :white_large_square: | Vec<int>    | Filter by nodes                                                                      |
| types          | :white_large_square: | Vec<string> | Filter by event type                                                                 |
| event_time_lte | :white_large_square: | int         | The timestamp is the time that the event time is less than or equal (unit: milisec)  |
| event_time_gte | :white_large_square: | int         | The timestamp is the time that the event time is great than or equal (unit: milisec) |
| limit          | :white_large_square: | int         | the maximum element of each request (range 10 - 500)                                 |

### Event Object

| name       | required           | type   | Description                             |
| ---------- | ------------------ | ------ | --------------------------------------- |
| id         | :white_check_mark: | int    | ID of event                             |
| zone_id    | :white_check_mark: | int    | N/A                                     |
| type       | :white_check_mark: | string | type of event (node, room, session,...) |
| node_id    | :white_check_mark: | int    | N/A                                     |
| event_time | :white_check_mark: | int    | the timestamp of event                  |
| message    | :white_check_mark: | string | detail message of event                 |

### Response

| status code | content-type     | response                                      |
| ----------- | ---------------- | --------------------------------------------- |
| 200         | application/json | { data: Array<[Event Object](#event-object)>} |
| 401         | application/json | {code: string, message: string}               |

</details>

## Room

<details>
  <summary>
    <code>GET</code> 
    <code><b>/api/rooms</b></code> 
    <code>List Room</code>
 </summary>

### Header

| name         | required           | type       | Description  |
| ------------ | ------------------ | ---------- | ------------ |
| Authoriation | :white_check_mark: | Bear token | access token |

### Query Params

| name             | required             | type        | Description                                                                         |
| ---------------- | -------------------- | ----------- | ----------------------------------------------------------------------------------- |
| zone_ids         | :white_large_square: | Vec<int>    | Filter by zones                                                                     |
| node_ids         | :white_large_square: | Vec<int>    | Filter by nodes                                                                     |
| status           | :white_large_square: | Vec<int>    |                                                                                     |
| start_time_lte   | :white_large_square: | int         | The timestamp is the time that the event time is less than or equal (unit: milisec) |
| start_time_gte   | :white_large_square: | int         | The timestamp is the time that the event time is less than or equal (unit: milisec) |
| start_time_order | :white_large_square: | int         | order by start time of room. (1: asc, -1: desc)                                     |
| limit            | :white_large_square: | int         | the maximum element of each request (range 10 - 500)                                |
| offset           | :white_large_square: | int         | the start offset of first element from list                                         |
| fields           | :white_large_square: | Vec<string> | Select field to show                                                                |

### Room Object

| name           | required             | type                                              | Description                                       |
| -------------- | -------------------- | ------------------------------------------------- | ------------------------------------------------- |
| zone_id        | :white_check_mark:   | int                                               | N/A                                               |
| node_id        | :white_check_mark:   | int                                               | N/A                                               |
| type           | :white_check_mark:   | string                                            | type of node (connector, console, gateway, media) |
| id             | :white_check_mark:   | int                                               | ID of room                                        |
| status         | :white_check_mark:   | enum                                              | status of room: LIVING / ENDED                    |
| started_at     | :white_check_mark:   | int                                               | the start timestamp of room                       |
| ended_at       | :white_large_square: | int                                               | the end timestamp of room                         |
| duration       | :white_large_square: | int                                               | the duration of room. Will calculated after ended |
| online_session | :white_large_square: | int                                               | the number session is online in room              |
| record         | :white_large_square: | string                                            | the record of room                                |
| metadata       | :white_large_square: | json                                              | metadata of the room                              |
| sessions       | :white_large_square: | Array<[Session Object](#session-endpoint-object)> | list session of room                              |

### Response

| status code | content-type     | response                                     |
| ----------- | ---------------- | -------------------------------------------- |
| 200         | application/json | { data: Array<[Event Object](#room-object)>} |
| 401         | application/json | {code: string, message: string}              |

</details>

<details>
  <summary>
    <code>GET</code> 
    <code><b>/api/room/:room_id/sessions</b></code> 
    <code>List Session</code>
 </summary>

### Header

| name         | required           | type       | Description  |
| ------------ | ------------------ | ---------- | ------------ |
| Authoriation | :white_check_mark: | Bear token | access token |

### Query Params

| name    | required             | type        | Description          |
| ------- | -------------------- | ----------- | -------------------- |
| zone_id | :white_large_square: | int         | N/A                  |
| node_id | :white_large_square: | int         | N/A                  |
| fields  | :white_large_square: | Vec<string> | Select field to show |

### Session endpoint object

| name                | required             | type | Description                         |
| ------------------- | -------------------- | ---- | ----------------------------------- |
| id                  | :white_check_mark:   | int  | N/A                                 |
| endpoint_session_id | :white_check_mark:   | int  | Endpoint id of session when in room |
| zone_id             | :white_check_mark:   | int  | N/A                                 |
| node_id             | :white_check_mark:   | int  | N/A                                 |
| status              | :white_check_mark:   | int  | 1: Online, 0: Offline               |
| join_at             | :white_check_mark:   | int  | session join timestamp              |
| leave_at            | :white_large_square: | int  | session leave timestamp             |
| metadata            | :white_large_square: | json | extradata of session                |

### Response

| status code | content-type     | response                                                   |
| ----------- | ---------------- | ---------------------------------------------------------- |
| 200         | application/json | { data: Array<[Session Object](#session-endpoitn-object)>} |
| 401         | application/json | {code: string, message: string}                            |

</details>

## Session

<details>
  <summary>
    <code>GET</code> 
    <code><b>/api/sessions</b></code> 
    <code>List Room</code>
 </summary>

### Header

| name         | required           | type       | Description  |
| ------------ | ------------------ | ---------- | ------------ |
| Authoriation | :white_check_mark: | Bear token | access token |

### Query Params

| name    | required             | type        | Description          |
| ------- | -------------------- | ----------- | -------------------- |
| zone_id | :white_large_square: | int         | N/A                  |
| node_id | :white_large_square: | int         | N/A                  |
| fields  | :white_large_square: | Vec<string> | Select field to show |

### Session Object

| name           | required             | type                                                | Description                                       |
| -------------- | -------------------- | --------------------------------------------------- | ------------------------------------------------- |
| id             | :white_check_mark:   | int                                                 | N/A                                               |
| zone_id        | :white_check_mark:   | int                                                 | N/A                                               |
| node_id        | :white_check_mark:   | int                                                 | N/A                                               |
| status         | :white_check_mark:   | int                                                 | 1: Online, 0: Offline                             |
| online_at      | :white_check_mark:   | int                                                 | session online timestamp                          |
| latest_ping_ts | :white_large_square: | int                                                 | timestamp of latest ping                          |
| metadata       | :white_large_square: | json                                                | metadata of the session                           |
| rooms          | :white_large_square: | vec<{room_id: number, endpoint_session_id: number}> | list the room that the session is joining current |

### Response

| status code | content-type     | response                                          |
| ----------- | ---------------- | ------------------------------------------------- |
| 200         | application/json | { data: Array<[Session Object](#session-object)>} |
| 401         | application/json | {code: string, message: string}                   |

</details>
