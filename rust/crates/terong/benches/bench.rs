use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use terong::event_buffer::EventBuffer;

fn build_event_buf(event_buf: &mut EventBuffer<'_, u32>) {
    event_buf.clear();
}

fn identical_keys_pressed(event_buf: &EventBuffer<'_, u32>) -> bool {
    let mut keys = event_buf.recent_pressed_keys(Some(&0));
    keys.next() == keys.next()
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut event_buf = EventBuffer::new(|new, old| new - old > 300);

    c.bench_function("build_event_buf", |b| {
        b.iter(|| build_event_buf(black_box(&mut event_buf)));
    });

    c.bench_function("identical_keys_pressed", |b| {
        b.iter(|| identical_keys_pressed(black_box(&event_buf)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
