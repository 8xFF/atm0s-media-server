//! This module implements the state machine pattern for easy state transition
//!

use derive_more::From;
use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
};

pub trait StateQueue<T> {
    fn push(&mut self, event: T);
    fn pop(&mut self) -> Option<T>;
}

pub struct StateDestroyingQueue<T> {
    data: VecDeque<T>,
    destroy_event: Option<T>,
}

impl<T> Deref for StateDestroyingQueue<T> {
    type Target = VecDeque<T>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> StateDestroyingQueue<T> {
    pub fn build(data: &mut VecDeque<T>, destroy_event: T) -> Self {
        let data = std::mem::take(data);
        Self {
            data,
            destroy_event: Some(destroy_event),
        }
    }
}

impl<T> StateQueue<T> for StateDestroyingQueue<T> {
    fn push(&mut self, event: T) {
        self.data.push_back(event);
    }

    fn pop(&mut self) -> Option<T> {
        if self.destroy_event.is_none() {
            assert_eq!(self.data.is_empty(), true);
            return None;
        }
        if self.data.is_empty() {
            self.destroy_event.take()
        } else {
            self.data.pop_front()
        }
    }
}

pub struct McContext<C, S> {
    ctx: C,
    next: Option<S>,
}

impl<C, S> McContext<C, S> {
    pub fn switch(&mut self, next: S) {
        self.next = Some(next);
    }

    pub fn next_state(&mut self) -> Option<S> {
        self.next.take()
    }
}

impl<C, S> From<C> for McContext<C, S> {
    fn from(ctx: C) -> Self {
        Self { ctx, next: None }
    }
}

impl<C, S> Deref for McContext<C, S> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}

impl<C, S> DerefMut for McContext<C, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctx
    }
}

impl<T> StateQueue<T> for VecDeque<T> {
    fn push(&mut self, event: T) {
        self.push_back(event);
    }

    fn pop(&mut self) -> Option<T> {
        self.pop_front()
    }
}

#[allow(unused)]
#[cfg(test)]
mod tests {
    use super::*;

    enum Input {
        Req,
        Destroy,
    }

    enum Output {
        Res(Result<(), ()>),
        Destroy,
    }

    struct Context {}

    trait State {
        fn on_event(&mut self, ctx: &mut McContext<Context, Box<dyn State>>, event: Input);
        fn pop_event(&mut self, ctx: &mut McContext<Context, Box<dyn State>>) -> Option<Output>;
    }

    struct Running(VecDeque<Output>);
    struct Destroying(StateDestroyingQueue<Output>);

    struct Object {
        ctx: McContext<Context, Box<dyn State>>,
        states: Box<dyn State>,
    }

    impl Object {
        pub fn new() -> Self {
            Self {
                ctx: Context {}.into(),
                states: Box::new(Running(VecDeque::new())),
            }
        }

        pub fn on_event(&mut self, event: Input) {
            self.states.on_event(&mut self.ctx, event);
            if let Some(next) = self.ctx.next_state() {
                self.states = next;
            }
        }

        pub fn pop_event(&mut self, ctx: &mut McContext<Context, Box<dyn State>>) -> Option<Output> {
            self.states.pop_event(ctx)
        }
    }

    impl State for Running {
        fn on_event(&mut self, ctx: &mut McContext<Context, Box<dyn State>>, event: Input) {
            match event {
                Input::Req => {
                    self.0.push_back(Output::Res(Ok(())));
                }
                Input::Destroy => ctx.switch(Box::new(Destroying(StateDestroyingQueue::build(&mut self.0, Output::Destroy)))),
            }
        }

        fn pop_event(&mut self, _ctx: &mut McContext<Context, Box<dyn State>>) -> Option<Output> {
            self.0.pop_front()
        }
    }

    impl State for Destroying {
        fn on_event(&mut self, _ctx: &mut McContext<Context, Box<dyn State>>, event: Input) {
            match event {
                Input::Req => {
                    self.0.push(Output::Res(Err(())));
                }
                Input::Destroy => {
                    //do nothing
                }
            }
        }

        fn pop_event(&mut self, _ctx: &mut McContext<Context, Box<dyn State>>) -> Option<Output> {
            self.0.pop()
        }
    }
}
