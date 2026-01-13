use std::io::{Read, Write};
use std::net::TcpStream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = std::env::args().nth(1).unwrap_or_else(|| "gpt-oss:20b".to_string());
    let prompt = "Count from 1 to 200";

    println!("Запрос к модели: {}", model);
    println!("Промпт: {}", prompt);
    println!("{}", "-".repeat(60));

    let mut stream = TcpStream::connect("localhost:11434")?;
    stream.set_nodelay(true)?;

    let request_body = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": prompt
        }],
        "stream": true
    });

    let body_str = serde_json::to_string(&request_body)?;
    let http_request = format!(
        "POST /api/chat HTTP/1.1\r\n\
         Host: localhost:11434\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        body_str.len(),
        body_str
    );

    stream.write_all(http_request.as_bytes())?;
    stream.flush()?;

    let mut buffer = [0u8; 8192];
    let mut response_buffer = String::new();

    loop {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        response_buffer.push_str(&String::from_utf8_lossy(&buffer[..n]));
        if response_buffer.contains("\r\n\r\n") {
            let parts: Vec<&str> = response_buffer.splitn(2, "\r\n\r\n").collect();
            response_buffer = parts[1].to_string();
            break;
        }
    }

    let mut previous_content = String::new();

    loop {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            break;
        }

        response_buffer.push_str(&String::from_utf8_lossy(&buffer[..n]));

        while let Some(newline_pos) = response_buffer.find('\n') {
            let line = response_buffer[..newline_pos].trim().to_string();
            response_buffer = response_buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if line.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }

            let chunk: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if let Some(message) = chunk.get("message") {
                if let Some(role) = message.get("role").and_then(|r| r.as_str()) {
                    if role == "assistant" {
                        if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                            if content != previous_content {
                                if content.starts_with(&previous_content) {
                                    let delta = &content[previous_content.len()..];
                                    print!("{}", delta);
                                    std::io::stdout().flush()?;
                                } else {
                                    print!("{}", content);
                                    std::io::stdout().flush()?;
                                }
                                previous_content = content.to_string();
                            }
                        }
                    }
                }
            }

            if chunk.get("done").and_then(|d| d.as_bool()).unwrap_or(false) {
                println!();
                return Ok(());
            }
        }
    }

    println!();
    Ok(())
}

