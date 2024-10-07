use crossterm::event::{Event, EventStream, KeyCode};
use futures::StreamExt;
use tokio::sync::mpsc::{Receiver, Sender};

pub fn listen() -> Receiver<KeyCode> {
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::task::spawn_local(poll_event_stream(tx));
    rx
}

async fn poll_event_stream(input_tx: Sender<KeyCode>) {
    let mut reader = EventStream::new();
    loop {
        match reader.next().await {
            Some(Ok(event)) => {
                if let Event::Key(key_ev) = event {
                    input_tx.send(key_ev.code).await.unwrap();
                }
            }
            Some(Err(e)) => panic!("input::poll_event_stream(): {e}"),
            None => {} // ??
        }
    }
}
