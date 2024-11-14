# Standalone Mode

Standalone mode is designed for testing purposes, allowing you to run both a media server and gateway server on a single machine with minimal configuration.

## Prerequisites

Before starting, you only need to download media binary (or docker or build from source) and a GeoIP database.

## Basic Usage

### Prepare GeoIP database

```bash
mkdir -p maxminddb-data
cd maxminddb-data
wget https://github.com/P3TERX/GeoLite.mmdb/raw/download/GeoLite2-City.mmdb
```

### Start the server

Start the server with default logging configuration:

```bash
RUST_LOG=atm0s_sdn_network=error,info \
RUST_BACKTRACE=1 \
./atm0s-media-server standalone
```

## Advanced configuration

```bash
❯ ./atm0s-media-server standalone --help
Usage: atm0s-media-server standalone [OPTIONS]

Options:
      --console-port <CONSOLE_PORT>
          The port for console server [env: CONSOLE_PORT=] [default: 8080]
      --gateway-port <GATEWAY_PORT>
          The port for gateway server [env: GATEWAY_PORT=] [default: 3000]
      --geo-db <GEO_DB>
          The path to the GeoIP database [env: GEO_DB=] [default: ./maxminddb-data/GeoLite2-City.mmdb]
      --max-cpu <MAX_CPU>
          Maximum CPU usage (in percent) allowed for routing to a media node or gateway node [env: MAX_CPU=] [default: 60]
      --max-memory <MAX_MEMORY>
          Maximum memory usage (in percent) allowed for routing to a media node or gateway node [env: MAX_MEMORY=] [default: 80]
      --max-disk <MAX_DISK>
          Maximum disk usage (in percent) allowed for routing to a media node or gateway node [env: MAX_DISK=] [default: 90]
      --multi-tenancy-sync <MULTI_TENANCY_SYNC>
          Multi-tenancy sync endpoint [env: MULTI_TENANCY_SYNC=]
      --multi-tenancy-sync-interval-ms <MULTI_TENANCY_SYNC_INTERVAL_MS>
          Multi-tenancy sync interval in milliseconds [env: MULTI_TENANCY_SYNC_INTERVAL_MS=] [default: 30000]
      --record-cache <RECORD_CACHE>
          Record cache directory [env: RECORD_CACHE=] [default: ./record_cache/]
      --record-mem-max-size <RECORD_MEM_MAX_SIZE>
          Maximum size of the recording cache in bytes [env: RECORD_MEM_MAX_SIZE=] [default: 100000000]
      --record-upload-worker <RECORD_UPLOAD_WORKER>
          Number of workers for uploading recordings [env: RECORD_UPLOAD_WORKER=] [default: 5]
      --db-uri <DB_URI>
          DB Uri [env: DB_URI=] [default: sqlite://connector.db?mode=rwc]
      --s3-uri <S3_URI>
          S3 Uri [env: S3_URI=] [default: http://minioadmin:minioadmin@localhost:9000/record/?path_style=true]
      --hook-uri <HOOK_URI>
          Hook URI [env: HOOK_URI=]
      --hook-workers <HOOK_WORKERS>
          Number of workers for hook [env: HOOK_WORKERS=] [default: 8]
      --hook-body-type <HOOK_BODY_TYPE>
          Hook body type [env: HOOK_BODY_TYPE=] [default: protobuf-json] [possible values: protobuf-json, protobuf-binary]
      --destroy-room-after-ms <DESTROY_ROOM_AFTER_MS>
          Destroy room after no-one online, default is 2 minutes [env: DESTROY_ROOM_AFTER_MS=] [default: 120000]
      --storage-tick-interval-ms <STORAGE_TICK_INTERVAL_MS>
          Storage tick interval, default is 1 minute [env: STORAGE_TICK_INTERVAL_MS=] [default: 60000]
      --rtpengine-rtp-ip <RTPENGINE_RTP_IP>
          The IP address for RTPengine RTP listening. Default: 127.0.0.1 [env: RTPENGINE_RTP_IP=] [default: 127.0.0.1]
      --media-instance-count <MEDIA_INSTANCE_COUNT>
          Media instance count [env: MEDIA_INSTANCE_COUNT=] [default: 2]
  -h, --help
          Print help
```