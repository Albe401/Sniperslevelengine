# Actividad visual en el mapa de niveles

Los cuadros muestran tres fuentes independientes sin cambiar puntuaciones estructurales, OI, niveles, cálculos del motor ni condiciones de absorción:

* Cian: aceptación AMT del perfil tick existente, mediante `acceptanceIntensity`. Solo sus categorías existentes Alta/Media/Baja; no probabilidad.
* Rojo: ventas inferidas al BID en acciones/ETF, cantidad de acciones observadas en 60 segundos dentro del intervalo exacto de un centavo `[lower, upper)` que contiene el nivel. No asigna CVD global ni el bucket más cercano a un strike.
* Ámbar: ventas inferidas al BID de opciones, contratos observados en cinco minutos para el strike y todos los vencimientos seleccionados, mediante `barrierFlow`. PUT no equivale a venta.

Por fuente, la intensidad visual es `valor / máximo positivo` entre cuadros con datos comparables. La estrella identifica ese máximo; los empates comparten estrella. No compara acciones con contratos, no mide crecimiento frente a la muestra anterior y no cambia la fuerza estructural. El interior del cuadro y cada fila tienen variables propias; se conservan borde exterior, títulos y conectores.

Las fuentes conservan reglas separadas de vigencia y cobertura. La lectura de acciones requiere ventana completa, cadencia RTH reciente y zonas causales; se informa UNKNOWN, clasificación y zonas excluidas. La aceptación exige también cadencia reciente del subyacente porque `asOfMs` puede avanzar después del cierre. Los datos ausentes o vencidos se muestran como `—`, sin máximo ni resplandor. Cero observado sigue siendo cero.

La web evalúa capturas al reloj guardado y las identifica como tales. Los tooltips contienen cantidades completas, unidades, ventanas y límites de la muestra. Las cantidades compactas y las abreviaturas estrechas son solo presentación.

Validación: 58 pruebas pertinentes en siete archivos, TypeScript y compilación Vite aprobados. Incluye independencia de ganadores, empates, cero/ausencia, activo equivocado, relojes futuros/vencidos, cobertura, límites adyacentes, capturas, conservación de scores y separación de styles interior/exterior. Vista previa sintética identificada, comprobada a cuadros de 150 y 100 px; captura vencida elimina todos los resaltados. Estas pruebas validan la presentación; no certifican cobertura de todo el mercado ni capacidad predictiva de los motores.
