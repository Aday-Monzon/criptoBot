// Importaciones necesarias
use ethers::{
    providers::{Middleware, Provider, Ws},
    types::{Address, Filter, H256},
};
use futures_util::StreamExt;
use std::sync::Arc;
use tracing::info;

// Topic del evento Swap en Uniswap V2
// keccak256("Swap(address,uint256,uint256,uint256,uint256,address)")
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
    info!("👂 Escuchando eventos de swap en QuickSwap...");

    // Topic del evento Swap
    let topic: H256 = TOPIC_SWAP.parse().expect("Topic inválido");

    // Crear filtro para escuchar solo eventos Swap de ese pool
    // Crear filtro para escuchar swaps de CUALQUIER pool
    let filtro = Filter::new().topic0(topic);
    // Suscribirse a los logs
    let mut stream = proveedor
        .subscribe_logs(&filtro)
        .await
        .expect("Error al suscribirse a logs");

    // Procesar cada evento de swap
    while let Some(log) = stream.next().await {
        info!(
            "🔄 Swap detectado — bloque: {:?} — tx: {:?}",
            log.block_number, log.transaction_hash
        );
    }
}
