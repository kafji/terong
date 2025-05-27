use super::Obfuscator;
use crate::event_logger::EventLog;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    io::{BufRead, BufReader, BufWriter, Read, Write},
    slice,
    sync::mpsc,
    thread,
};

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
