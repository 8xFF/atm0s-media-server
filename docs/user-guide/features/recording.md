# Recording (Incomplete)

For maximum performance and flexibility, we don't do any media transcoding when recording. Instead, we record raw media data and store it in a file. This way can also be very flexible; for example, we can record all rooms but only compose some rooms to a single file after that. This way also allow us to replay the recording without any transcoding.

For simple integration, we use S3 as the storage backend. Each media server will collect raw streams and put them in the S3 upload queue. The S3 destination is configured in the media server config file or fetched dynamically from the connector service.

TODO: Write about some best practices.

# How to use
