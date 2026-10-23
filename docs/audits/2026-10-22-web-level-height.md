# Altura del mapa de niveles web · 2026-10-22

Al retirar el panel visual CVD, `bindLevelCardSpacing()` siguió midiendo la altura intrínseca de `.levels-route-design > .flow-column` para fijar `--level-overview-height`. La columna perdió el alto del panel retirado y el mapa de niveles se encogió, aunque su CSS tenía un alto base de 824 px. El cambio conserva el cálculo CVD y mantiene oculto su panel; suma al cálculo de la altura el espacio que ocupaba ese panel, con piso de 824 px. La reserva es de 166 px si la columna tiene más de 264 px de ancho y 180 px si es más estrecha, por el salto de línea del panel antiguo. La columna y los cuadros siguen desplazándose internamente si su contenido excede el espacio.

Se comparó la web publicada anterior con la compilación local usando Edge headless y una captura QQQ guardada, esperando un segundo después del primer render. Para medir el estado anterior, se insertó el HTML CVD antiguo **solo en el navegador temporal de prueba**; el producto no lo vuelve a insertar.

| Ancho de ventana | Columna de flujo | Web anterior sin CVD | Con panel antiguo en ensayo | Compilación corregida sin CVD | Cuadros visibles |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1920 px | 369 px | 701 px | 867 px | 867 px | 7/7 |
| 1440 px | 260 px | 742 px | 922 px | 922 px | 7/7 |
| 800 px | 494 px | 706 px por fila | 872 px por fila | 872 px por fila | 7/7 |

En los tres anchos, el extremo inferior del último cuadro quedó dentro de la pista de niveles (645/646 px, 680/681 px y 666/667 px respectivamente), sin recorte. TypeScript y Vite compilaron. Las 31 pruebas dirigidas pasaron. `git diff --check` pasó. No se alteraron fórmulas ni datos.

Comparación de la suite completa: antes del ajuste de altura, en `15943ad`, pasaron 233 archivos y fallaron 7 (1765 pruebas aprobadas, 3 fallidas, 5 omitidas). Con el ajuste `43b8327`, pasaron 233 archivos y fallaron los mismos 7 (1766 pruebas aprobadas, 3 fallidas, 5 omitidas); el test nuevo explica la prueba aprobada adicional. Los seis fallos por archivos históricos ausentes son `asset-watchlist`, `intraday-control`, `news-performance`, `route-replay`, `news-alert-real-replay` y `option-oi-profile`. `structure-study` sigue esperando `BRK.B` en el catálogo de 10 símbolos. Ninguno de esos tests, sus módulos ni las fuentes de fixture se modificaron en `15943ad..43b8327`. Los registros completos de ambas ejecuciones se guardaron localmente como `snipers-web-vitest-final-20261022.log` y `snipers-web-layout-suite-20261022.log`. Esta comparación acredita que el ajuste de altura no añadió fallos a la suite; no sustituye reparar los siete fallos de base.

Despliegue: `43b8327` llegó a `main` sin forzar. Vercel pasó al bundle `index-DqgkmJ_n.js`. En la web publicada, sin insertar ningún contenido de prueba, QQQ mostró siete de siete cuadros dentro de la pista: a 1920 px de ventana, mapa de 896 px y última tarjeta a 674/675 px de pista; a 1440 px, mapa de 951 px y última tarjeta a 709/710 px; a 800 px, mapa de 901 px por fila y última tarjeta a 695/696 px. El panel CVD no estaba en el DOM. Las alturas absolutas difieren del fixture local porque la captura de producción se actualizó entre mediciones; el mismo cálculo de reserva se verificó localmente contra el diseño anterior en los tres anchos.

Comprobación HTTP final: raíz, `/level-engine`, `assets/index-DqgkmJ_n.js` y `/api/frame?symbol=AAPL` respondieron 200. La raíz conservó la política `frame-ancestors` para los dominios autorizados y no devolvió `X-Frame-Options`.
