//! stdout as SPEC CLI-07 wants it: once its reader has closed it (a broken pipe, as in
//! `bench ledger journal | head`), the rest of the output is dropped and the command goes on to
//! the exit code it earns, so a success still exits 0 and a failure keeps its code.

use std::io::{self, Write};

/// A writer that drops everything after its inner writer reports [`io::ErrorKind::BrokenPipe`],
/// remembering that it did. Every other error passes through.
#[derive(Debug)]
pub(crate) struct Pipe<W> {
    inner: W,
    closed: bool,
}

impl<W> Pipe<W> {
    /// `inner`, not yet closed.
    pub(crate) const fn new(inner: W) -> Self {
        Self {
            inner,
            closed: false,
        }
    }

    /// Whether the reader closed the pipe: some output was dropped.
    pub(crate) const fn reader_closed(&self) -> bool {
        self.closed
    }

    /// Settles `result`: a broken pipe marks the pipe closed and counts as `done`.
    fn settle<T>(&mut self, result: io::Result<T>, done: T) -> io::Result<T> {
        match result {
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                self.closed = true;
                Ok(done)
            }
            other => other,
        }
    }
}

impl<W: Write> Write for Pipe<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Ok(buf.len());
        }
        let result = self.inner.write(buf);
        self.settle(result, buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        let result = self.inner.flush();
        self.settle(result, ())
    }
}

#[cfg(test)]
mod tests {
    use super::Pipe;
    use std::io::{self, Write};

    /// A writer whose reader has gone, or that fails some other way.
    struct Failing(io::ErrorKind);

    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(self.0.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(self.0.into())
        }
    }

    #[test]
    fn a_broken_pipe_drops_the_rest_and_is_remembered() {
        let mut pipe = Pipe::new(Failing(io::ErrorKind::BrokenPipe));

        let wrote = pipe.write_all(b"one\n").and_then(|()| pipe.flush());

        assert!(
            matches!(wrote, Ok(())),
            "the reader chose to stop: {wrote:?}"
        );
        assert!(pipe.reader_closed(), "and that is remembered");
    }

    #[test]
    fn any_other_failure_passes_through() {
        let mut pipe = Pipe::new(Failing(io::ErrorKind::StorageFull));

        let wrote = pipe.write_all(b"one\n");

        assert_eq!(
            wrote.map_err(|e| e.kind()),
            Err(io::ErrorKind::StorageFull),
            "still an output error"
        );
        assert!(!pipe.reader_closed(), "not a closed reader");
    }
}
