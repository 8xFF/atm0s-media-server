# Media Server

The media server is implemented by waiting for client requests and then forking a task to run each request. Each task runs in a separate process.

Because each transport has a different way of running tasks, we need to implement a different task runner for each transport, which is done in the servers/media-server/ directory.

In the future, we are considering splitting the media-server into a library and a binary for easier use in other projects and for extending the logic.
