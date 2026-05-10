use ethers::{
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, TransactionRequest, U256},
};
use std::sync::Arc;
use tracing::info;

const TOKEN_WPOL: &str = "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270";
const TOKEN_USDT: &str = "0xc2132D05D31c914a87C6611C10748AEb04B58e8F";

const UMBRAL_AVISO_USDT: u64 = 10_000_000;
const MIN_PROFIT_USDT: u64 = 1_000_000;
const PORCENTAJE_MONTO: u128 = 100;

struct ReservasPool {
    token0: Address,
    token1: Address,
    reserve0: U256,
    reserve1: U256,
}

pub async fn enviar_telegram(token: &str, chat_id: &str, mensaje: &str) {
    if token.is_empty() || chat_id.is_empty() {
        return;
    }

    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let cliente = reqwest::Client::new();
    let _ = cliente
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": mensaje
        }))
        .send()
        .await;
}

async fn llamar_address(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    contrato: Address,
    firma: &str,
) -> Option<Address> {
    let calldata = ethers::utils::keccak256(firma)[..4].to_vec();
    let tx = TransactionRequest::new().to(contrato).data(calldata);
    let resultado = cliente.call(&tx.into(), None).await.ok()?;

    if resultado.len() < 32 {
        return None;
    }

    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&resultado[12..32]);
    Some(Address::from(bytes))
}

async fn consultar_reservas(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    pool: &str,
) -> Option<ReservasPool> {
    let pool_addr: Address = pool.parse().ok()?;
    let token0 = llamar_address(cliente, pool_addr, "token0()").await?;
    let token1 = llamar_address(cliente, pool_addr, "token1()").await?;

    let selector = &ethers::utils::keccak256("getReserves()")[..4];
    let tx = TransactionRequest::new()
        .to(pool_addr)
        .data(selector.to_vec());
    let resultado = cliente.call(&tx.into(), None).await.ok()?;

    if resultado.len() < 64 {
        return None;
    }

    Some(ReservasPool {
        token0,
        token1,
        reserve0: U256::from_big_endian(&resultado[0..32]),
        reserve1: U256::from_big_endian(&resultado[32..64]),
    })
}

fn reserva_de_token(reservas: &ReservasPool, token: Address) -> Option<U256> {
    if reservas.token0 == token {
        Some(reservas.reserve0)
    } else if reservas.token1 == token {
        Some(reservas.reserve1)
    } else {
        None
    }
}

fn contiene_par_wpol_usdt(reservas: &ReservasPool, wpol: Address, usdt: Address) -> bool {
    (reservas.token0 == wpol && reservas.token1 == usdt)
        || (reservas.token0 == usdt && reservas.token1 == wpol)
}

fn calcular_monto(reserva_compra_wpol: U256, reserva_venta_wpol: U256) -> U256 {
    let reserva_minima = reserva_compra_wpol.min(reserva_venta_wpol);
    let monto = reserva_minima / U256::from(PORCENTAJE_MONTO);

    let minimo = U256::from(10u128) * U256::from(10u128).pow(U256::from(18u32));
    let maximo = U256::from(5_000u128) * U256::from(10u128).pow(U256::from(18u32));

    monto.max(minimo).min(maximo)
}

async fn saldo_usdt_en_contrato(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    contrato_addr: Address,
) -> U256 {
    let usdt_addr: Address = match TOKEN_USDT.parse() {
        Ok(a) => a,
        Err(_) => return U256::zero(),
    };

    let selector = &ethers::utils::keccak256("balanceOf(address)")[..4];
    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&ethers::abi::encode(&[ethers::abi::Token::Address(
        contrato_addr,
    )]));

    let tx = TransactionRequest::new().to(usdt_addr).data(calldata);

    match cliente.call(&tx.into(), None).await {
        Ok(bytes) if bytes.len() >= 32 => U256::from_big_endian(&bytes[..32]),
        _ => U256::zero(),
    }
}

pub async fn ejecutar_oportunidad(
    diferencia: f64,
    pool_compra: String,
    pool_venta: String,
    _precio: f64,
    wallet: &LocalWallet,
    rpc_mainnet: &str,
) {
    info!(
        "Coordinador activado - diferencia: {:.4}% - preparando arbitraje V2...",
        diferencia
    );

    let token_tg = std::env::var("TELEGRAM_TOKEN").unwrap_or_default();
    let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
    let contrato_arbitrage = match std::env::var("CONTRATO_ARBITRAGE") {
        Ok(valor) if !valor.is_empty() => valor,
        _ => {
            info!("CONTRATO_ARBITRAGE no configurado, abortando ejecucion");
            enviar_telegram(
                &token_tg,
                &chat_id,
                "Oportunidad detectada, pero falta CONTRATO_ARBITRAGE con el contrato corregido.",
            )
            .await;
            return;
        }
    };

    let proveedor = Provider::<Http>::try_from(rpc_mainnet).expect("Error conectando a Polygon");
    let wallet_mainnet = wallet.clone().with_chain_id(137u64);
    let cliente = Arc::new(SignerMiddleware::new(proveedor, wallet_mainnet));

    let wpol_addr: Address = TOKEN_WPOL.parse().expect("WPOL invalido");
    let usdt_addr: Address = TOKEN_USDT.parse().expect("USDT invalido");
    let contrato_addr: Address = contrato_arbitrage
        .parse()
        .expect("CONTRATO_ARBITRAGE invalido");

    info!("Consultando reservas del pool barato: {}", pool_compra);
    let reservas_compra = match consultar_reservas(&cliente, &pool_compra).await {
        Some(r) => r,
        None => {
            info!("No se pudieron obtener reservas del pool barato, abortando");
            return;
        }
    };

    info!("Consultando reservas del pool caro: {}", pool_venta);
    let reservas_venta = match consultar_reservas(&cliente, &pool_venta).await {
        Some(r) => r,
        None => {
            info!("No se pudieron obtener reservas del pool caro, abortando");
            return;
        }
    };

    if !contiene_par_wpol_usdt(&reservas_compra, wpol_addr, usdt_addr)
        || !contiene_par_wpol_usdt(&reservas_venta, wpol_addr, usdt_addr)
    {
        info!("Alguno de los pools no es WPOL/USDT V2, abortando");
        return;
    }

    let reserva_compra_wpol = match reserva_de_token(&reservas_compra, wpol_addr) {
        Some(r) => r,
        None => return,
    };
    let reserva_venta_wpol = match reserva_de_token(&reservas_venta, wpol_addr) {
        Some(r) => r,
        None => return,
    };

    let monto_entrada = calcular_monto(reserva_compra_wpol, reserva_venta_wpol);
    let monto_wpol = monto_entrada.as_u128() as f64 / 1e18;
    let min_profit = U256::from(MIN_PROFIT_USDT);

    info!(
        "Monto dinamico calculado: {:.2} WPOL - beneficio minimo: {:.2} USDT",
        monto_wpol,
        MIN_PROFIT_USDT as f64 / 1_000_000.0
    );

    let selector = &ethers::utils::keccak256(
        "ejecutarArbitraje(address,address,address,address,uint256,uint256)",
    )[..4];

    let pool_compra_addr: Address = pool_compra.parse().expect("Pool compra invalido");
    let pool_venta_addr: Address = pool_venta.parse().expect("Pool venta invalido");

    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&ethers::abi::encode(&[
        ethers::abi::Token::Address(pool_compra_addr),
        ethers::abi::Token::Address(pool_venta_addr),
        ethers::abi::Token::Address(wpol_addr),
        ethers::abi::Token::Address(usdt_addr),
        ethers::abi::Token::Uint(monto_entrada),
        ethers::abi::Token::Uint(min_profit),
    ]));

    let tx = TransactionRequest::new()
        .to(contrato_addr)
        .from(wallet.address())
        .data(calldata)
        .value(U256::zero());

    info!("Simulando transaccion con {:.2} WPOL...", monto_wpol);
    match cliente.call(&tx.clone().into(), None).await {
        Ok(_) => info!("Simulacion exitosa, enviando transaccion real..."),
        Err(e) => {
            info!("Simulacion fallida: {}", e);
            enviar_telegram(
                &token_tg,
                &chat_id,
                &format!(
                    "Oportunidad detectada pero la simulacion fallo\nDiferencia: {:.4}%\nMonto: {:.2} WPOL\nError: {}",
                    diferencia, monto_wpol, e
                ),
            )
            .await;
            return;
        }
    }

    match cliente.send_transaction(tx, None).await {
        Ok(tx_pendiente) => {
            let hash = format!("{:?}", tx_pendiente.tx_hash());
            info!("Transaccion enviada - hash: {}", hash);

            enviar_telegram(
                &token_tg,
                &chat_id,
                &format!(
                    "Arbitraje enviado\nDiferencia: {:.4}%\nMonto: {:.2} WPOL\nHash: {}\nhttps://polygonscan.com/tx/{}",
                    diferencia, monto_wpol, hash, hash
                ),
            )
            .await;

            match tx_pendiente.await {
                Ok(Some(recibo)) => {
                    info!("Transaccion confirmada en bloque {:?}", recibo.block_number)
                }
                Ok(None) => {
                    info!("Transaccion sin recibo todavia, se omite consulta de saldo");
                    return;
                }
                Err(e) => {
                    info!("Error esperando confirmacion: {}", e);
                    return;
                }
            }

            let saldo_contrato = saldo_usdt_en_contrato(&cliente, contrato_addr).await;
            let saldo_usdt = saldo_contrato.as_u64();
            let saldo_legible = saldo_usdt as f64 / 1_000_000.0;

            info!("USDT acumulado en contrato: {:.2}", saldo_legible);

            if saldo_usdt >= UMBRAL_AVISO_USDT {
                enviar_telegram(
                    &token_tg,
                    &chat_id,
                    &format!(
                        "Ganancia acumulada en contrato: {:.2} USDT. No se retiro automaticamente.",
                        saldo_legible
                    ),
                )
                .await;
            }
        }
        Err(e) => {
            info!("Error enviando transaccion: {}", e);
        }
    }
}

fn parse_usdt_env(nombre: &str) -> Option<U256> {
    let valor = std::env::var(nombre).ok()?;
    let normalizado = valor.trim().replace(',', ".");
    let mut partes = normalizado.split('.');
    let enteros = partes.next()?.parse::<u128>().ok()?;
    let decimales = partes.next().unwrap_or("");

    if partes.next().is_some() || decimales.len() > 6 {
        return None;
    }

    let mut decimales_padded = decimales.to_string();
    while decimales_padded.len() < 6 {
        decimales_padded.push('0');
    }

    let fraccion = if decimales_padded.is_empty() {
        0
    } else {
        decimales_padded.parse::<u128>().ok()?
    };

    Some(U256::from(enteros * 1_000_000 + fraccion))
}

pub async fn ejecutar_oportunidad_v3(
    diferencia: f64,
    pool_compra: String,
    pool_venta: String,
    _precio: f64,
    wallet: &LocalWallet,
    rpc_mainnet: &str,
) {
    info!(
        "Coordinador activado - diferencia: {:.4}% - preparando arbitraje V3...",
        diferencia
    );

    let token_tg = std::env::var("TELEGRAM_TOKEN").unwrap_or_default();
    let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();

    let contrato_arbitrage = match std::env::var("CONTRATO_ARBITRAGE") {
        Ok(valor) if !valor.is_empty() => valor,
        _ => {
            info!("CONTRATO_ARBITRAGE no configurado, abortando ejecucion V3");
            return;
        }
    };

    let pool_flash_v2 = match std::env::var("POOL_FLASH_V2") {
        Ok(valor) if !valor.is_empty() => valor,
        _ => {
            info!("POOL_FLASH_V2 no configurado, V3 queda solo monitorizado");
            enviar_telegram(
                &token_tg,
                &chat_id,
                "Oportunidad V3 detectada, pero falta POOL_FLASH_V2 para prestar USDT desde V2.",
            )
            .await;
            return;
        }
    };

    let monto_entrada = match parse_usdt_env("MONTO_V3_USDT") {
        Some(monto) if monto > U256::zero() => monto,
        _ => {
            info!("MONTO_V3_USDT no configurado o invalido, V3 queda solo monitorizado");
            enviar_telegram(
                &token_tg,
                &chat_id,
                "Oportunidad V3 detectada, pero falta MONTO_V3_USDT. Ejemplo: MONTO_V3_USDT=25",
            )
            .await;
            return;
        }
    };

    let proveedor = Provider::<Http>::try_from(rpc_mainnet).expect("Error conectando a Polygon");
    let wallet_mainnet = wallet.clone().with_chain_id(137u64);
    let cliente = Arc::new(SignerMiddleware::new(proveedor, wallet_mainnet));

    let contrato_addr: Address = contrato_arbitrage
        .parse()
        .expect("CONTRATO_ARBITRAGE invalido");
    let pool_flash_addr: Address = pool_flash_v2.parse().expect("POOL_FLASH_V2 invalido");
    let pool_compra_addr: Address = pool_compra.parse().expect("Pool compra V3 invalido");
    let pool_venta_addr: Address = pool_venta.parse().expect("Pool venta V3 invalido");
    let wpol_addr: Address = TOKEN_WPOL.parse().expect("WPOL invalido");
    let usdt_addr: Address = TOKEN_USDT.parse().expect("USDT invalido");
    let min_profit = U256::from(MIN_PROFIT_USDT);

    let selector = &ethers::utils::keccak256(
        "ejecutarArbitrajeV3(address,address,address,address,address,uint256,uint256)",
    )[..4];

    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&ethers::abi::encode(&[
        ethers::abi::Token::Address(pool_flash_addr),
        ethers::abi::Token::Address(pool_compra_addr),
        ethers::abi::Token::Address(pool_venta_addr),
        ethers::abi::Token::Address(usdt_addr),
        ethers::abi::Token::Address(wpol_addr),
        ethers::abi::Token::Uint(monto_entrada),
        ethers::abi::Token::Uint(min_profit),
    ]));

    let tx = TransactionRequest::new()
        .to(contrato_addr)
        .from(wallet.address())
        .data(calldata)
        .value(U256::zero());

    let monto_usdt = monto_entrada.as_u128() as f64 / 1_000_000.0;
    info!("Simulando arbitraje V3 con {:.2} USDT...", monto_usdt);

    match cliente.call(&tx.clone().into(), None).await {
        Ok(_) => info!("Simulacion V3 exitosa, enviando transaccion real..."),
        Err(e) => {
            info!("Simulacion V3 fallida: {}", e);
            enviar_telegram(
                &token_tg,
                &chat_id,
                &format!(
                    "Oportunidad V3 detectada pero la simulacion fallo\nDiferencia: {:.4}%\nMonto: {:.2} USDT\nError: {}",
                    diferencia, monto_usdt, e
                ),
            )
            .await;
            return;
        }
    }

    match cliente.send_transaction(tx, None).await {
        Ok(tx_pendiente) => {
            let hash = format!("{:?}", tx_pendiente.tx_hash());
            info!("Transaccion V3 enviada - hash: {}", hash);
            enviar_telegram(
                &token_tg,
                &chat_id,
                &format!(
                    "Arbitraje V3 enviado\nDiferencia: {:.4}%\nMonto: {:.2} USDT\nHash: {}\nhttps://polygonscan.com/tx/{}",
                    diferencia, monto_usdt, hash, hash
                ),
            )
            .await;
        }
        Err(e) => info!("Error enviando transaccion V3: {}", e),
    }
}
