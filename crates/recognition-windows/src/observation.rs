//! Ordered observation state, independent of OS events and input hooks.
use crate::{ContextOutput, ContextReadResult, Error};
#[cfg(windows)]
use pointer_gesture_recognition::Output;
use std::collections::VecDeque;
/// One nonblocking read, distinguishing temporary silence from drained completion.
#[derive(Debug)]
#[cfg(windows)]
#[expect(
    clippy::large_enum_variant,
    reason = "Transient reads keep frames inline to avoid per-input allocation"
)]
pub enum ReadResult {
    /// An owned observation; deliver effects before reading another.
    Frame(Output),
    /// No observation yet; wait before reading again.
    Pending,
    /// Normal completion after final observations have drained.
    Completed,
}
pub(crate) struct Buffer {
    frames: VecDeque<ContextOutput>,
    capacity: usize,
    lagged: bool,
    outcome: Option<Result<(), Error>>,
}
impl Buffer {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            frames: VecDeque::with_capacity(capacity + 2),
            capacity,
            lagged: false,
            outcome: None,
        }
    }
    pub(crate) fn send(&mut self, output: ContextOutput) -> Result<(), Error> {
        if self.outcome.is_some() {
            return Err(Error::WorkerStopped);
        }
        self.lagged |= self.frames.len() >= self.capacity;
        // The caller stops capture at the first overflow. Two reserved slots
        // retain that triggering observation and the subsequent shutdown frame.
        if self.frames.len() < self.capacity + 2 {
            self.frames.push_back(output);
        }
        if self.lagged {
            Err(Error::ConsumerLagged)
        } else {
            Ok(())
        }
    }
    pub(crate) fn ready(&self) -> bool {
        !self.frames.is_empty() || self.outcome.is_some()
    }
    pub(crate) fn outcome(&self) -> Option<Result<(), Error>> {
        self.outcome
    }
    pub(crate) fn finish(&mut self, outcome: Result<(), Error>) {
        self.outcome = Some(if self.lagged && outcome.is_ok() {
            Err(Error::ConsumerLagged)
        } else {
            outcome
        });
    }
    pub(crate) fn take(&mut self) -> Result<ContextReadResult, Error> {
        match self.frames.pop_front() {
            Some(frame) => Ok(ContextReadResult::Frame(frame)),
            None => self.outcome.map_or(Ok(ContextReadResult::Pending), |r| {
                r.map(|()| ContextReadResult::Completed)
            }),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use pointer_gesture_recognition::{Configuration, Engine, Event};
    fn frame(generation: u64) -> ContextOutput {
        let mut frame = Engine::new(Configuration {
            start_distance: 10.,
            change_distance: 5.,
            total_duration_ns: 0,
            idle_duration_ns: 0,
            maximum_points: 8,
            maximum_directions: 8,
            button: 1,
            passes_through_empty_matcher: true,
        })
        .unwrap()
        .step(Event::Start, 0);
        frame.input_generation = generation;
        ContextOutput {
            output: frame,
            epoch: Some(generation),
        }
    }
    #[test]
    fn overflow_retains_triggering_and_final_observations_in_order() {
        let mut buffer = Buffer::new(1);
        buffer.send(frame(1)).unwrap();
        assert_eq!(buffer.send(frame(2)), Err(Error::ConsumerLagged));
        assert_eq!(buffer.send(frame(3)), Err(Error::ConsumerLagged));
        buffer.finish(Err(Error::ConsumerLagged));
        for expected in 1..=3 {
            match buffer.take().unwrap() {
                ContextReadResult::Frame(frame) => {
                    assert_eq!(frame.output.input_generation, expected);
                    assert_eq!(frame.epoch, Some(expected));
                }
                _ => panic!("Expected retained overflow frame {expected}"),
            }
        }
        assert!(matches!(buffer.take(), Err(Error::ConsumerLagged)));
    }
    #[test]
    fn empty_is_not_completion_and_closed_buffers_reject_later_publication() {
        let mut buffer = Buffer::new(1);
        assert!(!buffer.ready());
        assert!(matches!(buffer.take().unwrap(), ContextReadResult::Pending));
        buffer.send(frame(1)).unwrap();
        assert!(buffer.ready());
        assert!(matches!(
            buffer.take().unwrap(),
            ContextReadResult::Frame(_)
        ));
        assert!(!buffer.ready());
        buffer.finish(Ok(()));
        assert_eq!(buffer.outcome(), Some(Ok(())));
        assert!(buffer.ready());
        assert!(matches!(
            buffer.take().unwrap(),
            ContextReadResult::Completed
        ));
        assert_eq!(buffer.send(frame(2)), Err(Error::WorkerStopped));
        assert!(matches!(
            buffer.take().unwrap(),
            ContextReadResult::Completed
        ));
    }
}
