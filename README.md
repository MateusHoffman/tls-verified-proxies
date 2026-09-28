# tls-verified-proxies

Validador de proxies escrito em Rust. Baixa uma lista pública, testa centenas de proxies ao mesmo tempo e, em 1 segundo, devolve só os que realmente chegam até o site que você pediu.

## Por que isso existe

Você já mandou alguém buscar algo pra você? Isso é um proxy.

Em vez de o seu computador falar direto com um site, ele pede para outro computador ir lá e trazer a resposta.

É igual mandar um entregador buscar sua encomenda: quem aparece na loja é ele, não você.

Empresas fazem isso o tempo todo. Um comparador de preços consulta 50 lojas por segundo, se todos os pedidos saíssem do mesmo endereço, as lojas bloqueariam na hora.

Existem listas públicas com milhares de proxies, de graça, na internet, mas são proxies ruins e muitas nem funciona.

Com isso criei esse validador de proxies extremamente rápido, que processa centenas de proxies em apenas 1 segundo.

## Como funciona

Escrevi o validador em Rust.

Eu abro um túnel com CONNECT e faço o handshake TLS até o destino.

No TLS 1.3 o servidor assina o handshake com a chave privada dele. Proxy nenhum forja isso.

Fechou handshake, proxy aprovado. Nem precisa de resposta HTTP.

rustls no TLS, com a raiz de confiança compilada no binário. tokio na concorrência: 1.200 testes disparados juntos, deadline global de 1 segundo.

Cada proxy é testado de dois jeitos em paralelo, e basta um dar certo:

- **normal** — manda o `CONNECT`, espera a resposta do proxy, só então manda o ClientHello.
- **pipelined** — manda o `CONNECT` e o ClientHello de uma vez, economizando uma ida e volta.

## Como rodar

### 1. Instale o Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Compile uma vez

```bash
cargo build --release
```

### 3. Rode passando o site que os proxies precisam alcançar (obrigatório)

```bash
./target/release/proxy https://example.com.br
```

Sem porta na URL, vale 443. Para testar outra porta, escreva ela: `https://example.com.br:8443`.

## O que sai

No terminal, um proxy por linha, na ordem em que fecharam o handshake — ou seja, do mais rápido ao mais lento — e no fim o resumo:

```
20.197.203.93:8080
107.150.41.226:18080
45.76.68.10:9000
14 funcionando de 1173 até example.com.br:443 em 1043 ms → working.json
```

E o arquivo `working.json`, na pasta de onde você rodou, com a mesma lista pronta para outro programa consumir:

```json
["20.197.203.93:8080", "107.150.41.226:18080", "45.76.68.10:9000"]
```

## Detalhes

- **Fonte da lista:** `free-proxy-list.net`, baixada a cada execução.
- **Cache:** a página fica em `/tmp/free-proxy-list.txt` por 15 minutos. Tendo cache, os testes começam na hora enquanto a lista nova baixa em segundo plano — quando ela chega, os proxies inéditos entram no teste sem esperar a próxima rodada.
- **Prazo:** 1 segundo, fixo. Quem não fechou o handshake até lá fica de fora.
- **O alvo importa:** um proxy pode alcançar um site e ser bloqueado em outro. Valide sempre contra o site que você vai usar de verdade.
