use qlib::stock_quote::StockQuote;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let socket = UdpSocket::bind("127.0.0.1:34254").unwrap();
    println!("Ресивер запущен на {}", "127.0.0.1:34254");
    if let Err(e) = receive_loop(socket) {
        eprintln!("Ошибка в receive_loop: {}", e);
    }
}

fn receive_loop(socket: UdpSocket) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = [0u8; 1024];
    let mut is_pinger_active = false;
    let socket = Arc::new(socket);
    println!("Ожидание данных...");

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, src_addr)) => match StockQuote::from_bytes(&buf[..size]) {
                Some(stock_quote) => {
                    if !is_pinger_active {
                        is_pinger_active = true;
                        let ping_socket = socket.clone();
                        thread::spawn(move || {
                            if let Err(e) = ping(ping_socket, src_addr) {
                                eprintln!("Ошибка пинга: {}", e);
                            }
                        });
                    }
                    println!("{} \n {}", src_addr, stock_quote);
                }
                None => {}
            },
            Err(e) => {
                eprintln!("Ошибка получения данных: {}", e);
            }
        }
    }
}

fn ping(socket: Arc<UdpSocket>, src_adr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let bytes = "PING".as_bytes();
        socket.send_to(bytes, &src_adr)?;
        thread::sleep(Duration::from_millis(2000));
    }
}