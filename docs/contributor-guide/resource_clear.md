# Resource Clear

This server is built on top of [sans-io-runtime](https://github.com/atm0s-org/sans-io-runtime), which is a runtime for building server applications. With this reason we don't use mechanism of `Drop` to release resource. Instead, we use a queue to manage the resource release.

We have 2 types of tasks:

- Self-managed tasks: for example, the endpoint task
- Dependent tasks: for example, the cluster room task

For self-managed tasks, the task itself determines when to kill itself. For dependent tasks, the task will be killed when all of its dependent tasks are un-linked. Each task type has a different way to handle task termination.

## Self-managed task

Example: The endpoint task can automatically release resources when the client disconnects.

## Dependent task

Example: The cluster room task needs to wait for all dependent tasks (endpoint track, mixer, or data channel) to be un-linked before destroying itself.
