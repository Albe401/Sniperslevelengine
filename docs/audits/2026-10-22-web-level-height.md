# Altura del mapa de niveles web · 2026-10-22

Al retirar el panel visual CVD, `bindLevelCardSpacing()` siguió midiendo la altura intrínseca de `.levels-route-design > .flow-column` para fijar `--level-overview-height`. La columna perdió el alto del panel retirado y el mapa de niveles se encogió, aunque su CSS tenía un alto base de 824 px. El cambio conserva el cálculo CVD y mantiene oculto su panel; suma al cálculo de la altura el espacio que ocupaba ese panel, con piso de 824 px. La reserva es de 166 px si la columna tiene más de 264 px de ancho y 180 px si es más estrecha, por el salto de línea del panel antiguo. La columna y los cuadros siguen desplazándose internamente si su contenido excede el espacio.

Se comparó la web publicada anterior con la compilación local usando Edge headless y una captura QQQ guardada, esperando un segundo después del primer render. Para medir el estado anterior, se insertó el HTML CVD antiguo **solo en el navegador temporal de prueba**; el producto no lo vuelve a insertar.

| Ancho de ventana | Columna de flujo | Web anterior sin CVD | Con panel antiguo en ensayo | Compilación corregida sin CVD | Cuadros visibles |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1920 px | 369 px | 701 px | 867 px | 867 px | 7/7 |
| 1440 px | 260 px | 742 px | 922 px | 922 px | 7/7 |
| 800 px | 494 px | 706 px por fila | 872 px por fila | 872 px por fila | 7/7 |

En los tres anchos, el extremo inferior del último cuadro quedó dentro de la pista de niveles (645/646 px, 680/681 px y 666/667 px respectivamente), sin recorte. TypeScript y Vite compilaron. Las 31 pruebas dirigidas pasaron; la suite completa quedó en 233 archivos aprobados y los mismos 7 fallos ajenos/preexistentes documentados en `2026-10-22-web-snapshot-oi-cvd.md` (1766 pruebas aprobadas, 3 fallidas, 5 omitidas). `git diff --check` pasó. No se alteraron fórmulas ni datos.
