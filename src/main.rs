use flate2::read::GzDecoder;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use std::{collections::HashSet, io::{Read, Write}, net::SocketAddrV4, sync::{Arc, LazyLock}, time::{Duration, Instant}};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream, sync::mpsc::{UnboundedSender, unbounded_channel}, time::sleep_until};

// Site de onde vem a lista de proxies
const SOURCE: &str = "free-proxy-list.net";
// Onde a lista baixada fica guardada
const CACHE: &str = "/tmp/free-proxy-list.txt";
// Regras para conferir se um site é quem diz ser
static CONFIG: LazyLock<Arc<ClientConfig>> = LazyLock::new(|| {
    let roots = RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    Arc::new(ClientConfig::builder().with_root_certificates(roots).with_no_client_auth())
});

#[tokio::main]
async fn main() {
    let start = Instant::now();
    // Baixa a lista nova ao mesmo tempo, sem esperar
    let (html_tx, mut html_rx) = unbounded_channel();
    std::thread::spawn(move || html_tx.send(fetch().unwrap_or_default()));
    // Por aqui chegam os proxies que funcionaram
    let (tx, mut rx) = unbounded_channel();
    // Proxies já testados, para não repetir
    let mut tested = HashSet::new();
    // Lista guardada com menos de 15 min: começa por ela
    if std::fs::metadata(CACHE).and_then(|m| m.modified()).is_ok_and(|t| t.elapsed().unwrap_or_default() < Duration::from_secs(15 * 60)) {
        test(&std::fs::read_to_string(CACHE).unwrap_or_default(), &mut tested, &tx);
    }
    // Sem lista guardada: espera a nova chegar
    if tested.is_empty() {
        test(&html_rx.recv().await.unwrap_or_default(), &mut tested, &tx);
    }

    // Os testes têm 1 segundo
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    // Do mais rápido ao mais lento
    let mut working = Vec::new();
    loop {
        tokio::select! {
            // Acabou o tempo
            _ = sleep_until(deadline) => break,
            // Um proxy funcionou: mostra uma vez só
            Some(proxy) = rx.recv() => if !working.contains(&proxy) {
                println!("{proxy}");
                working.push(proxy);
            },
            // A lista nova chegou: testa os que faltavam
            Some(html) = html_rx.recv() => test(&html, &mut tested, &tx),
        }
    }

    // Salva o resultado e mostra o resumo
    let json: Vec<String> = working.iter().map(ToString::to_string).collect();
    std::fs::write("working.json", format!("{json:?}\n")).unwrap();
    println!("{} funcionando de {} em {} ms → working.json", working.len(), tested.len(), start.elapsed().as_millis());
}

// Baixa a página com a lista de proxies
fn fetch() -> std::io::Result<String> {
    let tcp = std::net::TcpStream::connect((SOURCE, 443))?;
    let mut stream = StreamOwned::new(ClientConnection::new(CONFIG.clone(), SOURCE.try_into().unwrap()).unwrap(), tcp);
    // Pede a página compactada, que chega mais rápido
    stream.write_all(format!("GET / HTTP/1.0\r\nHost: {SOURCE}\r\nAccept-Encoding: gzip\r\n\r\n").as_bytes())?;
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    // Pula o cabeçalho, fica só o conteúdo
    let body = &response[response.windows(4).position(|w| w == b"\r\n\r\n").map_or(0, |i| i + 4)..];
    let mut html = String::new();
    GzDecoder::new(body).read_to_string(&mut html)?;
    // Guarda para a próxima vez
    std::fs::write(CACHE, &html)?;
    Ok(html)
}

// Acha os proxies no texto e testa os novos
fn test(text: &str, tested: &mut HashSet<SocketAddrV4>, tx: &UnboundedSender<SocketAddrV4>) {
    // Tudo que parece "1.2.3.4:8080"
    let proxies = text.split(|c: char| !c.is_ascii_digit() && c != '.' && c != ':').filter_map(|s| s.parse().ok());
    for proxy in proxies.filter(|&p| tested.insert(p)) {
        // 2 sites x 2 jeitos: basta um dar certo
        for target in ["example.com", "one.one.one.one"] {
            for pipelined in [false, true] {
                tokio::spawn(check(proxy, target, pipelined, tx.clone()));
            }
        }
    }
}

// O proxy leva até o site verdadeiro?
async fn check(proxy: SocketAddrV4, target: &'static str, pipelined: bool, tx: UnboundedSender<SocketAddrV4>) -> Option<()> {
    let mut stream = TcpStream::connect(proxy).await.ok()?;
    // Prepara o "olá" para o site
    let mut tls = ClientConnection::new(CONFIG.clone(), target.try_into().ok()?).ok()?;
    let mut hello = Vec::new();
    tls.write_tls(&mut hello).ok()?;
    // Pede ao proxy: "me liga a este site"
    let mut request = format!("CONNECT {target}:443 HTTP/1.1\r\nHost: {target}:443\r\n\r\n").into_bytes();
    // Jeito rápido: manda o "olá" junto
    if pipelined {
        request.extend(&hello);
    }
    stream.write_all(&request).await.ok()?;

    // Espera o proxy responder
    let mut data = Vec::new();
    let end = loop {
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if stream.read_buf(&mut data).await.ok()? == 0 {
            return None;
        }
    };
    // Jeito normal: manda o "olá" só agora
    if !pipelined {
        stream.write_all(&hello).await.ok()?;
    }

    // O que vem depois já é do site
    let mut data = data.split_off(end);
    loop {
        let mut rest = &data[..];
        while !rest.is_empty() {
            tls.read_tls(&mut rest).ok()?;
            tls.process_new_packets().ok()?;
        }
        // O site provou quem é: proxy aprovado
        if !tls.is_handshaking() {
            return tx.send(proxy).ok();
        }
        // Ainda não: espera mais resposta
        data.clear();
        if stream.read_buf(&mut data).await.ok()? == 0 {
            return None;
        }
    }
}
