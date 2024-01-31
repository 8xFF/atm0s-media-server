# Recording (Incomplete)

For maximum performance and flexibility, we don't do any media transcode when recording. Instead, we record raw media data and store it in a file. This way also can be very flexible, for example we can record all rooms, but only compose some rooms to a single file after that.

For simple integrate, we use s3 as storage backend. Each media-server will collect raw streams and put to s3 upload queue. S3 destination is configured in media-server config file or fetch dynamic from connector service.

TODO: write about some best practice.