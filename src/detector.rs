// Importaciones necesarias para conectarnos y escuchar la blockchain
use ethers::providers::{Middleware, Provider, Ws};
use futures_util::StreamExt;
use tracing::info;

// Función principal del detector — asíncrona porque usa WebSocket
pub async fn iniciar(rpc_polygon: &str) {
    info!("📡 Conectando detector a Polygon...");

    // Conectar vía WebSocket
    let proveedor = Provider::<Ws>::connect(rpc_polygon)
        .await
        .expect("Error al conectar a Polygon");

    info!("✅ Detector conectado a Polygon correctamente");
    info!("👂 Escuchando bloques nuevos...");

    // Suscribirse a los bloques nuevos
    let mut stream = proveedor
        .subscribe_blocks()
        .await
        .expect("Error al suscribirse a bloques");

    // Procesar cada bloque que llegue
    while let Some(bloque) = stream.next().await {
        info!(
            "🟣 Bloque nuevo: #{} — {} transacciones",
            bloque.number.unwrap_or_default(),
            bloque.transactions.len()
        );
    }
}
