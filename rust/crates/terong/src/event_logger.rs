//! Provides utilities to record and obfuscate event logs.

pub mod obfuscate;

use anyhow::anyhow;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    io::{BufRead, BufReader, Read, Write},
    marker::PhantomData,
    slice,
    time::Instant,
};

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct EventLog<E> {
    pub event: E,
    pub stamp: u64,
}

#[derive(Debug)]
pub struct EventLogger<W, E> {
    writer: W,
    start: Option<Instant>,
    _event: PhantomData<E>,
}

impl<W, E> EventLogger<W, E>
where
    W: Write,
    E: Serialize + Send + Sync + 'static,
{
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            start: Default::default(),
            _event: Default::default(),
        }
    }

    pub fn log(&mut self, event: E) -> Result<(), anyhow::Error> {
        let stamp = if let Some(start) = self.start {
            let now = Instant::now();
            let d = now - start;
            match d.as_millis().try_into() {
                Ok(s) => s,
                Err(_) => {
                    // stamp can't fit in u64, rollover
                    self.start = Some(Instant::now());
                    0
                }
            }
        } else {
            self.start = Some(Instant::now());
            0
        };
        let log = EventLog { event, stamp };
        serde_json::to_writer(&mut self.writer, &log)?;
        self.writer.write_all(slice::from_ref(&b'\n'))?;
        Ok(())
    }
}

#[derive(Debug)]
struct Records<R, E> {
    source: BufReader<R>,
    line: String,
    _event: PhantomData<E>,
}

impl<R, E> Iterator for Records<R, E>
where
    R: Read,
    E: DeserializeOwned,
{
    type Item = Result<EventLog<E>, anyhow::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.line.clear();
        match self.source.read_line(&mut self.line) {
            Ok(n) => {
                if n == 0 {
                    return None;
                }
            }
            Err(err) => return Some(Err(anyhow!(err))),
        }
        match serde_json::from_str(&self.line) {
            Ok(r) => return Some(Ok(r)),
            Err(err) => return Some(Err(anyhow!(err))),
        }
    }
}

pub fn read_logs<E>(r: impl Read) -> impl Iterator<Item = Result<EventLog<E>, anyhow::Error>>
where
    E: DeserializeOwned,
{
    let r = BufReader::new(r);
    Records {
        source: r,
        line: String::new(),
        _event: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Cursor, Seek, SeekFrom},
        thread,
        time::Duration,
    };

    #[test]
    fn test_rwrwr() {
        let buffer = Cursor::new(Vec::<u8>::new());
        let mut logger = EventLogger::<_, String>::new(buffer);
        let logs = {
            logger.writer.seek(SeekFrom::Start(0)).unwrap();
            read_logs::<String>(&mut logger.writer)
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert!(logs.is_empty());

        logger.writer.seek(SeekFrom::End(0)).unwrap();
        logger.log("hello".to_owned()).unwrap();
        thread::sleep(Duration::from_millis(100));

        let logs = {
            logger.writer.seek(SeekFrom::Start(0)).unwrap();
            read_logs(&mut logger.writer).collect::<Result<Vec<_>, _>>().unwrap()
        };
        assert_eq!(
            logs,
            &[EventLog {
                event: "hello".to_owned(),
                stamp: 0
            }]
        );

        logger.writer.seek(SeekFrom::End(0)).unwrap();
        logger.log("world".to_owned()).unwrap();
        thread::sleep(Duration::from_millis(100));

        let logs = {
            logger.writer.seek(SeekFrom::Start(0)).unwrap();
            read_logs(&mut logger.writer).collect::<Result<Vec<_>, _>>().unwrap()
        };
        assert_eq!(logs.len(), 2);
        assert_eq!(
            logs[0],
            EventLog {
                event: "hello".to_owned(),
                stamp: 0,
            }
        );
        assert_eq!(logs[1].event, "world");
        assert!(logs[1].stamp >= 100, "stamp was {}", logs[1].stamp);
    }
}
