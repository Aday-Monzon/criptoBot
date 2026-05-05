use crate::coordinador::ejecutar_oportunidad;
use crate::evaluador::{DatosSwap, evaluar_arbitraje};
use crate::pools::POOLS_WPOL_USDT;
use ethers::{
    abi::{ParamType, decode},
    providers::{Middleware, Provider, Ws},
    types::{Filter, H256, I256},
};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

// Topics de swap
const TOPIC_V2: &str = "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822";
const TOPIC_V3: &str = "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67";

pub async fn iniciar(rpc_polygon: &str, wallet: &ethers::signers::LocalWallet, rpc_amoy: &str) {
    info!("📡 Conectando detector a Polygon...");

    let proveedor = Arc::new(
        Provider::<Ws>::connect(rpc_polygon)
            .await
            .expect("Error al conectar a Polygon"),
    );

    info!("✅ Detector conectado a Polygon correctamente");

    // Construir lista de direcciones y versiones desde la config
    let direcciones_v2: Vec<ethers::types::Address> = POOLS_WPOL_USDT
        .iter()
        .filter(|p| p.version == 2)
        .map(|p| p.direccion.parse().expect("Dirección inválida"))
        .collect();

    let direcciones_v3: Vec<ethers::types::Address> = POOLS_WPOL_USDT
        .iter()
        .filter(|p| p.version == 3)
        .map(|p| p.direccion.parse().expect("Dirección inválida"))
        .collect();

    info!(
        "👂 Escuchando {} pools V2 y {} pools V3...",
        direcciones_v2.len(),
        direcciones_v3.len()
    );

    // Filtros separados para V2 y V3
    let topic_v2: H256 = TOPIC_V2.parse().expect("Topic V2 inválido");
    let topic_v3: H256 = TOPIC_V3.parse().expect("Topic V3 inválido");

    let filtro_v2 = Filter::new().address(direcciones_v2).topic0(topic_v2);

    let filtro_v3 = Filter::new().address(direcciones_v3).topic0(topic_v3);

    // Suscribirse a ambos streams
    let mut stream_v2 = proveedor
        .subscribe_logs(&filtro_v2)
        .await
        .expect("Error suscribiendo V2");

    let mut stream_v3 = proveedor
        .subscribe_logs(&filtro_v3)
        .await
        .expect("Error suscribiendo V3");

    // Mapa de precios compartido
    let mut precios: HashMap<String, f64> = HashMap::new();

    // Mapa de tokens preconfigurado — no necesitamos consultar la blockchain
    let mut tokens_por_pool: HashMap<String, (String, String)> = HashMap::new();
    for pool in POOLS_WPOL_USDT {
        // WPOL / USDT
        tokens_por_pool.insert(
            pool.direccion.to_string(),
            (
                "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270".to_string(), // WPOL
                "0xc2132d05d31c914a87c6611c10748aeb04b58e8f".to_string(), // USDT
            ),
        );
    }

    info!(
        "🚀 Bot activo — monitorizando par WPOL/USDT en {} pools",
        POOLS_WPOL_USDT.len()
    );

    loop {
        tokio::select! {
            Some(log) = stream_v2.next() => {
                let tipos = vec![
                    ParamType::Uint(256), // amount0In
                    ParamType::Uint(256), // amount1In
                    ParamType::Uint(256), // amount0Out
                    ParamType::Uint(256), // amount1Out
                ];

                if let Ok(datos) = decode(&tipos, &log.data) {
                    let pool = format!("{:?}", log.address).to_lowercase();
                    let swap = DatosSwap {
                        pool: pool.clone(),
                        amount0_in: datos[0].clone().into_uint().unwrap_or_default().as_u128(),
                        amount1_in: datos[1].clone().into_uint().unwrap_or_default().as_u128(),
                        amount0_out: datos[2].clone().into_uint().unwrap_or_default().as_u128(),
                        amount1_out: datos[3].clone().into_uint().unwrap_or_default().as_u128(),
                    };

                    info!("🔵 Swap V2 en {} ({})",
                        POOLS_WPOL_USDT.iter().find(|p| p.direccion == pool).map(|p| p.dex).unwrap_or("?"),
                        &pool[..10]);

                    if let Some((dif, pa, pb, precio)) = evaluar_arbitraje(&swap, &mut precios, &tokens_por_pool) {
                        ejecutar_oportunidad(dif, pa, pb, precio, wallet, rpc_amoy).await;
                    }
                }
            }

            Some(log) = stream_v3.next() => {
                let tipos = vec![
                    ParamType::Int(256),  // amount0
                    ParamType::Int(256),  // amount1
                    ParamType::Uint(256), // sqrtPriceX96
                    ParamType::Uint(256), // liquidity
                    ParamType::Int(32),   // tick
                ];

                if let Ok(datos) = decode(&tipos, &log.data) {
                    let pool = format!("{:?}", log.address).to_lowercase();
                    let amount0 = I256::from_raw(datos[0].clone().into_uint().unwrap_or_default());
                    let amount1 = I256::from_raw(datos[1].clone().into_uint().unwrap_or_default());
                    let amount0_abs = amount0.unsigned_abs().as_u128();
                    let amount1_abs = amount1.unsigned_abs().as_u128();

                    let swap = DatosSwap {
                        pool: pool.clone(),
                        amount0_in: if amount0.is_positive() { amount0_abs } else { 0 },
                        amount1_in: if amount1.is_positive() { amount1_abs } else { 0 },
                        amount0_out: if amount0.is_negative() { amount0_abs } else { 0 },
                        amount1_out: if amount1.is_negative() { amount1_abs } else { 0 },
                    };

                    info!("🟣 Swap V3 en {} ({})",
                        POOLS_WPOL_USDT.iter().find(|p| p.direccion == pool).map(|p| p.dex).unwrap_or("?"),
                        &pool[..10]);

                    if let Some((dif, pa, pb, precio)) = evaluar_arbitraje(&swap, &mut precios, &tokens_por_pool) {
                        ejecutar_oportunidad(dif, pa, pb, precio, wallet, rpc_amoy).await;
                    }
                }
            }
        }
    }
}
