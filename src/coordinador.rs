use crate::pools::{
    POOLS_WPOL_USDT, RutaTriangularV2Generada, TOKEN_USDT, TOKEN_WPOL, rutas_triangulares_v2,
};
use ethers::{
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, TransactionRequest, U256},
};
use futures_util::future::join_all;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::time;
use tracing::{debug, info};

const UMBRAL_AVISO_USDT: u64 = 10_000_000;
const MIN_PROFIT_USDT_DEFECTO: u64 = 5_000_000;
const MULTIPLICADOR_MARGEN_LOCAL_DEFECTO: u64 = 2;
const ESCANER_V2_SEGUNDOS_DEFECTO: u64 = 5;
const ESCANER_V2_COOLDOWN_SEGUNDOS_DEFECTO: u64 = 60;
const ESCANER_TRIANGULAR_SEGUNDOS_DEFECTO: u64 = 10;
const ESCANER_TRIANGULAR_COOLDOWN_SEGUNDOS_DEFECTO: u64 = 120;
const GAS_TRIANGULAR_V2_DEFECTO: u64 = 450_000;

struct ReservasPool {
    token0: Address,
    token1: Address,
    reserve0: U256,
    reserve1: U256,
}

struct ResultadoArbitrajeV2 {
    monto_prestamo: U256,
    deuda_usdt: U256,
    salida_usdt: U256,
    beneficio_usdt: U256,
}

struct CandidatoArbitrajeV2 {
    par: String,
    pool_compra: String,
    pool_venta: String,
    token_base: Address,
    token_cotizacion: Address,
    simbolo_base: String,
    simbolo_cotizacion: String,
    decimales_base: u32,
    decimales_cotizacion: u32,
    diferencia: f64,
    precio_compra: f64,
    precio_venta: f64,
    resultado: ResultadoArbitrajeV2,
    min_profit: U256,
}

struct EvaluacionGas {
    gas_limit: U256,
    gas_price: U256,
    coste_usdt: f64,
}

struct ResultadoTriangularV2 {
    monto_inicial: U256,
    monto_final: U256,
    bruto: U256,
}

struct CandidatoTriangularV2 {
    resultado: ResultadoTriangularV2,
    gas: EvaluacionGas,
    neto: f64,
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

fn contiene_par_tokens(reservas: &ReservasPool, token_a: Address, token_b: Address) -> bool {
    (reservas.token0 == token_a && reservas.token1 == token_b)
        || (reservas.token0 == token_b && reservas.token1 == token_a)
}

fn reservas_ordenadas(
    reservas: &ReservasPool,
    token_in: Address,
    token_out: Address,
) -> Option<(U256, U256)> {
    if reservas.token0 == token_in && reservas.token1 == token_out {
        Some((reservas.reserve0, reservas.reserve1))
    } else if reservas.token1 == token_in && reservas.token0 == token_out {
        Some((reservas.reserve1, reservas.reserve0))
    } else {
        None
    }
}

fn get_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256) -> Option<U256> {
    if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
        return None;
    }

    let amount_in_with_fee = amount_in * U256::from(997u64);
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in * U256::from(1000u64) + amount_in_with_fee;

    if denominator.is_zero() {
        None
    } else {
        Some(numerator / denominator)
    }
}

fn get_amount_in(amount_out: U256, reserve_in: U256, reserve_out: U256) -> Option<U256> {
    if amount_out.is_zero() || reserve_in.is_zero() || reserve_out <= amount_out {
        return None;
    }

    let numerator = reserve_in * amount_out * U256::from(1000u64);
    let denominator = (reserve_out - amount_out) * U256::from(997u64);

    if denominator.is_zero() {
        None
    } else {
        Some(numerator / denominator + U256::one())
    }
}

fn simular_arbitraje_v2(
    monto_prestamo: U256,
    reservas_compra: &ReservasPool,
    reservas_venta: &ReservasPool,
    wpol: Address,
    usdt: Address,
) -> Option<ResultadoArbitrajeV2> {
    let (reserva_deuda_usdt, reserva_prestamo_wpol) =
        reservas_ordenadas(reservas_compra, usdt, wpol)?;
    let (reserva_venta_wpol, reserva_salida_usdt) = reservas_ordenadas(reservas_venta, wpol, usdt)?;

    let deuda_usdt = get_amount_in(monto_prestamo, reserva_deuda_usdt, reserva_prestamo_wpol)?;
    let salida_usdt = get_amount_out(monto_prestamo, reserva_venta_wpol, reserva_salida_usdt)?;
    let beneficio_usdt = salida_usdt.checked_sub(deuda_usdt)?;

    Some(ResultadoArbitrajeV2 {
        monto_prestamo,
        deuda_usdt,
        salida_usdt,
        beneficio_usdt,
    })
}

fn calcular_mejor_monto_v2(
    reservas_compra: &ReservasPool,
    reservas_venta: &ReservasPool,
    wpol: Address,
    usdt: Address,
    min_profit: U256,
) -> Option<ResultadoArbitrajeV2> {
    let reserva_compra_wpol = reserva_de_token(reservas_compra, wpol)?;
    let reserva_venta_wpol = reserva_de_token(reservas_venta, wpol)?;
    let reserva_minima = reserva_compra_wpol.min(reserva_venta_wpol);

    let unidad_wpol = U256::from(10u128).pow(U256::from(18u32));
    let minimo = unidad_wpol;
    let maximo = U256::from(5_000u128) * U256::from(10u128).pow(U256::from(18u32));
    let limite = maximo.min(reserva_minima / U256::from(20u64));

    if limite < minimo {
        return None;
    }

    let mut mejor: Option<ResultadoArbitrajeV2> = None;
    let pasos = U256::from(80u64);
    let incremento = (limite - minimo) / pasos;

    for i in 0..=80u64 {
        let monto = if i == 80 {
            limite
        } else {
            minimo + incremento * U256::from(i)
        };

        let Some(resultado) =
            simular_arbitraje_v2(monto, reservas_compra, reservas_venta, wpol, usdt)
        else {
            continue;
        };

        if resultado.beneficio_usdt <= min_profit {
            continue;
        }

        if mejor.as_ref().map_or(true, |actual| {
            resultado.beneficio_usdt > actual.beneficio_usdt
        }) {
            mejor = Some(resultado);
        }
    }

    mejor
}

fn min_profit_usdt() -> U256 {
    min_profit_token(6)
}

fn min_profit_token(decimales: u32) -> U256 {
    parse_token_env("MIN_PROFIT_USDT", decimales)
        .unwrap_or_else(|| escalar_unidades(MIN_PROFIT_USDT_DEFECTO, 6, decimales))
}

fn multiplicador_margen_local() -> u64 {
    std::env::var("MULTIPLICADOR_MARGEN_LOCAL")
        .ok()
        .and_then(|valor| valor.trim().parse::<u64>().ok())
        .filter(|valor| *valor > 0)
        .unwrap_or(MULTIPLICADOR_MARGEN_LOCAL_DEFECTO)
}

fn usdt_legible(valor: U256) -> f64 {
    token_legible(valor, 6)
}

fn token_legible(valor: U256, decimales: u32) -> f64 {
    valor.as_u128() as f64 / 10f64.powi(decimales as i32)
}

fn env_u64(nombre: &str, defecto: u64) -> u64 {
    std::env::var(nombre)
        .ok()
        .and_then(|valor| valor.trim().parse::<u64>().ok())
        .filter(|valor| *valor > 0)
        .unwrap_or(defecto)
}

fn env_bool(nombre: &str, defecto: bool) -> bool {
    std::env::var(nombre)
        .ok()
        .map(|valor| {
            matches!(
                valor.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "si" | "sí" | "yes" | "on"
            )
        })
        .unwrap_or(defecto)
}

fn precio_base_cotizacion(
    reservas: &ReservasPool,
    token_base: Address,
    token_cotizacion: Address,
    decimales_base: u32,
    decimales_cotizacion: u32,
) -> Option<f64> {
    let reserva_base = token_legible(reserva_de_token(reservas, token_base)?, decimales_base);
    let reserva_cotizacion = token_legible(
        reserva_de_token(reservas, token_cotizacion)?,
        decimales_cotizacion,
    );

    if reserva_base <= 0.0 || reserva_cotizacion <= 0.0 {
        None
    } else {
        Some(reserva_cotizacion / reserva_base)
    }
}

fn calcular_diferencia_porcentaje(precio_a: f64, precio_b: f64) -> f64 {
    if precio_a <= 0.0 || precio_b <= 0.0 {
        0.0
    } else {
        ((precio_a - precio_b) / precio_b).abs() * 100.0
    }
}

fn contrato_arbitrage_env() -> Option<String> {
    std::env::var("CONTRATO_ARBITRAGE")
        .ok()
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
}

fn construir_tx_arbitraje_v2(
    contrato_addr: Address,
    wallet_addr: Address,
    pool_compra: Address,
    pool_venta: Address,
    wpol_addr: Address,
    usdt_addr: Address,
    monto_entrada: U256,
    min_profit: U256,
) -> TransactionRequest {
    let selector = &ethers::utils::keccak256(
        "ejecutarArbitraje(address,address,address,address,uint256,uint256)",
    )[..4];

    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&ethers::abi::encode(&[
        ethers::abi::Token::Address(pool_compra),
        ethers::abi::Token::Address(pool_venta),
        ethers::abi::Token::Address(wpol_addr),
        ethers::abi::Token::Address(usdt_addr),
        ethers::abi::Token::Uint(monto_entrada),
        ethers::abi::Token::Uint(min_profit),
    ]));

    TransactionRequest::new()
        .to(contrato_addr)
        .from(wallet_addr)
        .data(calldata)
        .value(U256::zero())
}

async fn saldo_token_en_contrato(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    token_addr: Address,
    contrato_addr: Address,
) -> U256 {
    let selector = &ethers::utils::keccak256("balanceOf(address)")[..4];
    let mut calldata = selector.to_vec();
    calldata.extend_from_slice(&ethers::abi::encode(&[ethers::abi::Token::Address(
        contrato_addr,
    )]));

    let tx = TransactionRequest::new().to(token_addr).data(calldata);

    match cliente.call(&tx.into(), None).await {
        Ok(bytes) if bytes.len() >= 32 => U256::from_big_endian(&bytes[..32]),
        _ => U256::zero(),
    }
}

async fn estimar_gas_arbitraje_v2(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    tx: &TransactionRequest,
    precio_usdt_wpol: f64,
) -> Option<EvaluacionGas> {
    let gas_limit = match cliente.estimate_gas(&tx.clone().into(), None).await {
        Ok(gas) => gas,
        Err(e) => {
            info!(
                "No se pudo estimar gas con simulacion completa; candidato descartado: {}",
                e
            );
            return None;
        }
    };

    let gas_price = match cliente.get_gas_price().await {
        Ok(precio) => precio,
        Err(e) => {
            info!("No se pudo consultar gas price, usando 100 gwei: {}", e);
            U256::from(100_000_000_000u64)
        }
    };

    let coste_pol = gas_limit.as_u128() as f64 * gas_price.as_u128() as f64 / 1e18;
    let coste_usdt = coste_pol * precio_usdt_wpol;

    Some(EvaluacionGas {
        gas_limit,
        gas_price,
        coste_usdt,
    })
}

async fn estimar_gas_triangular_v2(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    gas_limit: U256,
    precio_token_por_pol: f64,
) -> Option<EvaluacionGas> {
    let gas_price = match cliente.get_gas_price().await {
        Ok(precio) => precio,
        Err(e) => {
            info!(
                "No se pudo consultar gas price triangular, usando 100 gwei: {}",
                e
            );
            U256::from(100_000_000_000u64)
        }
    };

    let coste_pol = gas_limit.as_u128() as f64 * gas_price.as_u128() as f64 / 1e18;
    Some(EvaluacionGas {
        gas_limit,
        gas_price,
        coste_usdt: coste_pol * precio_token_por_pol,
    })
}

fn precio_token_inicio_por_pol(
    ruta: &RutaTriangularV2Generada,
    reservas: &HashMap<String, ReservasPool>,
) -> Option<f64> {
    let primer_paso = ruta.pasos.first()?;
    let reservas_primer_pool = reservas.get(primer_paso.pool)?;
    let wpol: Address = TOKEN_WPOL.parse().ok()?;
    let token_inicio: Address = ruta.token_inicio.parse().ok()?;
    let (reserva_wpol, reserva_inicio) =
        reservas_ordenadas(reservas_primer_pool, wpol, token_inicio)?;

    let wpol_legible = token_legible(reserva_wpol, 18);
    let inicio_legible = token_legible(reserva_inicio, ruta.decimales_inicio);

    if wpol_legible <= 0.0 {
        None
    } else {
        Some(inicio_legible / wpol_legible)
    }
}

fn parse_token_literal(valor: &str, decimales_token: u32) -> Option<U256> {
    let normalizado = valor.trim().replace(',', ".");
    let mut partes = normalizado.split('.');
    let enteros = partes.next()?.parse::<u128>().ok()?;
    let decimales = partes.next().unwrap_or("");

    if partes.next().is_some() || decimales.len() > decimales_token as usize {
        return None;
    }

    let mut decimales_padded = decimales.to_string();
    while decimales_padded.len() < decimales_token as usize {
        decimales_padded.push('0');
    }

    let fraccion = if decimales_padded.is_empty() {
        0
    } else {
        decimales_padded.parse::<u128>().ok()?
    };

    Some(U256::from(enteros * 10u128.pow(decimales_token) + fraccion))
}

fn montos_triangular_v2(simbolo_inicio: &str, decimales_inicio: u32) -> Vec<U256> {
    let nombre_env = format!("MONTOS_TRIANGULAR_{}", simbolo_inicio);

    if let Ok(valor) = std::env::var(nombre_env) {
        let montos: Vec<_> = valor
            .split(',')
            .filter_map(|monto| parse_token_literal(monto, decimales_inicio))
            .filter(|monto| *monto > U256::zero())
            .collect();

        if !montos.is_empty() {
            return montos;
        }
    }

    [5u64, 10, 25, 50, 100]
        .into_iter()
        .map(|monto| U256::from(monto) * U256::from(10u128).pow(U256::from(decimales_inicio)))
        .collect()
}

fn simular_ruta_triangular_v2(
    ruta: &RutaTriangularV2Generada,
    monto_inicial: U256,
    reservas: &HashMap<String, ReservasPool>,
) -> Option<ResultadoTriangularV2> {
    let mut monto_actual = monto_inicial;

    for paso in &ruta.pasos {
        let reservas_pool = reservas.get(paso.pool)?;
        let token_in: Address = paso.token_in.parse().ok()?;
        let token_out: Address = paso.token_out.parse().ok()?;
        let (reserva_in, reserva_out) = reservas_ordenadas(reservas_pool, token_in, token_out)?;
        monto_actual = get_amount_out(monto_actual, reserva_in, reserva_out)?;
    }

    let bruto = monto_actual.checked_sub(monto_inicial)?;
    Some(ResultadoTriangularV2 {
        monto_inicial,
        monto_final: monto_actual,
        bruto,
    })
}

async fn leer_reservas_ruta_triangular_v2(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    ruta: &RutaTriangularV2Generada,
) -> HashMap<String, ReservasPool> {
    let lecturas = ruta.pasos.iter().map(|paso| async move {
        let reservas = consultar_reservas(cliente, paso.pool).await?;
        let token_in: Address = paso.token_in.parse().ok()?;
        let token_out: Address = paso.token_out.parse().ok()?;

        if contiene_par_tokens(&reservas, token_in, token_out) {
            Some((paso.pool.to_string(), reservas))
        } else {
            debug!(
                "Escaner triangular ignora pool con par inesperado {} {}",
                paso.dex, paso.pool
            );
            None
        }
    });

    join_all(lecturas).await.into_iter().flatten().collect()
}

async fn mejor_candidato_triangular_v2(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    ruta: &RutaTriangularV2Generada,
    reservas: &HashMap<String, ReservasPool>,
) -> Option<CandidatoTriangularV2> {
    let precio_token_por_pol = precio_token_inicio_por_pol(ruta, reservas)?;
    let gas_limit = U256::from(env_u64("GAS_TRIANGULAR_V2", GAS_TRIANGULAR_V2_DEFECTO));
    let gas = estimar_gas_triangular_v2(cliente, gas_limit, precio_token_por_pol).await?;
    let mut mejor: Option<CandidatoTriangularV2> = None;

    for monto in montos_triangular_v2(ruta.simbolo_inicio, ruta.decimales_inicio) {
        let Some(resultado) = simular_ruta_triangular_v2(ruta, monto, reservas) else {
            continue;
        };

        let bruto_legible = token_legible(resultado.bruto, ruta.decimales_inicio);
        let neto = bruto_legible - gas.coste_usdt;

        if neto <= 0.0 {
            debug!(
                "Triangular V2 descarta {} monto {:.6} {}: bruto {:.6}, gas {:.6}, neto {:.6}",
                ruta.nombre,
                token_legible(monto, ruta.decimales_inicio),
                ruta.simbolo_inicio,
                bruto_legible,
                gas.coste_usdt,
                neto
            );
            continue;
        }

        if mejor.as_ref().map_or(true, |actual| neto > actual.neto) {
            mejor = Some(CandidatoTriangularV2 {
                resultado,
                gas: EvaluacionGas {
                    gas_limit: gas.gas_limit,
                    gas_price: gas.gas_price,
                    coste_usdt: gas.coste_usdt,
                },
                neto,
            });
        }
    }

    mejor
}

async fn leer_reservas_v2_en_paralelo(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
) -> HashMap<String, ReservasPool> {
    let pools_v2: Vec<_> = POOLS_WPOL_USDT
        .iter()
        .filter(|pool| pool.version == 2)
        .collect();

    let lecturas = pools_v2.iter().map(|pool| async move {
        let token_base = match pool.token_base.parse::<Address>() {
            Ok(addr) => addr,
            Err(e) => {
                debug!(
                    "Escaner V2 ignora pool con token base invalido {}: {}",
                    pool.direccion, e
                );
                return None;
            }
        };
        let token_cotizacion = match pool.token_cotizacion.parse::<Address>() {
            Ok(addr) => addr,
            Err(e) => {
                debug!(
                    "Escaner V2 ignora pool con token cotizacion invalido {}: {}",
                    pool.direccion, e
                );
                return None;
            }
        };

        match consultar_reservas(cliente, pool.direccion).await {
            Some(reservas) if contiene_par_tokens(&reservas, token_base, token_cotizacion) => {
                Some((pool.direccion.to_string(), reservas))
            }
            Some(_) => {
                debug!(
                    "Escaner V2 ignora pool sin par esperado {}: {}",
                    pool.par, pool.direccion
                );
                None
            }
            None => {
                debug!("Escaner V2 no pudo leer reservas: {}", pool.direccion);
                None
            }
        }
    });

    join_all(lecturas).await.into_iter().flatten().collect()
}

async fn ejecutar_candidato_v2_calculado(
    cliente: &Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
    contrato_addr: Address,
    wallet_addr: Address,
    candidato: CandidatoArbitrajeV2,
    token_tg: &str,
    chat_id: &str,
) {
    let pool_compra_addr: Address = match candidato.pool_compra.parse() {
        Ok(addr) => addr,
        Err(e) => {
            info!("Pool compra invalido en candidato V2: {}", e);
            return;
        }
    };
    let pool_venta_addr: Address = match candidato.pool_venta.parse() {
        Ok(addr) => addr,
        Err(e) => {
            info!("Pool venta invalido en candidato V2: {}", e);
            return;
        }
    };

    let monto_base = token_legible(candidato.resultado.monto_prestamo, candidato.decimales_base);
    let tx = construir_tx_arbitraje_v2(
        contrato_addr,
        wallet_addr,
        pool_compra_addr,
        pool_venta_addr,
        candidato.token_base,
        candidato.token_cotizacion,
        candidato.resultado.monto_prestamo,
        candidato.min_profit,
    );

    info!(
        "Simulacion final rapida V2 {} con {:.4} {}...",
        candidato.par, monto_base, candidato.simbolo_base
    );

    match cliente.call(&tx.clone().into(), None).await {
        Ok(_) => info!("Simulacion final OK; enviando TX V2 y esperando recibo..."),
        Err(e) => {
            info!("Simulacion final fallida: {}", e);
            enviar_telegram(
                token_tg,
                chat_id,
                &format!(
                    "Candidato V2 descartado en simulacion final {}\nDif: {:.4}%\nMonto: {:.4} {}\nError: {}",
                    candidato.par, candidato.diferencia, monto_base, candidato.simbolo_base, e
                ),
            )
            .await;
            return;
        }
    }

    match cliente.send_transaction(tx, None).await {
        Ok(tx_pendiente) => {
            let hash = format!("{:?}", tx_pendiente.tx_hash());
            info!(
                "TX V2 enviada a mempool; pendiente de confirmacion - hash: {}",
                hash
            );
            enviar_telegram(
                token_tg,
                chat_id,
                &format!(
                    "TX V2 enviada, pendiente de confirmacion {}\nDiferencia: {:.4}%\nMonto: {:.4} {}\nHash: {}\nhttps://polygonscan.com/tx/{}",
                    candidato.par, candidato.diferencia, monto_base, candidato.simbolo_base, hash, hash
                ),
            )
            .await;

            match tx_pendiente.await {
                Ok(Some(recibo)) => {
                    let bloque = recibo
                        .block_number
                        .map(|numero| numero.to_string())
                        .unwrap_or_else(|| "desconocido".to_string());

                    if recibo
                        .status
                        .map(|estado| estado.as_u64() == 1)
                        .unwrap_or(false)
                    {
                        info!("TX V2 confirmada OK en bloque {}", bloque);
                        enviar_telegram(
                            token_tg,
                            chat_id,
                            &format!(
                                "Arbitraje V2 EXITOSO {}\nBloque: {}\nHash: {}\nhttps://polygonscan.com/tx/{}",
                                candidato.par, bloque, hash, hash
                            ),
                        )
                        .await;
                    } else {
                        info!("TX V2 minada con estado FAIL en bloque {}", bloque);
                        enviar_telegram(
                            token_tg,
                            chat_id,
                            &format!(
                                "Arbitraje V2 FALLIDO {}\nBloque: {}\nHash: {}\nhttps://polygonscan.com/tx/{}",
                                candidato.par, bloque, hash, hash
                            ),
                        )
                        .await;
                    }
                }
                Ok(None) => {
                    info!("TX V2 enviada, pero aun sin recibo");
                    enviar_telegram(
                        token_tg,
                        chat_id,
                        &format!(
                            "TX V2 enviada, pero aun sin recibo {}\nHash: {}\nhttps://polygonscan.com/tx/{}",
                            candidato.par, hash, hash
                        ),
                    )
                    .await;
                }
                Err(e) => {
                    info!("Error esperando confirmacion V2: {}", e);
                    enviar_telegram(
                        token_tg,
                        chat_id,
                        &format!(
                            "No se pudo confirmar el estado del arbitraje V2 {}\nHash: {}\nError: {}\nhttps://polygonscan.com/tx/{}",
                            candidato.par, hash, e, hash
                        ),
                    )
                    .await;
                }
            }
        }
        Err(e) => info!("Error enviando transaccion V2: {}", e),
    }
}

fn mejor_candidato_v2(reservas: &HashMap<String, ReservasPool>) -> Option<CandidatoArbitrajeV2> {
    let mut mejor: Option<CandidatoArbitrajeV2> = None;
    let pools_v2: Vec<_> = POOLS_WPOL_USDT
        .iter()
        .filter(|pool| pool.version == 2)
        .collect();

    for (i, pool_a) in pools_v2.iter().enumerate() {
        for pool_b in pools_v2.iter().skip(i + 1) {
            if pool_a.par != pool_b.par {
                continue;
            }

            let Ok(token_base) = pool_a.token_base.parse::<Address>() else {
                continue;
            };
            let Ok(token_cotizacion) = pool_a.token_cotizacion.parse::<Address>() else {
                continue;
            };
            let min_profit = min_profit_token(pool_a.decimales_cotizacion);
            let min_profit_local = min_profit * U256::from(multiplicador_margen_local());

            let Some(reservas_a) = reservas.get(pool_a.direccion) else {
                continue;
            };
            let Some(reservas_b) = reservas.get(pool_b.direccion) else {
                continue;
            };

            let Some(precio_a) = precio_base_cotizacion(
                reservas_a,
                token_base,
                token_cotizacion,
                pool_a.decimales_base,
                pool_a.decimales_cotizacion,
            ) else {
                continue;
            };
            let Some(precio_b) = precio_base_cotizacion(
                reservas_b,
                token_base,
                token_cotizacion,
                pool_a.decimales_base,
                pool_a.decimales_cotizacion,
            ) else {
                continue;
            };

            let (
                pool_compra,
                pool_venta,
                reservas_compra,
                reservas_venta,
                precio_compra,
                precio_venta,
            ) = if precio_a < precio_b {
                (
                    pool_a.direccion,
                    pool_b.direccion,
                    reservas_a,
                    reservas_b,
                    precio_a,
                    precio_b,
                )
            } else {
                (
                    pool_b.direccion,
                    pool_a.direccion,
                    reservas_b,
                    reservas_a,
                    precio_b,
                    precio_a,
                )
            };

            let diferencia = calcular_diferencia_porcentaje(precio_compra, precio_venta);
            let Some(resultado) = calcular_mejor_monto_v2(
                reservas_compra,
                reservas_venta,
                token_base,
                token_cotizacion,
                min_profit_local,
            ) else {
                debug!(
                    "Escaner V2 descarta {} {} -> {}: dif {:.4}% sin beneficio bruto mayor a {:.6} {}",
                    pool_a.par,
                    pool_compra,
                    pool_venta,
                    diferencia,
                    usdt_legible(min_profit_local),
                    pool_a.simbolo_cotizacion
                );
                continue;
            };

            if mejor.as_ref().map_or(true, |actual| {
                resultado.beneficio_usdt > actual.resultado.beneficio_usdt
            }) {
                mejor = Some(CandidatoArbitrajeV2 {
                    par: pool_a.par.to_string(),
                    pool_compra: pool_compra.to_string(),
                    pool_venta: pool_venta.to_string(),
                    token_base,
                    token_cotizacion,
                    simbolo_base: pool_a.simbolo_base.to_string(),
                    simbolo_cotizacion: pool_a.simbolo_cotizacion.to_string(),
                    decimales_base: pool_a.decimales_base,
                    decimales_cotizacion: pool_a.decimales_cotizacion,
                    diferencia,
                    precio_compra,
                    precio_venta,
                    resultado,
                    min_profit,
                });
            }
        }
    }

    mejor
}

pub async fn iniciar_escaner_v2(rpc_mainnet: &str, wallet: &LocalWallet) {
    if !env_bool("ESCANER_V2_ACTIVO", true) {
        info!("Escaner V2 desactivado por ESCANER_V2_ACTIVO=false");
        return;
    }

    let token_tg = std::env::var("TELEGRAM_TOKEN").unwrap_or_default();
    let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
    let ejecutar = env_bool("ESCANER_V2_EJECUTAR", true);
    let intervalo = env_u64("ESCANER_V2_SEGUNDOS", ESCANER_V2_SEGUNDOS_DEFECTO);
    let cooldown = env_u64(
        "ESCANER_V2_COOLDOWN_SEGUNDOS",
        ESCANER_V2_COOLDOWN_SEGUNDOS_DEFECTO,
    );

    let contrato_arbitrage = match contrato_arbitrage_env() {
        Some(valor) => valor,
        None => {
            info!("Escaner V2 activo solo en modo observacion: falta CONTRATO_ARBITRAGE");
            String::new()
        }
    };

    let proveedor = Provider::<Http>::try_from(rpc_mainnet).expect("Error conectando a Polygon");
    let wallet_mainnet = wallet.clone().with_chain_id(137u64);
    let cliente = Arc::new(SignerMiddleware::new(proveedor, wallet_mainnet));

    let min_profit = min_profit_usdt();
    let min_profit_local = min_profit * U256::from(multiplicador_margen_local());
    let min_neto = parse_usdt_env("MIN_PROFIT_NETO_USDT").unwrap_or_else(min_profit_usdt);
    let mut ultimo_envio: HashMap<String, std::time::Instant> = HashMap::new();
    let pools_v2_count = POOLS_WPOL_USDT
        .iter()
        .filter(|pool| pool.version == 2)
        .count();

    info!(
        "Escaner V2 activo: {} pools, cada {}s, ejecucion={}, margen bruto {:.2} USDT, neto minimo {:.2} USDT",
        pools_v2_count,
        intervalo,
        ejecutar,
        usdt_legible(min_profit_local),
        usdt_legible(min_neto)
    );

    loop {
        let reservas = leer_reservas_v2_en_paralelo(&cliente).await;

        if let Some(candidato) = mejor_candidato_v2(&reservas) {
            let beneficio_bruto = token_legible(
                candidato.resultado.beneficio_usdt,
                candidato.decimales_cotizacion,
            );
            let monto_base =
                token_legible(candidato.resultado.monto_prestamo, candidato.decimales_base);
            let clave = format!("{}:{}", candidato.pool_compra, candidato.pool_venta);
            let en_cooldown = ultimo_envio
                .get(&clave)
                .is_some_and(|ultimo| ultimo.elapsed() < Duration::from_secs(cooldown));

            info!(
                "Escaner V2 candidato {}: dif {:.4}% precio compra {:.6} venta {:.6} monto {:.4} {} bruto {:.6} {} compra {} venta {}",
                candidato.par,
                candidato.diferencia,
                candidato.precio_compra,
                candidato.precio_venta,
                monto_base,
                beneficio_bruto,
                candidato.simbolo_base,
                candidato.simbolo_cotizacion,
                candidato.pool_compra,
                candidato.pool_venta
            );

            if !contrato_arbitrage.is_empty() {
                let contrato_addr: Address = match contrato_arbitrage.parse() {
                    Ok(addr) => addr,
                    Err(e) => {
                        info!("CONTRATO_ARBITRAGE invalido para escaner V2: {}", e);
                        time::sleep(Duration::from_secs(intervalo)).await;
                        continue;
                    }
                };
                let pool_compra_addr: Address = match candidato.pool_compra.parse() {
                    Ok(addr) => addr,
                    Err(_) => {
                        time::sleep(Duration::from_secs(intervalo)).await;
                        continue;
                    }
                };
                let pool_venta_addr: Address = match candidato.pool_venta.parse() {
                    Ok(addr) => addr,
                    Err(_) => {
                        time::sleep(Duration::from_secs(intervalo)).await;
                        continue;
                    }
                };
                let tx = construir_tx_arbitraje_v2(
                    contrato_addr,
                    wallet.address(),
                    pool_compra_addr,
                    pool_venta_addr,
                    candidato.token_base,
                    candidato.token_cotizacion,
                    candidato.resultado.monto_prestamo,
                    min_profit,
                );
                let Some(gas) =
                    estimar_gas_arbitraje_v2(&cliente, &tx, candidato.precio_compra).await
                else {
                    time::sleep(Duration::from_secs(intervalo)).await;
                    continue;
                };
                let neto = beneficio_bruto - gas.coste_usdt;

                info!(
                    "Escaner V2 neto: bruto {:.6} USDT - gas {:.6} USDT (limit {}, price {} wei) = {:.6} USDT",
                    beneficio_bruto, gas.coste_usdt, gas.gas_limit, gas.gas_price, neto
                );

                if neto <= usdt_legible(min_neto) {
                    debug!(
                        "Escaner V2 descarta por neto insuficiente: {:.6} <= {:.6} USDT",
                        neto,
                        usdt_legible(min_neto)
                    );
                } else if en_cooldown {
                    info!(
                        "Escaner V2 candidato en cooldown {}s para {}",
                        cooldown, clave
                    );
                } else if ejecutar {
                    ultimo_envio.insert(clave, std::time::Instant::now());
                    enviar_telegram(
                        &token_tg,
                        &chat_id,
                        &format!(
                            "Escaner V2 va a ejecutar {}\nDif: {:.4}%\nBruto: {:.6} {}\nGas est.: {:.6} {}\nNeto: {:.6} {}\nMonto: {:.4} {}",
                            candidato.par,
                            candidato.diferencia,
                            beneficio_bruto,
                            candidato.simbolo_cotizacion,
                            gas.coste_usdt,
                            candidato.simbolo_cotizacion,
                            neto,
                            candidato.simbolo_cotizacion,
                            monto_base,
                            candidato.simbolo_base
                        ),
                    )
                    .await;
                    ejecutar_candidato_v2_calculado(
                        &cliente,
                        contrato_addr,
                        wallet.address(),
                        candidato,
                        &token_tg,
                        &chat_id,
                    )
                    .await;
                } else {
                    info!("Escaner V2 no ejecuta porque ESCANER_V2_EJECUTAR=false");
                }
            }
        } else {
            debug!("Escaner V2: sin candidatos rentables en esta vuelta");
        }

        time::sleep(Duration::from_secs(intervalo)).await;
    }
}

pub async fn iniciar_escaner_triangular_v2(rpc_mainnet: &str, wallet: &LocalWallet) {
    if !env_bool("ESCANER_TRIANGULAR_ACTIVO", true) {
        info!("Escaner triangular V2 desactivado por ESCANER_TRIANGULAR_ACTIVO=false");
        return;
    }

    let token_tg = std::env::var("TELEGRAM_TOKEN").unwrap_or_default();
    let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
    let ejecutar = env_bool("ESCANER_TRIANGULAR_EJECUTAR", false);
    let intervalo = env_u64(
        "ESCANER_TRIANGULAR_SEGUNDOS",
        ESCANER_TRIANGULAR_SEGUNDOS_DEFECTO,
    );
    let cooldown = env_u64(
        "ESCANER_TRIANGULAR_COOLDOWN_SEGUNDOS",
        ESCANER_TRIANGULAR_COOLDOWN_SEGUNDOS_DEFECTO,
    );
    let min_neto = parse_usdt_env("MIN_PROFIT_NETO_USDT").unwrap_or_else(U256::zero);
    let proveedor = Provider::<Http>::try_from(rpc_mainnet).expect("Error conectando a Polygon");
    let wallet_mainnet = wallet.clone().with_chain_id(137u64);
    let cliente = Arc::new(SignerMiddleware::new(proveedor, wallet_mainnet));
    let mut ultimo_aviso: HashMap<String, std::time::Instant> = HashMap::new();
    let rutas = rutas_triangulares_v2();

    info!(
        "Escaner triangular V2 activo: {} rutas, cada {}s, ejecucion={}, neto minimo {:.6} USDT",
        rutas.len(),
        intervalo,
        ejecutar,
        usdt_legible(min_neto)
    );

    if ejecutar {
        info!(
            "ESCANER_TRIANGULAR_EJECUTAR=true, pero la triangulacion V2 queda solo en observacion hasta anadir contrato de ejecucion"
        );
    }

    loop {
        for ruta in &rutas {
            let reservas = leer_reservas_ruta_triangular_v2(&cliente, ruta).await;

            if reservas.len() != ruta.pasos.len() {
                debug!(
                    "Escaner triangular V2 no pudo leer todos los pools de {} ({}/{})",
                    ruta.nombre,
                    reservas.len(),
                    ruta.pasos.len()
                );
                continue;
            }

            let Some(candidato) = mejor_candidato_triangular_v2(&cliente, ruta, &reservas).await
            else {
                debug!(
                    "Escaner triangular V2 sin candidato rentable para {}",
                    ruta.nombre
                );
                continue;
            };

            let bruto = token_legible(candidato.resultado.bruto, ruta.decimales_inicio);
            let monto_in = token_legible(candidato.resultado.monto_inicial, ruta.decimales_inicio);
            let monto_out = token_legible(candidato.resultado.monto_final, ruta.decimales_inicio);

            info!(
                "Triangular V2 candidato {}: monto_in {:.6} {} monto_out {:.6} {} bruto {:.6} {} gas {:.6} {} (limit {}, price {} wei) neto {:.6} {}",
                ruta.nombre,
                monto_in,
                ruta.simbolo_inicio,
                monto_out,
                ruta.simbolo_inicio,
                bruto,
                ruta.simbolo_inicio,
                candidato.gas.coste_usdt,
                ruta.simbolo_inicio,
                candidato.gas.gas_limit,
                candidato.gas.gas_price,
                candidato.neto,
                ruta.simbolo_inicio
            );

            if candidato.neto <= usdt_legible(min_neto) {
                debug!(
                    "Triangular V2 descarta por neto insuficiente: {:.6} <= {:.6} {}",
                    candidato.neto,
                    usdt_legible(min_neto),
                    ruta.simbolo_inicio
                );
                continue;
            }

            let en_cooldown = ultimo_aviso
                .get(&ruta.nombre)
                .is_some_and(|ultimo| ultimo.elapsed() < Duration::from_secs(cooldown));

            if en_cooldown {
                debug!(
                    "Triangular V2 candidato en cooldown {}s para {}",
                    cooldown, ruta.nombre
                );
                continue;
            }

            ultimo_aviso.insert(ruta.nombre.to_string(), std::time::Instant::now());
            enviar_telegram(
                &token_tg,
                &chat_id,
                &format!(
                    "Triangular V2 observado\nRuta: {}\nMonto in: {:.6} {}\nMonto out: {:.6} {}\nBruto: {:.6} {}\nGas est.: {:.6} {}\nNeto: {:.6} {}\nEjecucion: observacion",
                    ruta.nombre,
                    monto_in,
                    ruta.simbolo_inicio,
                    monto_out,
                    ruta.simbolo_inicio,
                    bruto,
                    ruta.simbolo_inicio,
                    candidato.gas.coste_usdt,
                    ruta.simbolo_inicio,
                    candidato.neto,
                    ruta.simbolo_inicio
                ),
            )
            .await;
        }

        time::sleep(Duration::from_secs(intervalo)).await;
    }
}

pub async fn ejecutar_oportunidad_v2_tokens(
    diferencia: f64,
    pool_compra: String,
    pool_venta: String,
    _precio: f64,
    token_base: Address,
    token_cotizacion: Address,
    simbolo_base: &str,
    simbolo_cotizacion: &str,
    decimales_base: u32,
    decimales_cotizacion: u32,
    wallet: &LocalWallet,
    rpc_mainnet: &str,
) {
    info!(
        "Coordinador activado - diferencia: {:.4}% - preparando arbitraje V2 {}...",
        diferencia, simbolo_cotizacion
    );

    let token_tg = std::env::var("TELEGRAM_TOKEN").unwrap_or_default();
    let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
    let contrato_arbitrage = match contrato_arbitrage_env() {
        Some(valor) => valor,
        None => {
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

    if !contiene_par_tokens(&reservas_compra, token_base, token_cotizacion)
        || !contiene_par_tokens(&reservas_venta, token_base, token_cotizacion)
    {
        info!(
            "Alguno de los pools no es {}/{}, abortando",
            simbolo_base, simbolo_cotizacion
        );
        return;
    }

    let min_profit = min_profit_token(decimales_cotizacion);
    let multiplicador_margen = multiplicador_margen_local();
    let min_profit_local = min_profit * U256::from(multiplicador_margen);
    let resultado_estimado = match calcular_mejor_monto_v2(
        &reservas_compra,
        &reservas_venta,
        token_base,
        token_cotizacion,
        min_profit_local,
    ) {
        Some(resultado) => resultado,
        None => {
            info!(
                "Oportunidad descartada: no hay monto V2 con beneficio estimado mayor al margen local de {:.6} {}",
                usdt_legible(min_profit_local),
                simbolo_cotizacion
            );
            return;
        }
    };

    let monto_entrada = resultado_estimado.monto_prestamo;
    let monto_base = token_legible(monto_entrada, decimales_base);
    let deuda_usdt = token_legible(resultado_estimado.deuda_usdt, decimales_cotizacion);
    let salida_usdt = token_legible(resultado_estimado.salida_usdt, decimales_cotizacion);
    let beneficio_usdt = token_legible(resultado_estimado.beneficio_usdt, decimales_cotizacion);

    info!(
        "Monto dinamico calculado: {:.4} {} - deuda: {:.6} {} - salida: {:.6} {} - beneficio estimado: {:.6} {} - beneficio minimo contrato: {:.6} {} - margen local: {:.6} {}",
        monto_base,
        simbolo_base,
        deuda_usdt,
        simbolo_cotizacion,
        salida_usdt,
        simbolo_cotizacion,
        beneficio_usdt,
        simbolo_cotizacion,
        token_legible(min_profit, decimales_cotizacion),
        simbolo_cotizacion,
        token_legible(min_profit_local, decimales_cotizacion),
        simbolo_cotizacion
    );

    let pool_compra_addr: Address = pool_compra.parse().expect("Pool compra invalido");
    let pool_venta_addr: Address = pool_venta.parse().expect("Pool venta invalido");
    let tx = construir_tx_arbitraje_v2(
        contrato_addr,
        wallet.address(),
        pool_compra_addr,
        pool_venta_addr,
        token_base,
        token_cotizacion,
        monto_entrada,
        min_profit,
    );

    let min_neto = parse_usdt_env("MIN_PROFIT_NETO_USDT").unwrap_or_else(min_profit_usdt);
    let precio_gas = precio_base_cotizacion(
        &reservas_compra,
        token_base,
        token_cotizacion,
        decimales_base,
        decimales_cotizacion,
    )
    .unwrap_or(_precio);
    let Some(gas) = estimar_gas_arbitraje_v2(&cliente, &tx, precio_gas).await else {
        info!("Oportunidad descartada: no se pudo validar gas/neto antes de ejecutar");
        return;
    };
    let neto = beneficio_usdt - gas.coste_usdt;

    info!(
        "Neto estimado V2 evento: bruto {:.6} {} - gas {:.6} {} (limit {}, price {} wei) = {:.6} {} - minimo {:.6} {}",
        beneficio_usdt,
        simbolo_cotizacion,
        gas.coste_usdt,
        simbolo_cotizacion,
        gas.gas_limit,
        gas.gas_price,
        neto,
        simbolo_cotizacion,
        usdt_legible(min_neto),
        simbolo_cotizacion
    );

    if neto <= usdt_legible(min_neto) {
        info!(
            "Oportunidad descartada: neto estimado {:.6} {} <= minimo {:.6} {}",
            neto,
            simbolo_cotizacion,
            usdt_legible(min_neto),
            simbolo_cotizacion
        );
        return;
    }

    info!(
        "Simulando transaccion con {:.4} {}...",
        monto_base, simbolo_base
    );
    match cliente.call(&tx.clone().into(), None).await {
        Ok(_) => info!("Simulacion OK; enviando TX V2 por evento y esperando recibo..."),
        Err(e) => {
            info!("Simulacion fallida: {}", e);
            enviar_telegram(
                &token_tg,
                &chat_id,
                &format!(
                    "Oportunidad detectada pero la simulacion fallo\nDiferencia: {:.4}%\nMonto: {:.4} {}\nError: {}",
                    diferencia, monto_base, simbolo_base, e
                ),
            )
            .await;
            return;
        }
    }

    match cliente.send_transaction(tx, None).await {
        Ok(tx_pendiente) => {
            let hash = format!("{:?}", tx_pendiente.tx_hash());
            info!(
                "TX V2 por evento enviada a mempool; pendiente de confirmacion - hash: {}",
                hash
            );

            enviar_telegram(
                &token_tg,
                &chat_id,
                &format!(
                    "TX V2 enviada, pendiente de confirmacion {}\nDiferencia: {:.4}%\nMonto: {:.4} {}\nHash: {}\nhttps://polygonscan.com/tx/{}",
                    simbolo_cotizacion, diferencia, monto_base, simbolo_base, hash, hash
                ),
            )
            .await;

            match tx_pendiente.await {
                Ok(Some(recibo)) => {
                    let bloque = recibo
                        .block_number
                        .map(|numero| numero.to_string())
                        .unwrap_or_else(|| "desconocido".to_string());

                    if recibo
                        .status
                        .map(|estado| estado.as_u64() == 1)
                        .unwrap_or(false)
                    {
                        info!("TX V2 por evento confirmada OK en bloque {}", bloque);
                        enviar_telegram(
                            &token_tg,
                            &chat_id,
                            &format!(
                                "Arbitraje V2 EXITOSO {}\nBloque: {}\nHash: {}\nhttps://polygonscan.com/tx/{}",
                                simbolo_cotizacion, bloque, hash, hash
                            ),
                        )
                        .await;
                    } else {
                        info!(
                            "TX V2 por evento minada con estado FAIL en bloque {}",
                            bloque
                        );
                        enviar_telegram(
                            &token_tg,
                            &chat_id,
                            &format!(
                                "Arbitraje V2 FALLIDO {}\nBloque: {}\nHash: {}\nhttps://polygonscan.com/tx/{}",
                                simbolo_cotizacion, bloque, hash, hash
                            ),
                        )
                        .await;
                        return;
                    }
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

            let saldo_contrato =
                saldo_token_en_contrato(&cliente, token_cotizacion, contrato_addr).await;
            let saldo_usdt = saldo_contrato.as_u64();
            let saldo_legible = saldo_usdt as f64 / 1_000_000.0;

            info!(
                "{} acumulado en contrato: {:.6}",
                simbolo_cotizacion, saldo_legible
            );

            if saldo_usdt >= UMBRAL_AVISO_USDT {
                enviar_telegram(
                    &token_tg,
                    &chat_id,
                    &format!(
                        "Ganancia acumulada en contrato: {:.6} {}. No se retiro automaticamente.",
                        saldo_legible, simbolo_cotizacion
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
    parse_token_env(nombre, 6)
}

fn parse_token_env(nombre: &str, decimales_token: u32) -> Option<U256> {
    let valor = std::env::var(nombre).ok()?;
    parse_token_literal(&valor, decimales_token)
}

fn escalar_unidades(valor: u64, decimales_origen: u32, decimales_destino: u32) -> U256 {
    if decimales_destino >= decimales_origen {
        U256::from(valor) * U256::from(10u128).pow(U256::from(decimales_destino - decimales_origen))
    } else {
        U256::from(valor) / U256::from(10u128).pow(U256::from(decimales_origen - decimales_destino))
    }
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
        Ok(valor) if !valor.trim().is_empty() => valor.trim().to_string(),
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
    let min_profit = min_profit_usdt();

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
    info!(
        "Simulando arbitraje V3 con {:.2} USDT - beneficio minimo contrato: {:.2} USDT...",
        monto_usdt,
        usdt_legible(min_profit)
    );

    match cliente.call(&tx.clone().into(), None).await {
        Ok(_) => info!("Simulacion V3 OK; enviando TX V3 y esperando recibo..."),
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
            info!(
                "TX V3 enviada a mempool; pendiente de confirmacion - hash: {}",
                hash
            );
            enviar_telegram(
                &token_tg,
                &chat_id,
                &format!(
                    "TX V3 enviada, pendiente de confirmacion\nDiferencia: {:.4}%\nMonto: {:.2} USDT\nHash: {}\nhttps://polygonscan.com/tx/{}",
                    diferencia, monto_usdt, hash, hash
                ),
            )
            .await;

            match tx_pendiente.await {
                Ok(Some(recibo)) => {
                    let bloque = recibo
                        .block_number
                        .map(|numero| numero.to_string())
                        .unwrap_or_else(|| "desconocido".to_string());

                    if recibo
                        .status
                        .map(|estado| estado.as_u64() == 1)
                        .unwrap_or(false)
                    {
                        info!("TX V3 confirmada OK en bloque {}", bloque);
                        enviar_telegram(
                            &token_tg,
                            &chat_id,
                            &format!(
                                "Arbitraje V3 EXITOSO\nBloque: {}\nHash: {}\nhttps://polygonscan.com/tx/{}",
                                bloque, hash, hash
                            ),
                        )
                        .await;
                    } else {
                        info!("TX V3 minada con estado FAIL en bloque {}", bloque);
                        enviar_telegram(
                            &token_tg,
                            &chat_id,
                            &format!(
                                "Arbitraje V3 FALLIDO\nBloque: {}\nHash: {}\nhttps://polygonscan.com/tx/{}",
                                bloque, hash, hash
                            ),
                        )
                        .await;
                    }
                }
                Ok(None) => {
                    info!("TX V3 enviada, pero aun sin recibo");
                    enviar_telegram(
                        &token_tg,
                        &chat_id,
                        &format!(
                            "TX V3 enviada, pero aun sin recibo\nHash: {}\nhttps://polygonscan.com/tx/{}",
                            hash, hash
                        ),
                    )
                    .await;
                }
                Err(e) => {
                    info!("Error esperando confirmacion V3: {}", e);
                    enviar_telegram(
                        &token_tg,
                        &chat_id,
                        &format!(
                            "No se pudo confirmar el estado del arbitraje V3\nHash: {}\nError: {}\nhttps://polygonscan.com/tx/{}",
                            hash, e, hash
                        ),
                    )
                    .await;
                }
            }
        }
        Err(e) => info!("Error enviando transaccion V3: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    fn wpol(cantidad: u128) -> U256 {
        U256::from(cantidad) * U256::from(10u128).pow(U256::from(18u32))
    }

    fn usdt(cantidad: u128) -> U256 {
        U256::from(cantidad) * U256::from(1_000_000u128)
    }

    fn pool_wpol_usdt(reserva_wpol: U256, reserva_usdt: U256) -> ReservasPool {
        ReservasPool {
            token0: addr(1),
            token1: addr(2),
            reserve0: reserva_wpol,
            reserve1: reserva_usdt,
        }
    }

    #[test]
    fn calcula_monto_v2_con_beneficio_real() {
        let wpol_addr = addr(1);
        let usdt_addr = addr(2);
        let pool_barato = pool_wpol_usdt(wpol(1_000_000), usdt(200_000_000));
        let pool_caro = pool_wpol_usdt(wpol(1_000_000), usdt(250_000_000));

        let resultado = calcular_mejor_monto_v2(
            &pool_barato,
            &pool_caro,
            wpol_addr,
            usdt_addr,
            U256::from(MIN_PROFIT_USDT_DEFECTO),
        )
        .expect("debe encontrar un monto rentable");

        assert!(resultado.monto_prestamo > U256::zero());
        assert!(resultado.salida_usdt > resultado.deuda_usdt + U256::from(MIN_PROFIT_USDT_DEFECTO));
    }

    #[test]
    fn descarta_v2_sin_beneficio_suficiente() {
        let wpol_addr = addr(1);
        let usdt_addr = addr(2);
        let pool_a = pool_wpol_usdt(wpol(1_000_000), usdt(200_000_000));
        let pool_b = pool_wpol_usdt(wpol(1_000_000), usdt(200_100_000));

        let resultado = calcular_mejor_monto_v2(
            &pool_a,
            &pool_b,
            wpol_addr,
            usdt_addr,
            U256::from(MIN_PROFIT_USDT_DEFECTO),
        );

        assert!(resultado.is_none());
    }

    #[test]
    fn simula_ruta_triangular_v2_con_beneficio() {
        static PASOS: &[crate::pools::PasoTriangularV2] = &[
            crate::pools::PasoTriangularV2 {
                pool: "pool-a-b",
                dex: "Test V2",
                token_in: "0x0101010101010101010101010101010101010101",
                token_out: "0x0202020202020202020202020202020202020202",
            },
            crate::pools::PasoTriangularV2 {
                pool: "pool-b-c",
                dex: "Test V2",
                token_in: "0x0202020202020202020202020202020202020202",
                token_out: "0x0303030303030303030303030303030303030303",
            },
            crate::pools::PasoTriangularV2 {
                pool: "pool-c-a",
                dex: "Test V2",
                token_in: "0x0303030303030303030303030303030303030303",
                token_out: "0x0101010101010101010101010101010101010101",
            },
        ];
        let ruta = crate::pools::RutaTriangularV2Generada {
            nombre: "A -> B -> C -> A".to_string(),
            token_inicio: "0x0101010101010101010101010101010101010101",
            simbolo_inicio: "A",
            decimales_inicio: 0,
            pasos: PASOS.to_vec(),
        };

        let mut reservas = HashMap::new();
        reservas.insert(
            "pool-a-b".to_string(),
            ReservasPool {
                token0: addr(1),
                token1: addr(2),
                reserve0: U256::from(1_000u64),
                reserve1: U256::from(1_000u64),
            },
        );
        reservas.insert(
            "pool-b-c".to_string(),
            ReservasPool {
                token0: addr(2),
                token1: addr(3),
                reserve0: U256::from(1_000u64),
                reserve1: U256::from(2_000u64),
            },
        );
        reservas.insert(
            "pool-c-a".to_string(),
            ReservasPool {
                token0: addr(3),
                token1: addr(1),
                reserve0: U256::from(1_000u64),
                reserve1: U256::from(1_000u64),
            },
        );

        let resultado = simular_ruta_triangular_v2(&ruta, U256::from(10u64), &reservas)
            .expect("debe simular una ruta rentable");

        assert!(resultado.monto_final > resultado.monto_inicial);
        assert_eq!(
            resultado.bruto,
            resultado.monto_final - resultado.monto_inicial
        );
    }
}
