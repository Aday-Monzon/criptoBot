use crate::coordinador::{ejecutar_oportunidad_v2_tokens, ejecutar_oportunidad_v3};
use crate::evaluador::{DatosSwap, evaluar_arbitraje};
use crate::pools::POOLS_WPOL_USDT;
use ethers::{
    abi::{ParamType, decode},
    providers::{Http, Middleware, Provider},
    types::{Address, Filter, H256, I256, Log, U64},
};
use std::{
    collections::HashMap,
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time;
use tracing::{debug, info};

const TOPIC_V2: &str = "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822";
const TOPIC_V3: &str = "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67";
const DETECTOR_HTTP_SEGUNDOS_DEFECTO: u64 = 6;
const DETECTOR_MAX_BLOQUES_LOGS_DEFECTO: u64 = 1;
const DETECTOR_RPC_COOLDOWN_SEGUNDOS_DEFECTO: u64 = 300;

type DetectorResult = Result<(), Box<dyn Error + Send + Sync>>;

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

fn es_pool_v2(pool: &str) -> bool {
    POOLS_WPOL_USDT
        .iter()
        .any(|p| p.version == 2 && p.direccion.eq_ignore_ascii_case(pool))
}

fn pool_config(pool: &str) -> Option<&'static crate::pools::Pool> {
    POOLS_WPOL_USDT
        .iter()
        .find(|p| p.direccion.eq_ignore_ascii_case(pool))
}

fn es_pool_uniswap_v3(pool: &str) -> bool {
    POOLS_WPOL_USDT
        .iter()
        .any(|p| p.version == 3 && p.dex == "Uniswap V3" && p.direccion.eq_ignore_ascii_case(pool))
}

async fn coordinar_oportunidad(
    dif: f64,
    pa: String,
    pb: String,
    precio: f64,
    wallet: &ethers::signers::LocalWallet,
    rpc_mainnet: &str,
) {
    if es_pool_v2(&pa) && es_pool_v2(&pb) {
        let Some(pool_a) = pool_config(&pa) else {
            return;
        };
        let Some(pool_b) = pool_config(&pb) else {
            return;
        };

        if pool_a.par != pool_b.par {
            info!(
                "Oportunidad monitorizada sin ejecutar: pools V2 de pares distintos. Compra: {} Venta: {}",
                pa, pb
            );
            return;
        }

        let token_base = match pool_a.token_base.parse() {
            Ok(addr) => addr,
            Err(_) => return,
        };
        let token_cotizacion = match pool_a.token_cotizacion.parse() {
            Ok(addr) => addr,
            Err(_) => return,
        };

        ejecutar_oportunidad_v2_tokens(
            dif,
            pa,
            pb,
            precio,
            token_base,
            token_cotizacion,
            pool_a.simbolo_base,
            pool_a.simbolo_cotizacion,
            pool_a.decimales_base,
            pool_a.decimales_cotizacion,
            wallet,
            rpc_mainnet,
        )
        .await;
    } else if es_pool_uniswap_v3(&pa) && es_pool_uniswap_v3(&pb) {
        ejecutar_oportunidad_v3(dif, pa, pb, precio, wallet, rpc_mainnet).await;
    } else {
        info!(
            "Oportunidad monitorizada sin ejecutar: combinacion no soportada todavia. Compra: {} Venta: {}",
            pa, pb
        );
    }
}

pub async fn iniciar(
    rpc_polygon_urls: Vec<String>,
    wallet: &ethers::signers::LocalWallet,
    rpc_mainnet: &str,
) {
    if !env_bool("DETECTOR_EVENTOS_ACTIVO", true) {
        info!("Detector por eventos desactivado por DETECTOR_EVENTOS_ACTIVO=false");
        return;
    }

    if rpc_polygon_urls.is_empty() {
        info!("Detector sin RPC_POLYGON configurado");
        return;
    }

    let cooldown_rpc = std::env::var("DETECTOR_RPC_COOLDOWN_SEGUNDOS")
        .ok()
        .and_then(|valor| valor.parse::<u64>().ok())
        .filter(|valor| *valor > 0)
        .unwrap_or(DETECTOR_RPC_COOLDOWN_SEGUNDOS_DEFECTO);
    let mut intento = 0usize;
    let mut bloqueados_hasta: HashMap<String, Instant> = HashMap::new();

    loop {
        let ahora = Instant::now();
        let Some(rpc_polygon) = (0..rpc_polygon_urls.len())
            .map(|offset| &rpc_polygon_urls[(intento + offset) % rpc_polygon_urls.len()])
            .find(|url| {
                bloqueados_hasta
                    .get(url.as_str())
                    .map_or(true, |hasta| *hasta <= ahora)
            })
        else {
            info!("Todos los RPC Polygon estan en cooldown. Reintentando en 5s...");
            time::sleep(Duration::from_secs(5)).await;
            continue;
        };

        if let Err(error) = escuchar_polygon_http(rpc_polygon, wallet, rpc_mainnet).await {
            bloqueados_hasta.insert(
                rpc_polygon.to_string(),
                Instant::now() + Duration::from_secs(cooldown_rpc),
            );
            info!(
                "Detector HTTP desconectado: {}. RPC en cooldown {}s; reintentando con otro RPC en 5s...",
                error, cooldown_rpc
            );
            time::sleep(Duration::from_secs(5)).await;
            intento = intento.wrapping_add(1);
        }
    }
}

async fn escuchar_polygon_http(
    rpc_polygon: &str,
    wallet: &ethers::signers::LocalWallet,
    rpc_mainnet: &str,
) -> DetectorResult {
    info!("Conectando detector HTTP a Polygon...");

    let proveedor = Arc::new(
        Provider::<Http>::try_from(rpc_polygon)
            .map_err(|error| format!("RPC Polygon invalido: {}", error))?,
    );
    proveedor
        .get_chainid()
        .await
        .map_err(|error| format!("error al conectar a Polygon: {}", error))?;

    info!("Detector HTTP conectado a Polygon correctamente");

    let direcciones_v2: Vec<ethers::types::Address> = POOLS_WPOL_USDT
        .iter()
        .filter(|p| p.version == 2)
        .map(|p| p.direccion.parse().expect("Direccion invalida"))
        .collect();

    let detector_v3_activo = env_bool("DETECTOR_V3_ACTIVO", false);
    let direcciones_v3: Vec<ethers::types::Address> = if detector_v3_activo {
        POOLS_WPOL_USDT
            .iter()
            .filter(|p| p.version == 3)
            .map(|p| p.direccion.parse().expect("Direccion invalida"))
            .collect()
    } else {
        Vec::new()
    };

    info!(
        "Escuchando {} pools V2 y {} pools V3 por HTTP...",
        direcciones_v2.len(),
        direcciones_v3.len()
    );

    let topic_v2: H256 = TOPIC_V2.parse().expect("Topic V2 invalido");
    let topic_v3: H256 = TOPIC_V3.parse().expect("Topic V3 invalido");
    let filtro_v2_base = Filter::new()
        .address(direcciones_v2.clone())
        .topic0(topic_v2);
    let filtro_v3_base = Filter::new()
        .address(direcciones_v3.clone())
        .topic0(topic_v3);

    let mut precios: HashMap<String, f64> = HashMap::new();
    let mut tokens_por_pool: HashMap<String, (String, String)> = HashMap::new();

    for pool in POOLS_WPOL_USDT {
        tokens_por_pool.insert(
            pool.direccion.to_string(),
            (
                pool.token_base.to_lowercase(),
                pool.token_cotizacion.to_lowercase(),
            ),
        );
    }

    let intervalo = std::env::var("DETECTOR_HTTP_SEGUNDOS")
        .ok()
        .and_then(|valor| valor.parse::<u64>().ok())
        .filter(|valor| *valor > 0)
        .unwrap_or(DETECTOR_HTTP_SEGUNDOS_DEFECTO);
    let max_bloques_logs = std::env::var("DETECTOR_MAX_BLOQUES_LOGS")
        .ok()
        .and_then(|valor| valor.parse::<u64>().ok())
        .filter(|valor| *valor > 0)
        .unwrap_or(DETECTOR_MAX_BLOQUES_LOGS_DEFECTO);
    let mut ultimo_bloque = proveedor
        .get_block_number()
        .await
        .map_err(|error| format!("error leyendo bloque inicial: {}", error))?;

    info!(
        "Bot activo: monitorizando {} pools desde bloque {}, cada {}s, max {} bloques por getLogs",
        POOLS_WPOL_USDT.len(),
        ultimo_bloque,
        intervalo,
        max_bloques_logs
    );

    loop {
        time::sleep(Duration::from_secs(intervalo)).await;

        let bloque_actual = proveedor
            .get_block_number()
            .await
            .map_err(|error| format!("error leyendo bloque actual: {}", error))?;

        if bloque_actual <= ultimo_bloque {
            continue;
        }

        let mut desde = ultimo_bloque + U64::from(1);
        let max_bloques = U64::from(max_bloques_logs);

        while desde <= bloque_actual {
            let hasta = std::cmp::min(desde + max_bloques - U64::from(1), bloque_actual);
            for log in consultar_logs(
                &proveedor,
                &filtro_v2_base,
                &direcciones_v2,
                topic_v2,
                desde,
                hasta,
                "V2",
            )
            .await?
            {
                procesar_log_v2(log, &mut precios, &tokens_por_pool, wallet, rpc_mainnet).await;
            }

            if detector_v3_activo {
                for log in consultar_logs(
                    &proveedor,
                    &filtro_v3_base,
                    &direcciones_v3,
                    topic_v3,
                    desde,
                    hasta,
                    "V3",
                )
                .await?
                {
                    procesar_log_v3(log, &mut precios, &tokens_por_pool, wallet, rpc_mainnet).await;
                }
            }

            ultimo_bloque = hasta;
            desde = hasta + U64::from(1);
        }
    }
}

fn es_error_rpc_saturado(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("too many requests")
        || error.contains("rate limit")
        || error.contains("tenant disabled")
        || error.contains("api key disabled")
}

fn es_error_rango_logs(error: &str) -> bool {
    error.to_ascii_lowercase().contains("invalid block range")
}

async fn consultar_logs(
    proveedor: &Arc<Provider<Http>>,
    filtro_base: &Filter,
    direcciones: &[Address],
    topic: H256,
    desde: U64,
    hasta: U64,
    nombre: &str,
) -> Result<Vec<Log>, Box<dyn Error + Send + Sync>> {
    let filtro = filtro_base.clone().from_block(desde).to_block(hasta);

    match proveedor.get_logs(&filtro).await {
        Ok(logs) => return Ok(logs),
        Err(error) => {
            let mensaje = error.to_string();
            if es_error_rpc_saturado(&mensaje) {
                return Err(format!("error consultando logs {}: {}", nombre, error).into());
            }

            info!(
                "Consulta agregada de logs {} fallo en bloques {}-{}: {}. Probando pool por pool...",
                nombre, desde, hasta, error
            );

            if !es_error_rango_logs(&mensaje) {
                debug!(
                    "Fallo getLogs {} no clasificado como rango invalido: {}",
                    nombre, error
                );
            }
        }
    }

    let mut logs = Vec::new();
    let mut fallos = 0usize;

    for direccion in direcciones {
        let filtro_pool = Filter::new()
            .address(*direccion)
            .topic0(topic)
            .from_block(desde)
            .to_block(hasta);

        match proveedor.get_logs(&filtro_pool).await {
            Ok(mut logs_pool) => logs.append(&mut logs_pool),
            Err(error) => {
                let mensaje = error.to_string();
                if es_error_rpc_saturado(&mensaje) {
                    return Err(format!("error consultando logs {}: {}", nombre, error).into());
                }

                fallos += 1;
                debug!(
                    "No se pudieron leer logs {} de pool {:?} en bloques {}-{}: {}",
                    nombre, direccion, desde, hasta, error
                );
            }
        }
    }

    if fallos > 0 {
        info!(
            "Logs {}: {} pools no respondieron en bloques {}-{}; se continua sin reiniciar detector",
            nombre, fallos, desde, hasta
        );
    }

    Ok(logs)
}

async fn procesar_log_v2(
    log: Log,
    precios: &mut HashMap<String, f64>,
    tokens_por_pool: &HashMap<String, (String, String)>,
    wallet: &ethers::signers::LocalWallet,
    rpc_mainnet: &str,
) {
    let tipos = vec![
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Uint(256),
    ];

    if let Ok(datos) = decode(&tipos, &log.data) {
        let pool = format!("{:?}", log.address).to_lowercase();
        let Some(config) = pool_config(&pool) else {
            return;
        };
        let swap = DatosSwap {
            pool: pool.clone(),
            amount0_in: datos[0].clone().into_uint().unwrap_or_default().as_u128(),
            amount1_in: datos[1].clone().into_uint().unwrap_or_default().as_u128(),
            amount0_out: datos[2].clone().into_uint().unwrap_or_default().as_u128(),
            amount1_out: datos[3].clone().into_uint().unwrap_or_default().as_u128(),
            decimales0: config.decimales_base,
            decimales1: config.decimales_cotizacion,
        };

        debug!(
            "Swap V2 en {} ({})",
            POOLS_WPOL_USDT
                .iter()
                .find(|p| p.direccion == pool)
                .map(|p| p.dex)
                .unwrap_or("?"),
            &pool[..10]
        );

        if let Some((dif, pa, pb, precio)) = evaluar_arbitraje(&swap, precios, tokens_por_pool) {
            coordinar_oportunidad(dif, pa, pb, precio, wallet, rpc_mainnet).await;
        }
    }
}

async fn procesar_log_v3(
    log: Log,
    precios: &mut HashMap<String, f64>,
    tokens_por_pool: &HashMap<String, (String, String)>,
    wallet: &ethers::signers::LocalWallet,
    rpc_mainnet: &str,
) {
    let tipos = vec![
        ParamType::Int(256),
        ParamType::Int(256),
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Int(32),
    ];

    if let Ok(datos) = decode(&tipos, &log.data) {
        let pool = format!("{:?}", log.address).to_lowercase();
        let Some(config) = pool_config(&pool) else {
            return;
        };
        let amount0 = I256::from_raw(datos[0].clone().into_uint().unwrap_or_default());
        let amount1 = I256::from_raw(datos[1].clone().into_uint().unwrap_or_default());
        let amount0_abs = amount0.unsigned_abs().as_u128();
        let amount1_abs = amount1.unsigned_abs().as_u128();

        let swap = DatosSwap {
            pool: pool.clone(),
            amount0_in: if amount0.is_positive() {
                amount0_abs
            } else {
                0
            },
            amount1_in: if amount1.is_positive() {
                amount1_abs
            } else {
                0
            },
            amount0_out: if amount0.is_negative() {
                amount0_abs
            } else {
                0
            },
            amount1_out: if amount1.is_negative() {
                amount1_abs
            } else {
                0
            },
            decimales0: config.decimales_base,
            decimales1: config.decimales_cotizacion,
        };

        debug!(
            "Swap V3 en {} ({})",
            POOLS_WPOL_USDT
                .iter()
                .find(|p| p.direccion == pool)
                .map(|p| p.dex)
                .unwrap_or("?"),
            &pool[..10]
        );

        if let Some((dif, pa, pb, precio)) = evaluar_arbitraje(&swap, precios, tokens_por_pool) {
            coordinar_oportunidad(dif, pa, pb, precio, wallet, rpc_mainnet).await;
        }
    }
}
