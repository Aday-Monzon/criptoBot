// Módulos del bot
mod constructor;
mod coordinador;
mod detector;
mod evaluador;
mod firmante;

// Runtime asíncrono
#[tokio::main]
async fn main() {
    // Inicializar sistema de logs
    tracing_subscriber::fmt::init();

    tracing::info!("🤖 CriptoBot iniciando...");
    tracing::info!("📡 Conectando a Polygon y Base...");
}
