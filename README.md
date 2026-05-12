# criptoBot

## Escaner V2

El bot arranca el detector por eventos y un escaner periodico de reservas V2.
El escaner puede comparar varios pools V2 del mismo par sin cambiar el contrato.
Ahora monitoriza:

- WPOL/USDT
- WPOL/USDC
- WPOL/DAI

Variables utiles:

```env
RUST_LOG=info
UMBRAL_PORCENTAJE=0.8
MIN_PROFIT_USDT=0
MULTIPLICADOR_MARGEN_LOCAL=1
MIN_PROFIT_NETO_USDT=0
ESCANER_V2_ACTIVO=true
ESCANER_V2_EJECUTAR=true
ESCANER_V2_SEGUNDOS=5
ESCANER_V2_COOLDOWN_SEGUNDOS=60
GAS_ARBITRAJE_V2=550000
```

El escaner solo ejecuta si encuentra beneficio bruto positivo, estima gas,
calcula beneficio neto y supera `MIN_PROFIT_NETO_USDT`. Con
`MIN_PROFIT_NETO_USDT=0`, solo ejecuta si el neto estimado es mayor que cero.

Con `RUST_LOG=info` se ocultan descartes normales y swaps individuales. Para
investigar ruido fino, usar `RUST_LOG=debug`.

Las ganancias no se retiran automaticamente a la wallet en cada arbitraje. Se
acumulan en el contrato para evitar gastar gas en retiros pequenos; el retiro se
hace manualmente cuando compense.
