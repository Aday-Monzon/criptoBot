// Importaciones necesarias para conectarnos a la blockchain
use ethers::providers::{Provider, Ws};
use tracing::info;

// Función principal del detector — asíncrona porque usa WebSocket
pub async fn iniciar(rpc_polygon: &str) {
    info!("📡 Conectando detector a Polygon...");

    // Intentar conectar vía WebSocket
    let proveedor = Provider::<Ws>::connect(rpc_polygon).await;

    // Verificar si la conexión fue exitosa
    match proveedor {
        Ok(_) => info!("✅ Detector conectado a Polygon correctamente"),
        Err(e) => info!("❌ Error al conectar: {}", e),
    }
}
