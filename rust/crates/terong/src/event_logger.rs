use anyhow::anyhow;
use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    io::{IoSlice, SeekFrom},
    sync::Arc,
    time::Instant,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufReader},
    spawn,
    sync::{Mutex, mpsc},
};

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct EventLog<E> {
    pub event: E,
    pub stamp: u64,
}

pub struct EventLogger<T, E> {
    store: Arc<Mutex<T>>,
    start: Option<Instant>,
    log_tx: mpsc::Sender<EventLog<E>>,
}

impl<T, E> EventLogger<T, E>
where
    T: AsyncWrite + AsyncSeek + Unpin + Send + 'static,
    E: Serialize + Send + 'static,
{
    pub fn new(store: T) -> Self {
        let store = Arc::new(Mutex::new(store));
        let (log_tx, mut log_rx) = mpsc::channel(1);

        // this task will run until the EventLogger is dropped via log_tx
        spawn({
            let store = store.clone();
            async move {
                loop {
                    let log = if let Some(log) = log_rx.recv().await {
                        log
                    } else {
                        break;
                    };
                    let log = serde_json::to_string(&log)?;
                    let mut store = store.lock().await;
                    store
                        .write_vectored(&[IoSlice::new(log.as_bytes()), IoSlice::new(b"\n")])
                        .await?;
                    store.flush().await?;
                }
                Result::<_, anyhow::Error>::Ok(())
            }
        });

        Self {
            store,
            start: Default::default(),
            log_tx,
        }
    }

    pub async fn log(&mut self, event: E) -> Result<(), anyhow::Error> {
        let stamp = if let Some(start) = self.start {
            let now = Instant::now();
            let d = now - start;
            match d.as_nanos().try_into() {
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
        self.log_tx
            .send(log)
            .await
            .map_err(|_| anyhow!("failed to send log message, channel closed"))?;
        Ok(())
    }
}

impl<T, E> EventLogger<T, E>
where
    T: AsyncRead + AsyncSeek + Unpin,
    E: DeserializeOwned,
{
    pub async fn stream(&mut self) -> Result<impl Stream<Item = Result<EventLog<E>, anyhow::Error>>, anyhow::Error> {
        let s = stream! {
            let mut store = self.store.lock().await;
            let store_ref = &mut *store;
            let mut buf = BufReader::new(store_ref);
            buf.seek(SeekFrom::Start(0)).await?;
            let mut lines = buf.lines();
            while let Some(line) = lines.next_line().await? {
                yield serde_json::from_str(&line).map_err(|err| anyhow!(err));
            }
            store.seek(SeekFrom::End(0)).await?;
        };
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::TryStreamExt;
    use std::io::Cursor;
    use tokio::task::yield_now;

    #[tokio::test]
    async fn test_rwrwr() {
        let store = Cursor::new(Vec::<u8>::new());
        let mut logger = EventLogger::<_, String>::new(store);

        let logs = logger.stream().await.unwrap().try_collect::<Vec<_>>().await.unwrap();
        assert!(logs.is_empty());

        {
            logger.log("hello".to_owned()).await.unwrap();
            // let the write actor run
            yield_now().await;
        }
        let logs = logger.stream().await.unwrap().try_collect::<Vec<_>>().await.unwrap();
        assert_eq!(
            logs,
            &[EventLog {
                event: "hello".to_owned(),
                stamp: 0
            }]
        );

        {
            logger.log("world".to_owned()).await.unwrap();
            // let the write actor run
            yield_now().await;
        }
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
