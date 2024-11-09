# Recording (Incomplete)

For maximum performance and flexibility, we don't perform any media transcoding during recording. Instead, we record raw media data and store it in a file. This approach provides great flexibility; for example, we can record all rooms but only compose selected rooms into a single file afterwards. This method also allows us to replay recordings without any transcoding.

For simple integration, we use S3 as the storage backend. Each media server collects raw streams and puts them in the S3 upload queue. The S3 destination is configured in the media server config file or fetched dynamically from the connector service.

## Record flow

To start recording, we need to create a join token with the record field set to true. The client will then join the room with this token. After joining, the server starts recording the media stream and creates chunks in memory. If there are too many chunks, they will be written to disk. When a chunk exceeds the size limit or time threshold, the media-server submits it to the upload queue.

## Upload flow

Media-server request connector node for getting s3 presigned url then upload the chunk to s3.

## Compose flow

We fired some hook to registered endpoint when a recording started, then you can save information to your database.

When a recording starts, a hook event is fired to your registered endpoint. Here's an example of the event structure:

```json
{
  "node": 22,
  "ts": 1731132577151,
  "event": {
    "Record": {
      "app": "app_id",
      "room": "room_uuid",
      "event": {
        "Started": {
          "path": "app_id/room_uuid/number"
        }
      }
    }
  }
}
```

You can convert record when received room destroyed event.

We provide 2 style of record convert, compose it to single file or transmux to multiple media files. Depend on your need, you can choose one of them.
Inside repo, we have a compose worker in media-record package. We provide two ways to compose:

- Compose by CLI
- Compose by API

### Compose by CLI

```bash
Record file converter for atm0s-media-server. This tool allow convert room raw record to multiple webm files

Usage: convert_record_cli [OPTIONS] --in-s3 <IN_S3>

Options:
      --in-s3 <IN_S3>                          S3 Source [env: IN_S3=]
      --transmux                               Transmux [env: TRANSMUX=]
      --transmux-out-s3 <TRANSMUX_OUT_S3>      Transmux S3 Dest [env: TRANSMUX_OUT_S3=]
      --transmux-out-path <TRANSMUX_OUT_PATH>  Transmux Folder Dest [env: TRANSMUX_OUT_PATH=]
      --compose-audio                          Compose audio [env: COMPOSE_AUDIO=]
      --compose-video                          Compose video [env: COMPOSE_VIDEO=]
      --compose-out-s3 <COMPOSE_OUT_S3>        Compose S3 URL [env: COMPOSE_OUT_S3=]
      --compose-out-path <COMPOSE_OUT_PATH>    Compose File Path [env: COMPOSE_OUT_PATH=]
  -h, --help                                   Print help
  -V, --version                                Print version
```

### Compose by API

The conversion worker provides a REST API endpoint to submit conversion jobs. You can convert recordings either by transmuxing to separate files or composing them into a single file.

To submit a conversion job, send a POST request to `/api/convert/job` with your Bearer token for authentication. Here's the request format:

```json
{
  "record_path": "path/to/recording",
  "transmux": {
    "custom_s3": "http://user:password@host:port/bucket/path?path_style=true"  // optional
  },
  "compose": {
    "audio": true,
    "video": true,
    "custom_s3": "presigned_url"   // optional
  }
}
```

The API will return a job ID that you can use to track the conversion progress:

```json
{
  "status": true,
  "data": {
    "job_id": "job_12345"
  }
}
```

Key parameters:
- `record_path`: S3 path to the source recording
- `transmux`: (Optional) Settings for converting to separate media files
- `compose`: (Optional) Settings for composing into a single file
  - `audio`: Enable audio composition
  - `video`: Enable video composition
  - `custom_s3`: Optional custom S3 output location

At least one of `transmux` or `compose` must be specified in the request.

After you got job id, each time the worker updates the job status, it will fire a hook event to your registered endpoint. You can use this event to update your database.

Here's list of the event structure:

- `Started`: { job_id: string } When a job is created
- `Completed`: { job_id: string } When a job is completed
- `Failed`: { job_id: string } When a job is failed


For more information about the event structure, please refer to protobuf definition in [media-server](https://github.com/8xFF/atm0s-media-server/blob/master/packages/protocol/proto/cluster/connector.proto).

```proto
message HookEvent {
    uint32 node = 1;
    uint64 ts = 2;
    oneof event {
        RoomEvent room = 3;
        PeerEvent peer = 4;
        RecordEvent record = 5;
        ComposeEvent compose = 6;
    }
}

message ComposeEvent {
    message RecordJobStarted {
        
    }

    message RecordJobFailed {
        string error = 2;
    }

    message RecordJobCompleted {
        message TrackTimeline {
            string path = 1;
            uint64 start = 2;
            uint64 end = 3; // Optional field, can be omitted
        }

        message TrackSummary {
            shared.Kind kind = 1;
            repeated TrackTimeline timeline = 2;
        }

        message SessionSummary {
            map<string, TrackSummary> track = 1;
        }

        message PeerSummary {
            map<uint64, SessionSummary> sessions = 1;
        }

        message TransmuxSummary {
            string metadata_json = 1;
            map<string, PeerSummary> peers = 2;
        }

        message ComposeSummary {
            string media_uri = 1;
        }

        TransmuxSummary transmux = 1;
        ComposeSummary compose = 2;
    }

    string app = 1;
    string job_id = 2;

    oneof event {
        RecordJobStarted started = 10;
        RecordJobFailed failed = 11;
        RecordJobCompleted completed = 12;
    }
}
```