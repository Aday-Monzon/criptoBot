// Importaciones necesarias
use ethers::{
    abi::{ParamType, decode},
    providers::{Middleware, Provider, Ws},
    types::{Filter, H256, U256},
};
use futures_util::StreamExt;
use std::sync::Arc;
use tracing::info;

// Topic del evento Swap en Uniswap V2
const TOPIC_SWAP: &str = "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822";

pub async fn iniciar(rpc_polygon: &str) {
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

    // Procesar cada evento de swap
    while let Some(log) = stream.next().await {
        // Decodificar los datos del evento
        let tipos = vec![
            ParamType::Uint(256), // amount0In
            ParamType::Uint(256), // amount1In
            ParamType::Uint(256), // amount0Out
            ParamType::Uint(256), // amount1Out
        ];

        if let Ok(datos) = decode(&tipos, &log.data) {
            let amount0_in: U256 = datos[0].clone().into_uint().unwrap_or_default();
            let amount1_in: U256 = datos[1].clone().into_uint().unwrap_or_default();
            let amount0_out: U256 = datos[2].clone().into_uint().unwrap_or_default();
            let amount1_out: U256 = datos[3].clone().into_uint().unwrap_or_default();

            info!(
                "🔄 Swap en pool {:?} — amount0_in: {} | amount1_in: {} | amount0_out: {} | amount1_out: {}",
                log.address, amount0_in, amount1_in, amount0_out, amount1_out
            );
        }
    }
}
