# Snipers · Web de snapshots JSON

La web tiene únicamente **Lectura rápida completa** y **Radar de inversión →
Cadena de opciones**. Utiliza los componentes originales, con sus colores y
gráfica interactiva. No utiliza fotos. No conecta a Massive, no ejecuta bots y
no genera órdenes. Lee exclusivamente los datos guardados en PostgreSQL/Neon.

## Flujo

1. Snipers de escritorio conserva su recepción segundo a segundo.
2. Cada cinco minutos, el escritor del escritorio conserva un lote atómico de
   AAPL, MSFT, AMZN, GOOGL, META, NVDA, TSLA, QQQ, SPY e IWM en la nueva base.
3. Solo se guardan los campos necesarios de Lectura rápida y la cadena de
   opciones; no se recopilan noticias, estudios generales ni aprendizaje para
   esta web. El escritorio sigue usando sus demás motores independientemente.
4. La web lee el último lote completo y actualiza cada cinco minutos. No añade
   otros cinco minutos de espera. Al cambiar un activo, lee el mismo proveedor
   de datos guardados. Los controles de gráfica y filtros usan el JSON guardado.
5. Fechas, cobertura y datos pendientes permanecen visibles. La cadena completa
   puede estar pendiente en un lote hasta que termine la consulta del escritorio.
   La web nunca completa huecos mediante consultas al mercado.

No se modificaron fórmulas: los renderizadores originales usan los datos del
lote y su reloj de captura. No confundir fuerzas o coberturas con probabilidades
calibradas. El código de los demás motores se conserva como referencia en
`frontend/src` y `engine`, pero no son secciones ni servicios de la web.

## Ejecutar y desplegar

Node 22 o superior:

```sh
npm ci
npm --prefix frontend ci
npm run build
npm test
node --env-file=.env server/server.mjs
```

Configurar `.env` siguiendo `.env.example`. `DATABASE_URL` queda exclusivamente
en el servidor: no se necesita clave de Massive en la web. Para acceso externo,
`HOST=0.0.0.0` exige `SNIPERS_WEB_PASSWORD`. Servir detrás de HTTPS y usar
`SNIPERS_SECURE_COOKIE=1`. Nunca subir `.env`, contraseñas ni datos locales.

El Dockerfile permite desplegar el servidor. GitHub almacena el código;
GitHub Pages por sí solo no ejecuta la API Node. Este repositorio no implica un
dominio público ya desplegado.

## Escritor del escritorio

`desktop-writer` conserva la implementación de referencia y sus pruebas. Está
integrada en el escritorio de origen; no se ejecuta dentro del servidor web.
La conexión nueva local está cifrada fuera del repositorio. En otro ordenador
se necesita configurar las rutas de runtimes, la credencial protegida y el
receptor original. El esquema se encuentra en `server/snapshot-schema.sql`.

`batches` identifica el lote y sus fechas; `assets` conserva el JSON comprimido
sin pérdida, SHA-256 y cobertura por campo. Un lote requiere los diez activos.
Las pruebas verifican el recálculo de hashes y la lectura real de Neon. Pueden
existir imágenes históricas de la prueba anterior en la base; esta web no las
lee ni las genera, y los lotes nuevos destinados a la web no crean fotos.

Validación y límites: `TESTING.md`. Evidencia y datos reales permanecen localmente
en `verification/`, excluidos de Git. No certifica todos los ticks de mercado
ni la precisión predictiva. Las secciones sin datos se mantienen pendientes.

## Fuentes primarias

- [Controlador Neon](https://neon.com/docs/serverless/serverless-driver)
- [Construcción Vite](https://vite.dev/guide/build)
- [Servidor HTTP Node](https://nodejs.org/api/http.html)
- [Lightweight Charts](https://tradingview.github.io/lightweight-charts/)
