use ethers::signers::{LocalWallet, Signer};
use std::str::FromStr;
use tracing::info;

pub async fn crear_wallet(clave_privada: Option<String>) -> LocalWallet {
    match clave_privada {
        // Si ya tenemos clave privada guardada, usarla
        Some(clave) => {
            let wallet = LocalWallet::from_str(&clave).expect("Clave privada inválida");
            info!("🔑 Wallet cargada correctamente");
            info!("📬 Dirección: {:?}", wallet.address());
            wallet
        }
        // Si no, generar una nueva y mostrar la clave para guardarla
        None => {
            let wallet = LocalWallet::new(&mut rand::thread_rng());
            info!("🔑 Wallet nueva generada");
            info!("📬 Dirección: {:?}", wallet.address());
            info!(
                "⚠️  Guarda esta clave en .env como CLAVE_PRIVADA: 0x{}",
                hex::encode(wallet.signer().to_bytes())
            );
            wallet
        }
    }
}
