use std::net::UdpSocket;
use qlib::stock_quote::StockQuote;

fn main() {
    let socket = UdpSocket::bind("127.0.0.1:34254").unwrap();
    println!("Ресивер запущен на {}", "127.0.0.1:34254");
    if let Err(e) = receive_loop(socket) {
        eprintln!("Ошибка в receive_loop: {}", e);
    }
}



    // Метод с циклом для получения метрик
    fn receive_loop(socket: UdpSocket) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0u8; 1024];

        println!("Ожидание данных...");

        loop {
            match socket.recv_from(&mut buf) {
                Ok((size, src_addr)) => match StockQuote::from_bytes(&buf[..size]) {
                    None => {}
                    Some(stock_quote) => {println!("{} \n {}",src_addr,stock_quote);}
                },
                Err(e) => {
                    eprintln!("Ошибка получения данных: {}", e);
                }
            }
        }
    }


