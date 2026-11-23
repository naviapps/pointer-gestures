//! One-at-a-time native work without blocking the input owner on that work.
use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
};

pub(crate) trait Signal: Send + Sync + 'static {
    fn signal(&self) -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SubmitError<J> {
    Busy(J),
    Disconnected(J),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Poll<R> {
    Pending,
    Completed(R),
    Panicked,
    Disconnected,
}

enum Completion<R> {
    Completed(R),
    Panicked,
}

pub(crate) struct Replay<J: Send + 'static, R: Send + 'static> {
    requests: Option<SyncSender<J>>,
    completions: Receiver<Completion<R>>,
    worker: Option<JoinHandle<()>>,
    in_flight: bool,
}

impl<J: Send + 'static, R: Send + 'static> Replay<J, R> {
    pub(crate) fn start(work: fn(J) -> R, signal: Arc<dyn Signal>) -> io::Result<Self> {
        let (requests, incoming) = mpsc::sync_channel(1);
        let (finished, completions) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("pointer-gesture-replay".into())
            .spawn(move || {
                while let Ok(job) = incoming.recv() {
                    let completion = match catch_unwind(AssertUnwindSafe(|| work(job))) {
                        Ok(result) => Completion::Completed(result),
                        Err(_) => Completion::Panicked,
                    };
                    let panicked = matches!(completion, Completion::Panicked);
                    // The one-in-flight contract guarantees room for this result.
                    // Publish before signaling so a woken owner can read immediately.
                    if finished.send(completion).is_err() {
                        break;
                    }
                    let signalled = signal.signal();
                    if panicked || !signalled {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests: Some(requests),
            completions,
            worker: Some(worker),
            in_flight: false,
        })
    }

    /// Accept at most one job until its completion is polled; never wait for work.
    /// A rejected job is returned unchanged. An accepted job is never retried.
    pub(crate) fn submit(&mut self, job: J) -> Result<(), SubmitError<J>> {
        let Some(requests) = self.requests.as_ref() else {
            return Err(SubmitError::Disconnected(job));
        };
        if self.in_flight {
            return Err(SubmitError::Busy(job));
        }
        match requests.try_send(job) {
            Ok(()) => {
                self.in_flight = true;
                Ok(())
            }
            Err(TrySendError::Full(job)) => Err(SubmitError::Busy(job)),
            Err(TrySendError::Disconnected(job)) => {
                self.requests.take();
                Err(SubmitError::Disconnected(job))
            }
        }
    }

    pub(crate) fn poll(&mut self) -> Poll<R> {
        match self.completions.try_recv() {
            Ok(Completion::Completed(result)) => {
                self.in_flight = false;
                Poll::Completed(result)
            }
            Ok(Completion::Panicked) => {
                self.in_flight = false;
                self.requests.take();
                Poll::Panicked
            }
            Err(TryRecvError::Empty) => Poll::Pending,
            Err(TryRecvError::Disconnected) => {
                self.in_flight = false;
                self.requests.take();
                Poll::Disconnected
            }
        }
    }

    pub(crate) fn thread(&self) -> Option<&JoinHandle<()>> {
        self.worker.as_ref()
    }

    /// The native caller must remove input hooks before joining or dropping this worker.
    /// An active native submission may itself need that owner to process hook messages.
    pub(crate) fn close_and_join(&mut self) -> Result<(), ()> {
        self.requests.take();
        self.worker
            .take()
            .map_or(Ok(()), |worker| worker.join().map_err(|_| ()))
    }
}

impl<J: Send + 'static, R: Send + 'static> Drop for Replay<J, R> {
    fn drop(&mut self) {
        let _ = self.close_and_join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicBool, Ordering::SeqCst},
        time::Duration,
    };

    const WAIT: Duration = Duration::from_secs(5);

    struct Notice {
        dropped: Arc<AtomicBool>,
    }
    impl Drop for Notice {
        fn drop(&mut self) {
            self.dropped.store(true, SeqCst);
        }
    }

    struct TestSignal {
        calls: mpsc::Sender<()>,
        succeeds: bool,
        _notice: Option<Notice>,
    }
    impl Signal for TestSignal {
        fn signal(&self) -> bool {
            self.calls.send(()).unwrap();
            self.succeeds
        }
    }

    struct Job {
        id: u32,
        accepted: u32,
        expected: u32,
        entered: Option<mpsc::Sender<()>>,
        release: Option<Receiver<()>>,
        panic: bool,
        _notice: Option<Notice>,
    }
    impl Job {
        fn new(id: u32) -> Self {
            Self {
                id,
                accepted: id,
                expected: id,
                entered: None,
                release: None,
                panic: false,
                _notice: None,
            }
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Outcome {
        id: u32,
        accepted: u32,
        expected: u32,
        worker: thread::ThreadId,
    }

    fn work(job: Job) -> Outcome {
        if let Some(entered) = job.entered.as_ref() {
            entered.send(()).unwrap();
        }
        if let Some(release) = job.release.as_ref() {
            release.recv_timeout(WAIT).unwrap();
        }
        assert!(!job.panic, "injected work panic");
        Outcome {
            id: job.id,
            accepted: job.accepted,
            expected: job.expected,
            worker: thread::current().id(),
        }
    }

    fn start(succeeds: bool) -> (Replay<Job, Outcome>, Receiver<()>) {
        let (calls, called) = mpsc::channel();
        let signal = Arc::new(TestSignal {
            calls,
            succeeds,
            _notice: None,
        });
        (Replay::start(work, signal).unwrap(), called)
    }

    fn submit(replay: &mut Replay<Job, Outcome>, job: Job) {
        assert!(replay.submit(job).is_ok());
    }

    fn completed(replay: &mut Replay<Job, Outcome>, called: &Receiver<()>) -> Outcome {
        called.recv_timeout(WAIT).unwrap();
        let Poll::Completed(outcome) = replay.poll() else {
            panic!("signalled completion was not published");
        };
        outcome
    }

    #[test]
    fn blocked_work_allows_owner_progress_and_returns_a_busy_job_unchanged() {
        let (mut replay, called) = start(true);
        let (entered, running) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let mut job = Job::new(1);
        job.entered = Some(entered);
        job.release = Some(gate);
        submit(&mut replay, job);
        running.recv_timeout(WAIT).unwrap();

        // These owner-side calls return while work is still held behind the gate.
        assert_eq!(replay.poll(), Poll::Pending);
        let Err(SubmitError::Busy(rejected)) = replay.submit(Job::new(2)) else {
            panic!("a second in-flight job was accepted");
        };
        assert_eq!(rejected.id, 2);

        release.send(()).unwrap();
        assert_eq!(completed(&mut replay, &called).id, 1);
        submit(&mut replay, rejected);
        assert_eq!(completed(&mut replay, &called).id, 2);
        replay.close_and_join().unwrap();
        assert!(called.try_recv().is_err());
    }

    #[test]
    fn sequential_requests_keep_order_and_use_the_same_worker() {
        let (mut replay, called) = start(true);
        let worker = replay.thread().unwrap().thread().id();
        assert_ne!(worker, thread::current().id());
        let mut outcomes = Vec::new();
        for id in 1..=3 {
            submit(&mut replay, Job::new(id));
            outcomes.push(completed(&mut replay, &called));
        }
        assert_eq!(
            outcomes.iter().map(|value| value.id).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert!(outcomes.iter().all(|value| value.worker == worker));
        replay.close_and_join().unwrap();
    }

    #[test]
    fn a_partial_outcome_is_returned_without_repeating_or_rewriting_work() {
        let (mut replay, called) = start(true);
        let mut job = Job::new(7);
        job.accepted = 1;
        job.expected = 4;
        submit(&mut replay, job);
        let outcome = completed(&mut replay, &called);
        assert_eq!((outcome.id, outcome.accepted, outcome.expected), (7, 1, 4));
        replay.close_and_join().unwrap();
        assert_eq!(replay.poll(), Poll::Disconnected);
        assert!(called.try_recv().is_err());
    }

    #[test]
    fn work_panic_is_published_and_signalled_once_before_worker_exit() {
        let (mut replay, called) = start(true);
        let mut job = Job::new(1);
        job.panic = true;
        submit(&mut replay, job);
        called.recv_timeout(WAIT).unwrap();
        assert_eq!(replay.poll(), Poll::Panicked);
        let Err(SubmitError::Disconnected(rejected)) = replay.submit(Job::new(2)) else {
            panic!("a panicked worker accepted more work");
        };
        assert_eq!(rejected.id, 2);
        replay.close_and_join().unwrap();
        assert_eq!(replay.poll(), Poll::Disconnected);
        assert!(called.try_recv().is_err());
    }

    #[test]
    fn signal_failure_preserves_published_outcome_and_exits_without_retrying() {
        let (mut replay, called) = start(false);
        submit(&mut replay, Job::new(9));
        assert_eq!(completed(&mut replay, &called).id, 9);
        replay.close_and_join().unwrap();
        assert_eq!(replay.poll(), Poll::Disconnected);
        assert!(called.try_recv().is_err());
        let Err(SubmitError::Disconnected(rejected)) = replay.submit(Job::new(10)) else {
            panic!("a closed worker accepted more work");
        };
        assert_eq!(rejected.id, 10);
    }

    #[test]
    fn close_and_drop_join_the_worker_and_release_its_resources() {
        for explicit_close in [true, false] {
            let job_dropped = Arc::new(AtomicBool::new(false));
            let signal_dropped = Arc::new(AtomicBool::new(false));
            let (calls, called) = mpsc::channel();
            let signal = Arc::new(TestSignal {
                calls,
                succeeds: true,
                _notice: Some(Notice {
                    dropped: Arc::clone(&signal_dropped),
                }),
            });
            let mut replay = Replay::start(work, signal).unwrap();
            let mut job = Job::new(1);
            job._notice = Some(Notice {
                dropped: Arc::clone(&job_dropped),
            });
            submit(&mut replay, job);
            if explicit_close {
                replay.close_and_join().unwrap();
                assert!(replay.thread().is_none());
                replay.close_and_join().unwrap();
                assert_eq!(completed(&mut replay, &called).id, 1);
                assert_eq!(replay.poll(), Poll::Disconnected);
            }
            drop(replay);
            assert!(job_dropped.load(SeqCst));
            assert!(signal_dropped.load(SeqCst));
        }
    }
}
