use super::Obfuscator;
use crate::event_logger::EventLog;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    io::{BufRead, BufReader, BufWriter, Read, Write},
    slice,
    sync::mpsc,
    thread,
};

pub fn obfuscate<O>(input: impl Read + Send + Sync, output: impl Write, mut obfuscator: O) -> Result<(), anyhow::Error>
where
    O: Obfuscator,
    O::Event: DeserializeOwned + Serialize + Clone + Send + Sync + 'static,
{
    thread::scope(|scope| {
        let (chunk_tx, chunk_rx) = mpsc::sync_channel(1);

        let reader = scope.spawn(move || {
            let mut r = BufReader::new(input);
            let mut line = String::new();
            let mut buf = Vec::new();
            while r.read_line(&mut line)? > 0 {
                let log: EventLog<O::Event> = serde_json::from_str(&line)?;
                line.clear();
                buf.push(log);
                if buf.len() >= 200_000 {
                    chunk_tx.send(buf.clone())?;
                    buf.clear();
                }
            }
            if !buf.is_empty() {
                chunk_tx.send(buf)?;
            }
            Result::<_, anyhow::Error>::Ok(())
        });

        let mut w = BufWriter::new(output);
        while let Ok(logs) = chunk_rx.recv() {
            for log in logs {
                if let Some(event) = obfuscator.obfuscate(log.event) {
                    let log = EventLog { event, ..log };
                    serde_json::to_writer(&mut w, &log)?;
                    w.write_all(slice::from_ref(&b'\n'))?;
                }
            }
        }
        w.flush()?;

        reader.join().unwrap()?;
        Result::<_, anyhow::Error>::Ok(())
    })
}
