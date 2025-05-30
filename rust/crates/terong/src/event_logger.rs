//! Provides utilities to record and obfuscate event logs.

pub mod obfuscate;

use anyhow::anyhow;
use async_stream::stream;
use bytes::{BufMut, BytesMut};
use futures::{Stream, TryStreamExt};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{io::SeekFrom, sync::Arc, time::Instant};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufReader},
    pin, spawn,
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
    op_tx: mpsc::Sender<WriteOp<E>>,
}

#[derive(Debug)]
enum WriteOp<E> {
    Write(EventLog<E>),
    Flush,
}

impl<T, E> EventLogger<T, E>
where
    T: AsyncWrite + AsyncSeek + Unpin + Send + 'static,
    E: Serialize + Send + 'static,
{
    pub fn new(store: T) -> Self {
        let store = Arc::new(Mutex::new(store));

        let (op_tx, mut op_rx) = mpsc::channel(1);

        // this actor will run until the EventLogger is dropped via op_rx
        spawn({
            let store = store.clone();
            async move {
                let mut buf = BytesMut::new();
                macro_rules! flush {
                    () => {{
                        let mut store = store.lock().await;
                        let buf = buf.split();
                        store.write_all(&buf).await?;
                        store.flush().await?;
                    }};
                }
                while let Some(op) = op_rx.recv().await {
                    match op {
                        WriteOp::Write(log) => {
                            let mut w = buf.writer();
                            serde_json::to_writer(&mut w, &log)?;
                            buf = w.into_inner();
                            buf.put_u8(b'\n');
                            if buf.len() >= 4096 {
                                flush!();
                            }
                        }
                        WriteOp::Flush => {
                            flush!();
                        }
                    }
                }
                Result::<_, anyhow::Error>::Ok(())
            }
        });

        Self {
            store,
            start: Default::default(),
            op_tx,
        }
    }

    pub async fn log(&mut self, event: E) -> Result<(), anyhow::Error> {
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
        self.op_tx
            .send(WriteOp::Write(log))
            .await
            .map_err(|_| anyhow!("failed to send write op"))?;
        Ok(())
    }

    pub async fn flush(&mut self) -> Result<(), anyhow::Error> {
        self.op_tx
            .send(WriteOp::Flush)
            .await
            .map_err(|_| anyhow!("failed to send flush op"))
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
            store.seek(SeekFrom::Start(0)).await?;
            {
                let logs = read_logs(&mut *store);
                pin!(logs);
                while let Some(log) = logs.try_next().await? {
                    yield Ok(log);
                }
            }
            store.seek(SeekFrom::End(0)).await?;
        };
        Ok(s)
    }
}

pub fn read_logs<E>(r: impl AsyncRead + Unpin) -> impl Stream<Item = Result<EventLog<E>, anyhow::Error>>
where
    E: DeserializeOwned,
{
    stream! {
        let mut buf = BufReader::new(r);
        let mut line = String::new();
        while buf.read_line(&mut line).await? > 0 {
            yield serde_json::from_str(&line).map_err(|err| anyhow!(err));
            line.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::TryStreamExt;
    use std::{io::Cursor, time::Duration};
    use tokio::{task::yield_now, time::sleep};

    #[tokio::test]
    async fn test_rwrwr() {
        let store = Cursor::new(Vec::<u8>::new());
        let mut logger = EventLogger::new(store);

        let logs = logger.stream().await.unwrap().try_collect::<Vec<_>>().await.unwrap();
        assert!(logs.is_empty());

        {
            logger.log("hello".to_owned()).await.unwrap();
            logger.flush().await.unwrap();
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
            sleep(Duration::from_millis(100)).await;
            logger.log("world".to_owned()).await.unwrap();
            logger.flush().await.unwrap();
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
        assert_eq!(logs[1].event, "world");
        assert!(logs[1].stamp > logs[0].stamp);
    }
}
