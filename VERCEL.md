# Desplegar solamente la web en Vercel

Importar `Albe401/Sniperslevelengine` y usar la **raíz del repositorio** como Root
Directory. `vercel.json` define un único proyecto web: interfaz Vite y una función
Node para la API de lectura. No activar la importación como varios servicios.

La configuración instala las dependencias de servidor y frontend, construye
`frontend/dist`, dirige `/api/*` a `api/index.mjs` y mantiene las demás rutas como
SPA. No ejecuta Rust, el escritor, bots ni el receptor del escritorio.

En **Settings → Environment Variables**, añadir para Production y Preview:

- `DATABASE_URL`: conexión de la nueva base Neon, como secreto del servidor.

La web abre directamente sin contraseña. Solo necesita DATABASE_URL en el servidor.
No usar prefijo VITE_ para secretos ni configurar claves de Massive.

La función lee solo el activo solicitado y verifica que el lote tenga los diez
activos completos. Las cadenas se transmiten comprimidas cuando el navegador
admite gzip. El navegador sigue actualizando la captura cada cinco minutos.

Después de añadir las variables, volver a desplegar la última revisión de `main`.
Si la importación anterior quedó detenida en detección multi-service, actualizar
la pantalla de importación y elegir un proyecto único con raíz `.`.

## Verificación

Las pruebas comprueban acceso público, bloqueo de escrituras, compresión y rutas.
El despliegue remoto debe confirmarse en Vercel.

Fuentes primarias: [Node Functions](https://vercel.com/docs/functions/runtimes/node-js),
[configuración](https://vercel.com/docs/project-configuration),
[límites](https://vercel.com/docs/functions/limitations).
