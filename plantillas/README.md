# Plantillas de flujos

Flujos de ejemplo para las pantallas que casi toda app tiene: inicio de sesión, registro, recuperar contraseña, secciones con la sesión iniciada y un pago de prueba. Están escritos para una app ficticia (`com.ejemplo.app`): copia la carpeta, cambia el `appId` y los textos por los de tu app.

| Archivo | Qué prueba | ¿Entra en "Ejecutar todas"? |
|---|---|---|
| `01-inicio-sesion-validacion.yaml` | Entrar sin datos muestra un aviso y no avanza | Sí |
| `02-registro-validaciones.yaml` | Campos obligatorios, correo con formato inválido y contraseña débil | Sí |
| `03-recuperar-contrasena.yaml` | Pide el correo y permite volver | Sí |
| `04-registro.yaml` | Crea una cuenta nueva | No (`requiere-datos`) |
| `05-secciones-con-sesion.yaml` | Pestañas principales con la sesión iniciada | Sí, si defines la cuenta de pruebas |
| `06-pago-tarjeta-de-prueba.yaml` | Agregar una tarjeta de prueba y pagar en el ambiente sandbox | No (`requiere-datos`) |
| `comun/abrir-app.yaml` | Paso compartido: abre la app limpia, con permisos | No (se usa con `runFlow`) |
| `comun/iniciar-sesion.yaml` | Paso compartido: inicia sesión solo si hace falta | No (se usa con `runFlow`) |
| `config.yaml` | Qué corre "Ejecutar todas" | — |

## Datos de prueba

Ningún archivo trae cuentas, contraseñas ni tarjetas: las variables están vacías y cada flujo que las necesita se detiene en su primer paso si faltan. Define los valores en el selector **Env** de la barra de ejecución o en una copia local que no subas al repositorio. Usa siempre una cuenta de pruebas y, para pagos, solo las tarjetas de prueba que publica tu proveedor y su ambiente sandbox.

## Lo que aprendimos al probar apps reales

- **Escapa los caracteres especiales.** El texto de `tapOn` y `assertVisible` se busca como expresión regular: un teléfono de ejemplo como `+52 55…` se escribe `"\\+52 55…"`. Al revés, `".*"` sirve para textos largos: `"Formato de correo inválido.*"`.
- **No uses `hideKeyboard` en Android.** Presiona "Atrás" y, en la primera pantalla, cierra la app. Para quitar el teclado, toca un texto que no haga nada, como el título.
- **Un campo de contraseña vacío puede no existir como elemento.** Si solo muestra puntos de ejemplo, `tapOn: { below: "Contraseña" }` cae en el icono del ojo y lo que escribes termina en el campo anterior. Pídele al equipo de la app un `testID` (`campo-contrasena` en estas plantillas) o tócalo por posición (`point: "40%,48%"`).
- **Los enlaces dentro de una frase solo responden en su parte.** En "¿No tienes una cuenta? Crear cuenta", tocar el centro de la frase no abre el registro: toca "Crear cuenta" o su posición.
- **Espera a que pase la animación de entrada** antes de tocar nada (`extendedWaitUntil` en `comun/abrir-app.yaml`).
- **Fija la ubicación** con `setLocation` en pantallas con mapa, para que el resultado no dependa de dónde está el simulador.
