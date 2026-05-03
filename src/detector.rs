// Importaciones necesarias
use crate::coordinador::ejecutar_oportunidad;
use crate::evaluador::{DatosSwap, evaluar_arbitraje};
use ethers::{
    abi::{ParamType, decode},
    providers::{Middleware, Provider, Ws},
    types::{Filter, H256, U256},
};
use futures_util::StreamExt;
use std::sync::Arc;
use tracing::info;

// Topic del evento Swap en Uniswap V2
const TOPIC_SWAP: &str = "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67";

pub async fn iniciar(rpc_polygon: &str, wallet: &ethers::signers::LocalWallet) {
    info!("📡 Conectando detector a Polygon...");

    // Conectar vía WebSocket
    let proveedor = Arc::new(
        Provider::<Ws>::connect(rpc_polygon)
            .await
            .expect("Error al conectar a Polygon"),
    );

    info!("✅ Detector conectado a Polygon correctamente");
    info!("👂 Escuchando swaps en Polygon...");

    // Topic del evento Swap
    let topic: H256 = TOPIC_SWAP.parse().expect("Topic inválido");

    // Filtro para todos los swaps
    let filtro = Filter::new().topic0(topic);

    // Suscribirse a los logs
    let mut stream = proveedor
        .subscribe_logs(&filtro)
        .await
        .expect("Error al suscribirse a logs");

    let mut precios = std::collections::HashMap::new();
    // Procesar cada evento de swap
    while let Some(log) = stream.next().await {
        // Decodificar los datos del evento
        let tipos = vec![
            ParamType::Int(256),  // amount0 — puede ser negativo en V3
            ParamType::Int(256),  // amount1 — puede ser negativo en V3
            ParamType::Uint(256), // sqrtPriceX96
            ParamType::Uint(256), // liquidity
            ParamType::Int(32),   // tick
        ];

        if let Ok(datos) = decode(&tipos, &log.data) {
            use ethers::types::I256;

            // En V3 los amounts son signed
            let amount0 = I256::from_raw(datos[0].clone().into_uint().unwrap_or_default());
            let amount1 = I256::from_raw(datos[1].clone().into_uint().unwrap_or_default());

            // Valor absoluto como u128
            let amount0_abs = amount0.unsigned_abs().as_u128();
            let amount1_abs = amount1.unsigned_abs().as_u128();

            let swap = DatosSwap {
                pool: format!("{:?}", log.address),
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
            };

            if let Some((diferencia, pool_a, pool_b)) = evaluar_arbitraje(&swap, &mut precios) {
                ejecutar_oportunidad(diferencia, pool_a, pool_b, wallet).await;
            }
        }
    }
}
