use ethers::{
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{TransactionRequest, U256},
};
use std::sync::Arc;
use tracing::info;

// Tokens del par WPOL/USDT en Polygon
const TOKEN_WPOL: &str = "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270";
const TOKEN_USDT: &str = "0xc2132D05D31c914a87C6611C10748AEb04B58e8F";

// Contrato FlashSwapArbitrage desplegado en Polygon mainnet
const CONTRATO_ARBITRAGE: &str = "0x99822a9C9A22DB1F3a7ABa5a996d04314435492f";

// Dirección de MetaMask para recibir ganancias
const WALLET_DESTINO: &str = "0x15a9361ECFC5552eE2040aE72eB7B2402b646E65";

// Umbral mínimo para retirar: 10 USDT (6 decimales)
const UMBRAL_RETIRO_USDT: u64 = 10_000_000;

// Porcentaje de reservas a usar como monto (1%)
const PORCENTAJE_MONTO: u128 = 100; // divisor: reserva / 100 = 1%

// ─────────────────────────────────────────────
// Envía un mensaje de texto al bot de Telegram
// ─────────────────────────────────────────────
pub async fn enviar_telegram(token: &str, chat_id: &str, mensaje: &str) {
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

// ─────────────────────────────────────────────
// Consulta las reservas reales de un pool V2
// Devuelve (reserve0, reserve1) o None si falla
// ─────────────────────────────────────────────
async fn consultar_reservas(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    pool: &str,
) -> Option<(U256, U256)> {
    // Selector de getReserves() — función estándar UniswapV2Pair
    let selector = &ethers::utils::keccak256("getReserves()")[..4];
    let calldata = selector.to_vec();

    let pool_addr: ethers::types::Address = pool.parse().ok()?;

    let tx = TransactionRequest::new().to(pool_addr).data(calldata);

    let resultado = cliente.call(&tx.into(), None).await.ok()?;

    // getReserves devuelve (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
    // Cada valor ocupa 32 bytes en el ABI encoding
    if resultado.len() < 64 {
        return None;
    }

    let reserve0 = U256::from_big_endian(&resultado[0..32]);
    let reserve1 = U256::from_big_endian(&resultado[32..64]);

    Some((reserve0, reserve1))
}

// ─────────────────────────────────────────────
// Calcula el monto dinámico basado en reservas
// Usa el 1% de la reserva mínima de WPOL
// ─────────────────────────────────────────────
fn calcular_monto(reserva_a: U256, reserva_b: U256) -> U256 {
    // Tomamos la reserva más pequeña para no impactar el pool con menos liquidez
    let reserva_minima = reserva_a.min(reserva_b);

    // 1% de la reserva mínima
    let monto = reserva_minima / U256::from(PORCENTAJE_MONTO);

    // Mínimo 10 WPOL, máximo 5000 WPOL para no mover demasiado el precio
    let minimo = U256::from(10u128) * U256::from(10u128).pow(U256::from(18u32));
    let maximo = U256::from(5_000u128) * U256::from(10u128).pow(U256::from(18u32));

    monto.max(minimo).min(maximo)
}

// ─────────────────────────────────────────────
// Consulta el saldo USDT acumulado en el contrato
// Devuelve el saldo o 0 si falla
// ─────────────────────────────────────────────
async fn saldo_usdt_en_contrato(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    contrato_addr: ethers::types::Address,
) -> U256 {
    let usdt_addr: ethers::types::Address = match TOKEN_USDT.parse() {
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

// ─────────────────────────────────────────────
// Punto de entrada principal del coordinador
// ─────────────────────────────────────────────
pub async fn ejecutar_oportunidad(
    diferencia: f64,
    pool_compra: String,
    pool_venta: String,
    _precio: f64,
    wallet: &LocalWallet,
    rpc_mainnet: &str,
) {
    info!(
        "⚙️  Coordinador activado — diferencia: {:.4}% — ejecutando arbitraje...",
        diferencia
    );

    let token_tg = std::env::var("TELEGRAM_TOKEN").unwrap_or_default();
    let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();

    // Conectar a Polygon mainnet
    let proveedor = Provider::<Http>::try_from(rpc_mainnet).expect("Error conectando a Polygon");
    let wallet_mainnet = wallet.clone().with_chain_id(137u64);
    let cliente = Arc::new(SignerMiddleware::new(proveedor, wallet_mainnet));

    // ── BLOQUE 1: Consultar reservas reales de ambos pools ──────────────────
    info!(
        "📊 Consultando reservas del pool de compra: {}",
        pool_compra
    );
    let reservas_compra = match consultar_reservas(&cliente, &pool_compra).await {
        Some(r) => r,
        None => {
            info!("❌ No se pudieron obtener reservas del pool de compra, abortando");
            return;
        }
    };

    info!("📊 Consultando reservas del pool de venta: {}", pool_venta);
    let reservas_venta = match consultar_reservas(&cliente, &pool_venta).await {
        Some(r) => r,
        None => {
            info!("❌ No se pudieron obtener reservas del pool de venta, abortando");
            return;
        }
    };

    info!(
        "📊 Reservas pool compra — reserve0: {} — reserve1: {}",
        reservas_compra.0, reservas_compra.1
    );
    info!(
        "📊 Reservas pool venta  — reserve0: {} — reserve1: {}",
        reservas_venta.0, reservas_venta.1
    );

    // ── BLOQUE 2: Calcular monto dinámico basado en reservas ────────────────
    // reserve0 es WPOL (token0 del par), usamos esas reservas para el cálculo
    let monto_entrada = calcular_monto(reservas_compra.0, reservas_venta.0);

    let monto_wpol = monto_entrada.as_u128() as f64 / 1e18;
    info!("💡 Monto dinámico calculado: {:.2} WPOL", monto_wpol);

    // Construir calldata con el monto dinámico
    let selector =
        &ethers::utils::keccak256("ejecutarArbitraje(address,address,address,uint256)")[..4];

    let pool_compra_addr: ethers::types::Address =
        pool_compra.parse().expect("Pool compra inválido");
    let pool_venta_addr: ethers::types::Address = pool_venta.parse().expect("Pool venta inválido");
    let token_prestamo_addr: ethers::types::Address = TOKEN_WPOL.parse().expect("Token inválido");
    let contrato_addr: ethers::types::Address =
        CONTRATO_ARBITRAGE.parse().expect("Contrato inválido");

    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&ethers::abi::encode(&[
        ethers::abi::Token::Address(pool_compra_addr),
        ethers::abi::Token::Address(pool_venta_addr),
        ethers::abi::Token::Address(token_prestamo_addr),
        ethers::abi::Token::Uint(monto_entrada),
    ]));

    let tx = TransactionRequest::new()
        .to(contrato_addr)
        .data(calldata)
        .value(U256::zero());

    // Simular antes de enviar
    info!("🧪 Simulando transacción con {:.2} WPOL...", monto_wpol);
    match cliente.call(&tx.clone().into(), None).await {
        Ok(_) => info!("✅ Simulación exitosa, enviando..."),
        Err(e) => {
            info!("❌ Simulación falló: {}", e);
            enviar_telegram(&token_tg, &chat_id, &format!(
                "⚠️ Oportunidad detectada pero simulación falló\nDiferencia: {:.4}%\nMonto: {:.2} WPOL\nError: {}",
                diferencia, monto_wpol, e
            )).await;
            return;
        }
    }

    info!("✍️  Firmando y enviando a Polygon mainnet...");
    match cliente.send_transaction(tx, None).await {
        Ok(tx_pendiente) => {
            let hash = format!("{:?}", tx_pendiente.tx_hash());
            info!("✅ Transacción enviada — hash: {}", hash);
            info!("🔗 https://polygonscan.com/tx/{}", hash);

            enviar_telegram(&token_tg, &chat_id, &format!(
                "🚨 Arbitraje ejecutado!\nDiferencia: {:.4}%\nMonto: {:.2} WPOL\nHash: {}\nhttps://polygonscan.com/tx/{}",
                diferencia, monto_wpol, hash, hash
            )).await;

            // ── BLOQUE 3: Umbral de retiro — solo retirar si hay ≥ 10 USDT ─────
            info!("💰 Consultando USDT acumulado en el contrato...");
            let saldo_contrato = saldo_usdt_en_contrato(&cliente, contrato_addr).await;
            let saldo_usdt = saldo_contrato.as_u64();
            let saldo_legible = saldo_usdt as f64 / 1_000_000.0;

            info!("💰 USDT en contrato: {:.2} USDT", saldo_legible);

            if saldo_usdt < UMBRAL_RETIRO_USDT {
                // Aún no llegamos al umbral, acumular
                let pendiente = (UMBRAL_RETIRO_USDT - saldo_usdt) as f64 / 1_000_000.0;
                info!(
                    "⏳ Acumulando... {:.2}/10 USDT — faltan {:.2} USDT para retirar",
                    saldo_legible, pendiente
                );
                enviar_telegram(&token_tg, &chat_id, &format!(
                    "⏳ Ganancia acumulada en contrato: {:.2} USDT\nFaltan {:.2} USDT para retirar automáticamente",
                    saldo_legible, pendiente
                )).await;
                return;
            }

            // Umbral alcanzado — proceder con el retiro
            info!(
                "✅ Umbral alcanzado ({:.2} USDT) — retirando al bot...",
                saldo_legible
            );

            let selector_retirar = &ethers::utils::keccak256("retirar(address)")[..4];
            let usdt_addr: ethers::types::Address = TOKEN_USDT.parse().expect("USDT inválido");

            let mut calldata_retirar = selector_retirar.to_vec();
            calldata_retirar.extend_from_slice(&ethers::abi::encode(&[
                ethers::abi::Token::Address(usdt_addr),
            ]));

            let tx_retirar = TransactionRequest::new()
                .to(contrato_addr)
                .data(calldata_retirar)
                .value(U256::zero());

            match cliente.send_transaction(tx_retirar, None).await {
                Ok(tx_ret) => {
                    let hash_ret = format!("{:?}", tx_ret.tx_hash());
                    info!("✅ Ganancia retirada del contrato — hash: {}", hash_ret);

                    // Transferir todo el USDT de la wallet del bot a MetaMask
                    info!("📤 Transfiriendo USDT a MetaMask...");
                    let destino: ethers::types::Address =
                        WALLET_DESTINO.parse().expect("Destino inválido");

                    // Consultar saldo USDT en la wallet del bot (después del retiro)
                    let selector_balance = &ethers::utils::keccak256("balanceOf(address)")[..4];
                    let mut calldata_balance = selector_balance.to_vec();
                    calldata_balance.extend_from_slice(&ethers::abi::encode(&[
                        ethers::abi::Token::Address(wallet.address()),
                    ]));

                    let tx_balance = TransactionRequest::new()
                        .to(usdt_addr)
                        .data(calldata_balance);

                    if let Ok(saldo_bytes) = cliente.call(&tx_balance.into(), None).await {
                        let saldo_bot = if saldo_bytes.len() >= 32 {
                            U256::from_big_endian(&saldo_bytes[..32])
                        } else {
                            U256::zero()
                        };

                        if saldo_bot > U256::zero() {
                            let selector_transfer =
                                &ethers::utils::keccak256("transfer(address,uint256)")[..4];
                            let mut calldata_transfer = selector_transfer.to_vec();
                            calldata_transfer.extend_from_slice(&ethers::abi::encode(&[
                                ethers::abi::Token::Address(destino),
                                ethers::abi::Token::Uint(saldo_bot),
                            ]));

                            let tx_transfer = TransactionRequest::new()
                                .to(usdt_addr)
                                .data(calldata_transfer)
                                .value(U256::zero());

                            match cliente.send_transaction(tx_transfer, None).await {
                                Ok(tx_t) => {
                                    let hash_t = format!("{:?}", tx_t.tx_hash());
                                    let enviado = saldo_bot.as_u64() as f64 / 1_000_000.0;
                                    info!(
                                        "✅ {:.2} USDT enviados a MetaMask — hash: {}",
                                        enviado, hash_t
                                    );
                                    enviar_telegram(&token_tg, &chat_id, &format!(
                                        "💸 {:.2} USDT enviados a tu MetaMask!\nhttps://polygonscan.com/tx/{}",
                                        enviado, hash_t
                                    )).await;
                                }
                                Err(e) => info!("❌ Error transfiriendo a MetaMask: {}", e),
                            }
                        } else {
                            info!("⚠️ Saldo en wallet del bot es 0 después del retiro");
                        }
                    }
                }
                Err(e) => info!("❌ Error retirando del contrato: {}", e),
            }
        }
        Err(e) => {
            info!("❌ Error enviando: {}", e);
        }
    }
}
