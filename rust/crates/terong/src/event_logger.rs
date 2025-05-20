use anyhow::anyhow;
use futures::{Stream, TryStreamExt, future};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    io::{IoSlice, SeekFrom},
    marker::PhantomData,
    time::Instant,
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio_stream::wrappers::LinesStream;

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct EventLog<E> {
    pub event: E,
    pub stamp: u64,
}

pub struct EventLogger<T, E> {
    store: T,
    start: Option<Instant>,
    _event: PhantomData<E>,
}

impl<T, E> EventLogger<T, E> {
    pub fn new(store: T) -> Self {
        Self {
            store,
            start: Default::default(),
            _event: Default::default(),
        }
    }
}

impl<T, E> EventLogger<T, E>
where
    T: AsyncWrite + AsyncSeek + Unpin,
    E: Serialize,
{
    pub async fn log(&mut self, event: E) -> Result<(), anyhow::Error> {
        self.store.seek(SeekFrom::End(0)).await?;
        let stamp = if let Some(start) = self.start {
            let now = Instant::now();
            let d = now - start;
            match d.as_nanos().try_into() {
                Ok(s) => s,
                Err(_) => {
                    self.start = Some(Instant::now());
                    0
                }
            }
        } else {
            self.start = Some(Instant::now());
            0
        };
        let log = EventLog { event, stamp };
        let log = serde_json::to_string(&log)?;
        self.store
            .write_vectored(&[IoSlice::new(log.as_bytes()), IoSlice::new(b"\n")])
            .await?;
        self.store.flush().await?;
        Ok(())
    }
}

impl<T, E> EventLogger<T, E>
where
    T: AsyncRead + AsyncSeek + Unpin,
    E: DeserializeOwned,
{
    pub async fn stream(&mut self) -> Result<impl Stream<Item = Result<EventLog<E>, anyhow::Error>>, anyhow::Error> {
        let mut buf = BufReader::new(&mut self.store);
        buf.seek(SeekFrom::Start(0)).await?;
        let lines = LinesStream::new(buf.lines());
        let s = lines
            .map_err(|err| anyhow!(err))
            .and_then(|line| future::ready(serde_json::from_str(&line).map_err(|err| anyhow!(err))));
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_rwrwr() {
        let store = Cursor::new(Vec::<u8>::new());
        let mut logger = EventLogger::<_, String>::new(store);

        let logs = logger.stream().await.unwrap().try_collect::<Vec<_>>().await.unwrap();
        assert!(logs.is_empty());

        logger.log("hello".to_owned()).await.unwrap();
        let logs = logger.stream().await.unwrap().try_collect::<Vec<_>>().await.unwrap();
        assert_eq!(
            logs,
            &[EventLog {
                event: "hello".to_owned(),
                stamp: 0
            }]
        );

        logger.log("world".to_owned()).await.unwrap();
        let logs = logger.stream().await.unwrap().try_collect::<Vec<_>>().await.unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(
            logs[0],
            EventLog {
                event: "hello".to_owned(),
                stamp: 0,
            }
        );
        assert_eq!(logs[1].event, "world".to_owned());
        assert!(logs[1].stamp > logs[0].stamp);
    }
}
