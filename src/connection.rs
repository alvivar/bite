use std::{
    collections::VecDeque,
    io::{
        self,
        ErrorKind::{BrokenPipe, Interrupted, WouldBlock},
        Read, Write,
    },
    net::{SocketAddr, TcpStream},
    time::Instant,
};

use crate::message::Messages;

const BUFFER_SIZE: usize = 4096;

pub struct Connection {
    pub id: usize,
    pub socket: TcpStream,
    pub addr: SocketAddr,
    pub send_queue: SendQueue,
    pub messages: Messages,
    pub last_read: Instant,
    pub last_write: Instant,
    pub closed: bool,
}

impl Connection {
    pub fn new(id: usize, socket: TcpStream, addr: SocketAddr) -> Connection {
        Connection {
            id,
            socket,
            addr,
            send_queue: SendQueue::new(),
            messages: Messages::new(),
            last_read: Instant::now(),
            last_write: Instant::now(),
            closed: false,
        }
    }

    pub fn try_read(&mut self) -> io::Result<Vec<u8>> {
        match read(&mut self.socket) {
            Ok(data) => Ok(data),

            Err(err) => {
                self.closed = true;

                Err(err)
            }
        }
    }

    pub fn try_write(&mut self) -> io::Result<()> {
        match self.send_queue.write_to(&mut self.socket) {
            Ok(()) => Ok(()),

            Err(err) => {
                self.closed = true;

                Err(err)
            }
        }
    }
}

fn read(socket: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);

    loop {
        let mut chunk = vec![0; BUFFER_SIZE];

        match socket.read(&mut chunk) {
            Ok(0) => {
                // Reading 0 bytes means the other side has closed the
                // connection or is done writing, then so are we.
                return Err(BrokenPipe.into());
            }

            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);

                if n < BUFFER_SIZE {
                    break;
                }
            }

            // Would block "errors" are the OS's way of saying that the
            // connection is not actually ready to perform this I/O operation.
            Err(ref err) if err.kind() == WouldBlock => break,

            // Got interrupted, we'll try again.
            Err(ref err) if err.kind() == Interrupted => continue,

            // Other errors we'll consider fatal.
            Err(err) => return Err(err),
        }
    }

    Ok(buffer)
}

/// Frames waiting to be written, in order. The front frame stays queued until
/// all its bytes are written, `sent` counts the ones already written.
pub struct SendQueue {
    frames: VecDeque<Vec<u8>>,
    sent: usize,
}

impl SendQueue {
    pub fn new() -> SendQueue {
        SendQueue {
            frames: VecDeque::new(),
            sent: 0,
        }
    }

    pub fn push(&mut self, frame: Vec<u8>) {
        self.frames.push_back(frame);
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Writes queued bytes until the queue is empty or the writer would block,
    /// keeping whatever is left for the next call.
    fn write_to(&mut self, writer: &mut impl Write) -> io::Result<()> {
        while let Some(frame) = self.frames.front() {
            match writer.write(&frame[self.sent..]) {
                Ok(0) => {
                    // Writing 0 bytes means the other side has closed the
                    // connection or is done writing, then so are we.
                    return Err(BrokenPipe.into());
                }

                Ok(n) => {
                    self.sent += n;

                    if self.sent == frame.len() {
                        self.frames.pop_front();
                        self.sent = 0;
                    }
                }

                // Would block "errors" are the OS's way of saying that the
                // connection is not actually ready to perform this I/O
                // operation. We'll continue when it becomes writable.
                Err(ref err) if err.kind() == WouldBlock => return Ok(()),

                // Got interrupted, we'll try again.
                Err(ref err) if err.kind() == Interrupted => continue,

                // Other errors we'll consider fatal.
                Err(err) => return Err(err),
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    enum Step {
        Accept(usize),
        Fail(ErrorKind),
    }

    /// A writer that follows a script, one step per `write` call.
    struct ScriptedWriter {
        steps: VecDeque<Step>,
        written: Vec<u8>,
    }

    impl ScriptedWriter {
        fn new(steps: Vec<Step>) -> ScriptedWriter {
            ScriptedWriter {
                steps: steps.into(),
                written: Vec::new(),
            }
        }
    }

    impl Write for ScriptedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            match self.steps.pop_front().expect("unexpected write") {
                Step::Accept(limit) => {
                    let n = limit.min(buf.len());
                    self.written.extend_from_slice(&buf[..n]);
                    Ok(n)
                }

                Step::Fail(kind) => Err(Error::from(kind)),
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn queue(frames: &[&[u8]]) -> SendQueue {
        let mut queue = SendQueue::new();
        for frame in frames {
            queue.push(frame.to_vec());
        }
        queue
    }

    #[test]
    fn would_block_before_progress_keeps_frame() {
        let mut queue = queue(&[b"abc"]);
        let mut writer = ScriptedWriter::new(vec![Step::Fail(WouldBlock)]);

        queue.write_to(&mut writer).unwrap();

        assert!(writer.written.is_empty());
        assert_eq!(queue.frames, [b"abc"]);
        assert_eq!(queue.sent, 0);
    }

    #[test]
    fn partial_write_resumes_from_offset() {
        let mut queue = queue(&[b"abcdef"]);

        let mut writer = ScriptedWriter::new(vec![Step::Accept(2), Step::Fail(WouldBlock)]);
        queue.write_to(&mut writer).unwrap();
        assert_eq!(writer.written, b"ab");
        assert_eq!(queue.frames, [b"abcdef"]);
        assert_eq!(queue.sent, 2);

        let mut writer = ScriptedWriter::new(vec![Step::Fail(Interrupted), Step::Accept(10)]);
        queue.write_to(&mut writer).unwrap();
        assert_eq!(writer.written, b"cdef");
        assert!(queue.is_empty());
        assert_eq!(queue.sent, 0);
    }

    #[test]
    fn frames_keep_order_across_blocked_writes() {
        let mut queue = queue(&[b"abc", b"def"]);
        let mut written = Vec::new();

        let mut writer = ScriptedWriter::new(vec![
            Step::Accept(3),
            Step::Accept(1),
            Step::Fail(WouldBlock),
        ]);
        queue.write_to(&mut writer).unwrap();
        written.extend(writer.written);
        assert_eq!(queue.frames, [b"def"]);
        assert_eq!(queue.sent, 1);

        queue.push(b"ghi".to_vec());

        let mut writer = ScriptedWriter::new(vec![Step::Accept(2), Step::Accept(3)]);
        queue.write_to(&mut writer).unwrap();
        written.extend(writer.written);

        assert_eq!(written, b"abcdefghi");
        assert!(queue.is_empty());
    }

    #[test]
    fn zero_write_fails() {
        let mut queue = queue(&[b"abc"]);
        let mut writer = ScriptedWriter::new(vec![Step::Accept(0)]);

        let err = queue.write_to(&mut writer).unwrap_err();

        assert_eq!(err.kind(), BrokenPipe);
    }

    #[test]
    fn write_error_fails() {
        let mut queue = queue(&[b"abc"]);
        let mut writer = ScriptedWriter::new(vec![Step::Fail(ErrorKind::ConnectionReset)]);

        let err = queue.write_to(&mut writer).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::ConnectionReset);
    }
}
