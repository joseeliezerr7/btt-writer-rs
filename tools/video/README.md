# Videos de presentación

Grabador de videos cortos de BTT Writer (adaptado del de Cerquín). Cada guion de
`scripts/` es una lista de escenas: lo que dice la narración y lo que se hace en
pantalla. El grabador maneja la app en el navegador con Playwright, genera la voz
con ElevenLabs (voz «Ninoska», latinoamericana neutra) y arma con ffmpeg el MP4,
los subtítulos `.vtt`, la portada `.jpg` y un reproductor `.html` con capítulos.

## Requisitos

- Node 20+, ffmpeg y ffprobe en el PATH.
- `npm install` en esta carpeta (Playwright 1.48; el navegador con
  `npx playwright install chromium` si no está).
- La app corriendo en el navegador con el backend simulado: `npm run dev` en
  `apps/desktop` (puerto 1420). El simulado ya trae a «ana» como compañera, el
  chunk 1:5-8 reservado por ella y un cambio remoto en 1:11-12.

## Grabar

Ensayo sin voz ni costo (deja la hoja de cuadros en `out/{video}-ensayo.jpg`):

```bash
DRY=1 node run.cjs equipo
```

Grabación real. La clave de ElevenLabs va en la variable `ELEVENLABS_API_KEY` o
en el archivo `~/.elevenlabs-key` (fuera del repositorio). La voz se guarda en
`tts/` por texto: regrabar sin cambiar la narración no gasta caracteres.

```bash
node run.cjs equipo
```

Salida en `out/`: `equipo.mp4`, `equipo.vtt`, `equipo.jpg` y `equipo.html`
(reproductor con capítulos; se abre junto a los otros tres archivos).
