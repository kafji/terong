use crate::{event_logger::EventLog, input_event::KeyCode, server::input_source::event::LocalInputEvent};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    slice,
    sync::mpsc,
    thread,
};
use strum::IntoEnumIterator;

/// Obfuscator maps `E` to `Option<E>`.
/// When `None` is returned the event is omitted from the output.
pub trait Obfuscator {
    type Event;
    fn obfuscate(&mut self, event: Self::Event) -> Option<Self::Event>;
}

pub struct LocalInputEventObfuscator {
    table: HashMap<KeyCode, KeyCode>,
}

impl LocalInputEventObfuscator {
    pub fn new() -> Self {
        let mut table = HashMap::new();
        let mut avail: Vec<_> = KeyCode::iter().collect();
        for k in KeyCode::iter() {
            let i = getrandom::u32().unwrap() as usize % avail.len();
            let v = avail.remove(i);
            table.insert(k, v);
        }
        Self { table }
    }
}

impl Obfuscator for LocalInputEventObfuscator {
    type Event = LocalInputEvent;
    fn obfuscate(&mut self, event: LocalInputEvent) -> Option<LocalInputEvent> {
        let event = match event {
            LocalInputEvent::KeyDown { key } => LocalInputEvent::KeyDown { key: self.table[&key] },
            LocalInputEvent::KeyRepeat { key } => LocalInputEvent::KeyDown { key: self.table[&key] },
            LocalInputEvent::KeyUp { key } => LocalInputEvent::KeyDown { key: self.table[&key] },
            _ => event,
        };
        Some(event)
    }
}

pub fn obfuscate<O>(
    input: impl Read + Send + Sync,
    output: impl Write + Send + Sync,
    mut obfuscator: O,
) -> Result<u64, anyhow::Error>
where
    O: Obfuscator,
    O::Event: DeserializeOwned + Serialize + Clone + Send + Sync + 'static,
{
    thread::scope(|scope| {
        let (chunk_tx, chunk_rx) = mpsc::sync_channel(10);

        let reader = scope.spawn(move || {
            let chunk_size = 100_000;
            let mut r = BufReader::new(input);
            let mut line = String::new();
            let mut buf = Vec::with_capacity(chunk_size);
            while r.read_line(&mut line)? > 0 {
                let log: EventLog<O::Event> = serde_json::from_str(&line)?;
                line.clear();
                buf.push(log);
                if buf.len() >= chunk_size {
                    chunk_tx.send(buf)?;
                    buf = Vec::with_capacity(chunk_size);
                }
            }
            if !buf.is_empty() {
                chunk_tx.send(buf)?;
            }
            Result::<_, anyhow::Error>::Ok(())
        });

        let mut records = 0;
        let mut w = BufWriter::new(output);
        while let Ok(logs) = chunk_rx.recv() {
            let logs = logs
                .into_iter()
                .filter_map(|log| obfuscator.obfuscate(log.event).map(|event| EventLog { event, ..log }));
            for log in logs {
                serde_json::to_writer(&mut w, &log)?;
                w.write_all(slice::from_ref(&b'\n'))?;
                records += 1;
            }
        }
        w.flush()?;

        reader.join().unwrap()?;

        Result::<_, anyhow::Error>::Ok(records)
    })
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Cursor, SeekFrom},
        time::Duration,
    };

    use futures::TryStreamExt;
    use tokio::{io::AsyncSeekExt, task::yield_now, time::sleep};

    use crate::event_logger::EventLogger;

    use super::*;

    struct StringObfuscator;

    impl Obfuscator for StringObfuscator {
        type Event = String;
        fn obfuscate(&mut self, event: String) -> Option<String> {
            if event.is_empty() {
                None
            } else {
                let mut cipher = String::new();
                for c in event.chars().rev() {
                    cipher.push(c);
                }
                Some(cipher)
            }
        }
    }

    #[tokio::test]
    async fn test_obfuscate() {
        let obfuscated = {
            let store = Cursor::new(Vec::<u8>::new());
            let mut logger = EventLogger::new(store);
            logger.log("hello").await.unwrap();
            sleep(Duration::from_millis(100)).await;
            logger.log("").await.unwrap();
            sleep(Duration::from_millis(100)).await;
            logger.log("world").await.unwrap();
            logger.flush().await.unwrap();
            yield_now().await;
            let mut input = logger.store.lock().await;
            input.seek(SeekFrom::Start(0)).await.unwrap();
            let mut output = Vec::new();
            obfuscate(&mut *input, &mut output, StringObfuscator).unwrap();
            output
        };

        let mut logger = EventLogger::new(Cursor::new(obfuscated));
        let logs = logger
            .stream()
            .await
            .unwrap()
            .try_collect::<Vec<EventLog<String>>>()
            .await
            .unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(
            logs[0],
            EventLog {
                event: "olleh".to_owned(),
                stamp: 0
            }
        );
        assert_eq!(logs[1].event, "dlrow");
        assert!(logs[1].stamp > 0);
    }
}
