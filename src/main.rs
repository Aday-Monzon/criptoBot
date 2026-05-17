// Módulos del bot
mod coordinador;
mod detector;
mod evaluador;
mod firmante;
mod pools;
mod rpc;

use dotenv::dotenv;
use std::env;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Cargar variables de entorno
    dotenv().ok();

    // Inicializar sistema de logs
    let filtro = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filtro).init();

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

    let rpc_polygon = rpc::urls_env(
        "RPC_POLYGON",
        "RPC_POLYGON_FALLBACKS",
        &[
            "https://polygon-bor-rpc.publicnode.com",
            "https://polygon-rpc.com",
        ],
    );

    // Iniciar detector por eventos y escaner periodico por reservas
    let rpc_mainnet_urls = rpc::urls_env(
        "RPC_MAINNET",
        "RPC_MAINNET_FALLBACKS",
        &[
            "https://polygon-bor-rpc.publicnode.com",
            "https://polygon-rpc.com",
        ],
    );
    let rpc_amoy = rpc::seleccionar_http(&rpc_mainnet_urls)
        .await
        .expect("Ningun RPC_MAINNET respondio");

    tokio::join!(
        detector::iniciar(rpc_polygon, &wallet, &rpc_amoy),
        coordinador::iniciar_escaner_v2(&rpc_amoy, &wallet),
        coordinador::iniciar_escaner_triangular_v2(&rpc_amoy, &wallet)
    );
}
