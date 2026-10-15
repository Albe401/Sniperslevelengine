# Verificación

Construcción de producción y TypeScript comprobados. Pasaron cinco pruebas de
servidor, diez del escritor de escritorio y 57 pruebas focalizadas del frontend
en el entorno local, incluidas comprobaciones con datos reales identificados.
La comparación independiente de diez activos y diez cadenas contra Neon fue
exacta. El navegador mostró dos secciones, cero fotos y cero peticiones externas.
No constituye certificación de todos los ticks ni precisión predictiva.

Los datos reales, capturas y logs de esas pruebas se conservan localmente y no
se publican en este repositorio. Las pruebas de datos reales del heatmap quedan
omitidas si no se proporciona `verification/real-option-chain-source.json`.
Las pruebas sintéticas permanecen ejecutables con `npm test` en el servidor y
`npm test` dentro de `frontend`. Los antiguos fixtures de mercado usados por
las pruebas Rust de referencia tampoco se publican.

Se conservan como casos corregidos: rutas de fixtures heredadas, tres alertas
de dependencias de pruebas (resueltas con actualización), y timeout al enviar
cadenas completas en JSONB sin comprimir. El archivo sin pérdida por activo
corrige el tamaño de entrega; se comprobaron sus hashes independientemente.
