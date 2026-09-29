# tls-verified-proxies

Validador de proxies escrito em Rust. Baixa uma lista pública, testa centenas de proxies ao mesmo tempo e devolve **um** proxy: o mais rápido que realmente chega até o site que você pediu.

## Por que isso existe

Você já mandou alguém buscar algo pra você? Isso é um proxy.

Em vez de o seu computador falar direto com um site, ele pede para outro computador ir lá e trazer a resposta.

É igual mandar um entregador buscar sua encomenda: quem aparece na loja é ele, não você.

Empresas fazem isso o tempo todo. Um comparador de preços consulta 50 lojas por segundo, se todos os pedidos saíssem do mesmo endereço, as lojas bloqueariam na hora.

Existem listas públicas com milhares de proxies, de graça, na internet, mas são proxies ruins e muitas nem funciona.

Com isso criei esse validador de proxies extremamente rápido: ele dispara centenas de testes juntos e te entrega o primeiro que responder — que é, por definição, o mais rápido.

## Como funciona

Escrevi o validador em Rust.

Eu abro um túnel com CONNECT e faço o handshake TLS até o destino.

No TLS 1.3 o servidor assina o handshake com a chave privada dele. Proxy nenhum forja isso.

Fechou handshake, proxy aprovado. Nem precisa de resposta HTTP.

O primeiro a fechar o handshake é o mais rápido de todos. Então é ele que sai, e o programa encerra na hora — sem esperar o resto.

rustls no TLS, com a raiz de confiança compilada no binário. tokio na concorrência: centenas de testes disparados juntos, limite de 1 segundo para alguém chegar.

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

Uma linha só na saída padrão: o proxy mais rápido. O resumo vai para a saída de erro, então não suja o resultado.

```
$ ./target/release/proxy https://dashskins.com.br
213.111.146.36:18080
mais rápido de 300 testados até dashskins.com.br:443 em 471 ms
```

Como a saída padrão tem só o endereço, dá para usar direto:

```bash
PROXY=$(./target/release/proxy https://dashskins.com.br) && curl -x "http://$PROXY" https://dashskins.com.br
```

O código de saída diz se deu certo: `0` achou, `1` ninguém chegou no prazo, `2` a URL está errada.

```
$ ./target/release/proxy https://example.com.br
nenhum dos 300 proxies alcançou example.com.br:443 em 1002 ms
```

## Detalhes

- **Fonte da lista:** `free-proxy-list.net`, baixada a cada execução.
- **Cache:** a página fica em `/tmp/free-proxy-list.txt` por 15 minutos. Tendo cache, os testes começam na hora enquanto a lista nova baixa em segundo plano — quando ela chega, os proxies inéditos entram no teste sem esperar a próxima rodada.
- **Prazo:** no máximo 1 segundo. Normalmente termina bem antes, porque encerra no primeiro que fecha o handshake — nos testes, entre 300 e 500 ms.
- **O alvo importa:** um proxy pode alcançar um site e ser bloqueado em outro. Valide sempre contra o site que você vai usar de verdade.
