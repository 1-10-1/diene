use std::time::{Duration, Instant};

/// A pausable stopwatch for measuring elapsed time.
#[derive(Debug)]

pub struct Stopwatch {
    state: State,
}

#[derive(Debug)]
enum State {
    Stopped,
    Running {
        start_time: Instant,
        accumulated: Duration,
    },
    Paused {
        accumulated: Duration,
    },
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}

impl Stopwatch {
    /// Creates a stopped stopwatch with zero elapsed time.
    pub fn new() -> Self {
        Self { state: State::Stopped }
    }

    /// Starts or resumes the stopwatch.
    ///
    /// Calling this while already running is a no-op.
    pub fn start(&mut self) {
        match self.state {
            State::Stopped => {
                self.state = State::Running {
                    start_time: Instant::now(),
                    accumulated: Duration::new(0, 0),
                };
            }
            State::Paused { accumulated } => {
                self.state = State::Running { start_time: Instant::now(), accumulated };
            }
            State::Running { .. } => {}
        }
    }

    /// Pauses the stopwatch.
    ///
    /// Calling this while stopped or already paused is a no-op.
    pub fn pause(&mut self) {
        if let State::Running { start_time, accumulated } = self.state {
            let elapsed = start_time.elapsed();
            self.state = State::Paused { accumulated: accumulated + elapsed };
        }
    }

    /// Stops the stopwatch and clears all accumulated elapsed time.
    pub fn reset(&mut self) {
        self.state = State::Stopped;
    }

    /// Returns elapsed time, excluding time spent paused.
    pub fn elapsed(&self) -> Duration {
        match self.state {
            State::Stopped => Duration::new(0, 0),
            State::Paused { accumulated } => accumulated,
            State::Running { start_time, accumulated } => accumulated + start_time.elapsed(),
        }
    }
}
