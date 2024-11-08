# Resource Clear

This server is built on top of [sans-io-runtime](https://github.com/atm0s-org/sans-io-runtime), which is a runtime for building server applications. With this reason we don't use mechanism of `Drop` to release resource. Instead, we use a queue to manage the resource release.

We have 2 types of task:

- Self-managed task: example is endpoint task
- Parrent managed task: example is cluster room task

For independent task, the task itself determine when to kill itself. For dependent task, the task will be killed when all the dependent task is killed. With each task type we have difference way to kill the task.

## Independent task

It keep an internal queue, when the task consider to be destroyed, it will switch to Destroying state, then wait for the queue to be empty. Once the queue is empty, the task will be killed.

Example with endpoint task, itself don't have queue but it has endpoint internal struct, which has queue. When endpoint task consider to be destroyed (Transport Disconnected), it will clear all resource (tracks ...) and switch to Destroying state, then wait for the queue to be empty. Once the queue is empty, the task will be killed.

## Dependent task

Each time it dependent task is removed, it will check if it safety to destroy with 2 conditions:

- All the dependent task is removed
- The queue is empty

If two conditions are met, the task will be killed immediately.

Example with cluster room task, it has some child dependent task like message channel, audio mixer, media track. Each time 