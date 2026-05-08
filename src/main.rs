// Módulos del bot
mod constructor;
mod coordinador;
mod detector;
mod evaluador;
mod firmante;
mod pools;
mod tokens;

use dotenv::dotenv;
use std::env;

#[tokio::main]
async fn main() {
    // Cargar variables de entorno
    dotenv().ok();

    // Inicializar sistema de logs
    tracing_subscriber::fmt::init();

    tracing::info!("🤖 CriptoBot iniciando...");

    // Crear wallet de desarrollo
    // Cargar clave privada si existe
    let clave_privada = env::var("CLAVE_PRIVADA").ok().filter(|s| !s.is_empty());

    // Crear wallet
    let wallet = firmante::crear_wallet(clave_privada).await;

    // Prueba de Telegram
    let tg_token = std::env::var("TELEGRAM_TOKEN").unwrap_or_default();
    let tg_chat = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
    coordinador::enviar_telegram(&tg_token, &tg_chat, "🤖 CriptoBot iniciado correctamente").await;

    // Obtener URL de Polygon del archivo .env
    let rpc_polygon = env::var("RPC_POLYGON").expect("RPC_POLYGON no encontrado en .env");

    // Iniciar el detector
    let rpc_amoy = env::var("RPC_MAINNET").expect("RPC_MAINNET no encontrado en .env");
    detector::iniciar(&rpc_polygon, &wallet, &rpc_amoy).await;
}
