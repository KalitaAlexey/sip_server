use async_std::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    prelude::*,
    task,
};
use futures::{channel::mpsc, sink::SinkExt};
use libsip::{Header, Method, ResponseGenerator, SipMessage};
use nom::error::VerboseError;
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

enum BrokerMsg {
    NewClient,
}

fn main() -> Result<()> {
    task::block_on(run_server("127.0.0.1:8080"))
}

async fn run_server(addr: impl ToSocketAddrs) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let (broker_sender, broker_receiver) = mpsc::unbounded();
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        println!("Accepting from: {}", stream.peer_addr()?);
        spawn_and_log_error(work_with_connection(stream, broker_sender.clone()));
    }
    println!("Shutting down");
    Ok(())
}

async fn run_broker(receiver: Receiver<BrokerMsg>) -> Result<()> {
    Ok(())
}

async fn work_with_connection(mut stream: TcpStream, mut broker: Sender<BrokerMsg>) -> Result<()> {
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
                }
                _ => continue,
            };
            println!(">>>>>>> Content: {:?}", m);
            let res = ResponseGenerator::new()
                .code(200)
                .headers(
                    m.1.headers()
                        .iter()
                        .map(|h| {
                            let mut h = h.clone();
                            if let Header::To(ref mut h) = h {
                                h.params.insert("branch".to_string(), "123456".to_string());
                            };
                            h
                        })
                        .collect(),
                )
                .build()
                .expect("failed to generate response");
            let res = res.to_string();
            println!("Response: {}", res);
            stream.write_all(res.as_bytes()).await?;
            if let SipMessage::Request { method, .. } = m.1 {
                if method == Method::Subscribe {
                    println!("Waiting");
                    // task::sleep(Duration::from_secs(20)).await;
                }
            }
        }
    }
    broker.send(BrokerMsg::NewClient).await?;
    Ok(())
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
