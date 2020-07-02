use async_std::{
    io::{BufReader, Read},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    prelude::*,
    task,
};
use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::{select, FutureExt};
use nom::error::VerboseError;
use std::{
    collections::hash_map::{Entry, HashMap},
    future::Future,
    sync::Arc,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

#[derive(Debug)]
enum Void {}

// main
fn main() -> Result<()> {
    task::block_on(accept_loop("127.0.0.1:8080"))
}

async fn accept_loop(addr: impl ToSocketAddrs) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let (broker_sender, broker_receiver) = mpsc::unbounded();
    let broker_handle = task::spawn(broker_loop(broker_receiver));
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        println!("Accepting from: {}", stream.peer_addr()?);
        spawn_and_log_error(connection_loop(broker_sender.clone(), stream));
    }
    drop(broker_sender);
    broker_handle.await;
    Ok(())
}

async fn connection_loop(mut broker: Sender<Event>, mut stream: TcpStream) -> Result<()> {
    // let stream = Arc::new(stream);
    // let mut reader = BufReader::new(stream);
    let mut buffer = [0; 4096];
    loop {
        let n = stream.read(&mut buffer).await?;
        if n > 0 {
            println!("Message: {}", String::from_utf8_lossy(&buffer[..n]));
            let m = match libsip::parse_message::<VerboseError<&[u8]>>(&buffer[..n]) {
                Ok(m) => m,
                Err(nom::Err::Error(e)) => {
                    let s = String::from_utf8_lossy(&e.errors[0].0);
                    eprintln!("Errors: {:?}, Kind: {:?}", s, e.errors[0].1);
                    continue;
                },
                _ => continue,
            };
            println!(">>>>>>> Content: {:?}", m);
        }
    }
    // let mut lines = reader.lines();

    // let name = match lines.next().await {
    //     None => Err("peer disconnected immediately")?,
    //     Some(line) => { let line = line?; println!("Line: {}", line); line }
    // };
    // let (_shutdown_sender, shutdown_receiver) = mpsc::unbounded::<Void>();
    // broker
    //     .send(Event::NewPeer {
    //         name: name.clone(),
    //         stream: Arc::clone(&stream),
    //         shutdown: shutdown_receiver,
    //     })
    //     .await
    //     .unwrap();

    // while let Some(line) = lines.next().await {
    //     let line = line?;
    //     let (dest, msg) = match line.find(':') {
    //         None => continue,
    //         Some(idx) => (&line[..idx], line[idx + 1..].trim()),
    //     };
    //     let dest: Vec<String> = dest
    //         .split(',')
    //         .map(|name| name.trim().to_string())
    //         .collect();
    //     let msg: String = msg.trim().to_string();

    //     broker
    //         .send(Event::Message {
    //             from: name.clone(),
    //             to: dest,
    //             msg,
    //         })
    //         .await
    //         .unwrap();
    // }

    Ok(())
}

async fn connection_writer_loop(
    messages: &mut Receiver<String>,
    stream: Arc<TcpStream>,
    shutdown: Receiver<Void>,
) -> Result<()> {
    let mut stream: &TcpStream = &*stream;
    let mut messages = messages.fuse();
    let mut shutdown = shutdown.fuse();
    loop {
        select! {
            msg = messages.next().fuse() => match msg {
                Some(msg) => stream.write_all(msg.as_bytes()).await?,
                None => break,
            },
            void = shutdown.next().fuse() => match void {
                Some(void) => match void {},
                None => break,
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
enum Event {
    NewPeer {
        name: String,
        stream: Arc<TcpStream>,
        shutdown: Receiver<Void>,
    },
    Message {
        from: String,
        to: Vec<String>,
        msg: String,
    },
}

async fn broker_loop(events: Receiver<Event>) {
    let (disconnect_sender, mut disconnect_receiver) = // 1
        mpsc::unbounded::<(String, Receiver<String>)>();
    let mut peers: HashMap<String, Sender<String>> = HashMap::new();
    let mut events = events.fuse();
    loop {
        let event = select! {
            event = events.next().fuse() => match event {
                None => break, // 2
                Some(event) => event,
            },
            disconnect = disconnect_receiver.next().fuse() => {
                let (name, _pending_messages) = disconnect.unwrap(); // 3
                assert!(peers.remove(&name).is_some());
                continue;
            },
        };
        match event {
            Event::Message { from, to, msg } => {
                for addr in to {
                    if let Some(peer) = peers.get_mut(&addr) {
                        let msg = format!("from {}: {}\n", from, msg);
                        peer.send(msg).await.unwrap() // 6
                    }
                }
            }
            Event::NewPeer {
                name,
                stream,
                shutdown,
            } => {
                match peers.entry(name.clone()) {
                    Entry::Occupied(..) => (),
                    Entry::Vacant(entry) => {
                        println!("{} has joined", name);
                        let (client_sender, mut client_receiver) = mpsc::unbounded();
                        entry.insert(client_sender);
                        let mut disconnect_sender = disconnect_sender.clone();
                        spawn_and_log_error(async move {
                            let res =
                                connection_writer_loop(&mut client_receiver, stream, shutdown)
                                    .await;
                            disconnect_sender
                                .send((name, client_receiver))
                                .await // 4
                                .unwrap();
                            res
                        });
                    }
                }
            }
        }
    }
    drop(peers); // 5
    drop(disconnect_sender); // 6
    while let Some((_name, _pending_messages)) = disconnect_receiver.next().await {}
}

fn spawn_and_log_error<F>(fut: F) -> task::JoinHandle<()>
where
    F: Future<Output = Result<()>> + Send + 'static,
{
    task::spawn(async move {
        if let Err(e) = fut.await {
            eprintln!("{}", e)
        }
    })
}
