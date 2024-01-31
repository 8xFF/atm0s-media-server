# Media Server

Media server is implementing by wait client request then fork a task for running that request. Each task will be run in a separated task.

Because each transport have different way to run task, so we need to implement different task runner for each transport. Which is done in servers/media-server/ directory.

In feature, we are considering to split media-server into lib and bin, for easier to use in other project and extending logic.