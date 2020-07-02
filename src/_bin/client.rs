use async_std::{
    io::{stdin, BufReader},
    net::{TcpStream, ToSocketAddrs},
    prelude::*,
    task,
};
use futures::{select, FutureExt};
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

// main
fn main() -> Result<()> {
    task::spawn(f());
    loop {

    }
    Ok(())
}

async fn f() {
    for i in 0..1000 {
        try_run(format!("{}", i)).await;
    }
}

async fn try_run(addr: String) -> Result<()> {
    println!("Hello {}", addr);
    task::sleep(Duration::from_secs(1)).await;
    println!("Hello {}", addr);
    Ok(())
}