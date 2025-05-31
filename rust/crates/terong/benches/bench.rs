use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use terong::{event_buffer::EventBuffer, event_logger::read_logs, server::input_source::event::LocalInputEvent};

fn events() -> Vec<(LocalInputEvent, u64)> {
    static EVENTS: &str = include_str!("../../../events.obfuscated.log");
    read_logs(EVENTS.as_bytes())
        .map(|log| log.map(|log| (log.event, log.stamp)))
        .collect::<Result<_, _>>()
        .unwrap()
}

fn build_event_buf(events: &[(LocalInputEvent, u64)], event_buf: &mut EventBuffer<'_, u64>) {
    event_buf.clear();
    for event in events {
        event_buf.push_event(event.0, event.1);
    }
}

fn identical_keys_presses(event_buf: &EventBuffer<'_, u64>) -> u64 {
    let keys = event_buf.recent_pressed_keys(Some(&0));
    keys.fold((0, None), |(sum, prev), next| match prev {
        Some(prev) => {
            if prev == next {
                (sum + 1, Some(next))
            } else {
                (sum, Some(next))
            }
        }
        None => (sum, Some(next)),
    })
    .0
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let events = events();

    c.bench_function("build_event_buf", |b| {
        b.to_async(&rt).iter(|| async {
            build_event_buf(
                black_box(&events),
                black_box(&mut EventBuffer::new(|new, old| new - old > 300)),
            );
        });
    });

    c.bench_function("identical_keys_pressed", |b| {
        let mut event_buf = EventBuffer::new(|_, _| false);
        build_event_buf(&events, &mut event_buf);
        b.iter(|| identical_keys_presses(black_box(&event_buf)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
