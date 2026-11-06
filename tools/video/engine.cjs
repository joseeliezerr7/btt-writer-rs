// Grabador de videos de presentación de BTT Writer (adaptado del de Cerquín).
// Maneja la app en el navegador (vite + backend simulado) con Playwright,
// genera la narración con ElevenLabs y arma el MP4, los subtítulos .vtt, la
// portada .jpg y un reproductor HTML con capítulos.
'use strict';

const { chromium } = require('playwright');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const ROOT = __dirname;
const BASE = process.env.VIDEOS_BASE_URL || 'http://localhost:1420';
const VOICE = process.env.ELEVENLABS_VOICE || 'zl1Ut8dvwcVSuQSB9XkG'; // Ninoska (latinoamericana neutra)
const MODEL = 'eleven_multilingual_v2';
const W = 1280;
const H = 720;
const DRY = !!process.env.DRY;

// La clave de ElevenLabs se pasa por variable de entorno o se lee de
// `~/.elevenlabs-key` (fuera del repositorio); nunca va al repositorio.
const KEY_FILE = path.join(process.env.USERPROFILE || process.env.HOME || '', '.elevenlabs-key');
const KEY = (process.env.ELEVENLABS_API_KEY || (fs.existsSync(KEY_FILE) ? fs.readFileSync(KEY_FILE, 'utf8') : '')).trim();
if (!KEY && !DRY) {
    throw new Error('Falta ELEVENLABS_API_KEY (o corré con DRY=1 para ensayar sin voz).');
}
const TTS_DIR = path.join(ROOT, 'tts');
const OUT_DIR = path.join(ROOT, 'out');
fs.mkdirSync(TTS_DIR, { recursive: true });
fs.mkdirSync(OUT_DIR, { recursive: true });

let charsUsed = 0;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function duration(file) {
    const out = execFileSync('ffprobe', ['-v', 'error', '-show_entries', 'format=duration', '-of', 'csv=p=0', file]).toString().trim();
    return parseFloat(out);
}

async function tts(text) {
    if (DRY) {
        return { file: null, seconds: text.length / 13 };
    }
    const hash = crypto.createHash('sha1').update(VOICE + MODEL + text).digest('hex').slice(0, 16);
    const file = path.join(TTS_DIR, hash + '.mp3');
    if (!fs.existsSync(file)) {
        const res = await fetch(`https://api.elevenlabs.io/v1/text-to-speech/${VOICE}?output_format=mp3_44100_128`, {
            method: 'POST',
            headers: { 'xi-api-key': KEY, 'Content-Type': 'application/json' },
            body: JSON.stringify({
                text,
                model_id: MODEL,
                voice_settings: { stability: 0.55, similarity_boost: 0.75, style: 0.1, use_speaker_boost: true },
            }),
        });
        if (!res.ok) {
            throw new Error('ElevenLabs ' + res.status + ': ' + (await res.text()));
        }
        fs.writeFileSync(file, Buffer.from(await res.arrayBuffer()));
        charsUsed += text.length;
    }
    return { file, seconds: duration(file) };
}

// Cursor, clics, resaltado y tarjetas de título dibujados dentro de la página.
const OVERLAY = () => {
    if (window.__vidReady) return;
    window.__vidReady = true;
    const install = () => {
        const root = document.documentElement;
        const css = document.createElement('style');
        css.textContent = `
            #vid-cursor{position:fixed;left:0;top:0;width:26px;height:26px;z-index:2147483647;pointer-events:none;transition:transform .08s linear;filter:drop-shadow(0 2px 3px rgba(0,0,0,.35))}
            .vid-ripple{position:fixed;width:44px;height:44px;margin:-22px 0 0 -22px;border-radius:50%;border:3px solid #2563eb;z-index:2147483646;pointer-events:none;animation:vidr .55s ease-out forwards}
            @keyframes vidr{from{transform:scale(.3);opacity:1}to{transform:scale(1.5);opacity:0}}
            #vid-hl{position:fixed;z-index:2147483645;pointer-events:none;border:3px solid #2563eb;border-radius:12px;box-shadow:0 0 0 9999px rgba(15,23,42,.38);transition:all .35s ease;opacity:0}
            #vid-card{position:fixed;inset:0;z-index:2147483647;display:none;align-items:center;justify-content:center;flex-direction:column;background:radial-gradient(1200px 600px at 20% 10%,#1d4ed8 0%,#0f172a 55%,#020617 100%);color:#fff;font-family:Inter,system-ui,sans-serif;text-align:center;padding:0 80px}
            #vid-card h1{font-size:60px;margin:0;font-weight:800;letter-spacing:-.03em;line-height:1.1}
            #vid-card p{font-size:26px;margin:18px 0 0;color:#bfdbfe;max-width:900px;line-height:1.4}
            #vid-card .brand{position:absolute;bottom:44px;font-size:17px;color:#93c5fd;letter-spacing:.25em;text-transform:uppercase}
            #vid-card .logo{width:84px;height:84px;border-radius:22px;background:#2563eb;display:grid;place-items:center;margin-bottom:28px;box-shadow:0 20px 60px rgba(37,99,235,.45)}
            #vid-banner{position:fixed;left:50%;bottom:28px;transform:translateX(-50%) translateY(20px);z-index:2147483646;background:rgba(15,23,42,.92);color:#fff;font-family:Inter,system-ui,sans-serif;font-size:19px;padding:12px 22px;border-radius:14px;opacity:0;transition:all .3s ease;max-width:900px;text-align:center;box-shadow:0 10px 40px rgba(0,0,0,.35)}
            #vid-banner.on{opacity:1;transform:translateX(-50%) translateY(0)}
        `;
        root.appendChild(css);

        const cursor = document.createElement('div');
        cursor.id = 'vid-cursor';
        cursor.innerHTML = '<svg viewBox="0 0 24 24" width="26" height="26"><path d="M3 2l7 19 2.5-7.5L20 11z" fill="#0f172a" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>';
        root.appendChild(cursor);
        const hl = document.createElement('div');
        hl.id = 'vid-hl';
        root.appendChild(hl);
        const card = document.createElement('div');
        card.id = 'vid-card';
        card.innerHTML = '<div class="logo"><svg width="46" height="46" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"/><path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"/></svg></div><h1></h1><p></p><div class="brand">BTT Writer · Trabajo en equipo</div>';
        root.appendChild(card);
        const banner = document.createElement('div');
        banner.id = 'vid-banner';
        root.appendChild(banner);

        document.addEventListener('mousemove', (e) => {
            cursor.style.transform = `translate(${e.clientX - 3}px, ${e.clientY - 2}px)`;
        }, true);
        document.addEventListener('mousedown', (e) => {
            const r = document.createElement('div');
            r.className = 'vid-ripple';
            r.style.left = e.clientX + 'px';
            r.style.top = e.clientY + 'px';
            root.appendChild(r);
            setTimeout(() => r.remove(), 600);
        }, true);

        let target = null;
        const loop = () => {
            let el = null;
            try { el = target && document.querySelector(target); } catch { el = null; }
            if (el && el.getClientRects().length) {
                const b = el.getBoundingClientRect();
                Object.assign(hl.style, { opacity: 1, top: b.top - 6 + 'px', left: b.left - 6 + 'px', width: b.width + 12 + 'px', height: b.height + 12 + 'px' });
            } else {
                hl.style.opacity = 0;
            }
            requestAnimationFrame(loop);
        };
        loop();

        window.__vid = {
            highlight: (sel) => { target = sel; },
            highlightEl: (el) => { el.setAttribute('data-vid-target', '1'); target = '[data-vid-target="1"]'; },
            clear: () => { target = null; document.querySelectorAll('[data-vid-target]').forEach((e) => e.removeAttribute('data-vid-target')); },
            card: (title, sub) => {
                card.querySelector('h1').textContent = title;
                card.querySelector('p').textContent = sub || '';
                card.style.display = title ? 'flex' : 'none';
            },
            banner: (text) => { banner.textContent = text || ''; banner.classList.toggle('on', !!text); },
        };
    };
    if (document.documentElement) install();
    else document.addEventListener('DOMContentLoaded', install);
};

function helpers(page) {
    const center = async (sel) => {
        const loc = page.locator(sel).first();
        await loc.waitFor({ state: 'visible', timeout: 15000 });
        await loc.scrollIntoViewIfNeeded();
        const b = await loc.boundingBox();
        return { x: b.x + b.width / 2, y: b.y + b.height / 2, loc };
    };
    const h = {
        page,
        sleep,
        async move(sel) {
            const c = await center(sel);
            await page.mouse.move(c.x, c.y, { steps: 22 });
            return c;
        },
        async click(sel, { settle = 700 } = {}) {
            await h.move(sel);
            await sleep(180);
            await page.mouse.down();
            await sleep(70);
            await page.mouse.up();
            await sleep(settle);
        },
        async type(sel, text, { delay = 55, clear = true } = {}) {
            await h.click(sel, { settle: 150 });
            if (clear) await page.keyboard.press('Control+A');
            await page.keyboard.type(text, { delay });
            await sleep(250);
        },
        async press(key) {
            await page.keyboard.press(key);
            await sleep(500);
        },
        async highlight(sel, ms) {
            const loc = page.locator(sel).first();
            await loc.waitFor({ state: 'visible', timeout: 15000 });
            await loc.scrollIntoViewIfNeeded();
            await loc.evaluate((el) => window.__vid.highlightEl(el));
            if (ms) { await sleep(ms); await h.clear(); }
        },
        async clear() {
            await page.evaluate(() => window.__vid.clear());
        },
        async card(title, sub) {
            await page.evaluate(([t, s]) => window.__vid.card(t, s), [title, sub]);
        },
        async banner(text) {
            await page.evaluate((t) => window.__vid.banner(t), text || '');
        },
        async scrollTo(sel) {
            await page.locator(sel).first().scrollIntoViewIfNeeded();
            await sleep(500);
        },
        async waitFor(sel, timeout = 15000) {
            await page.locator(sel).first().waitFor({ state: 'visible', timeout });
        },
    };
    return h;
}

function vttTime(s) {
    const ms = Math.max(0, Math.round(s * 1000));
    const hh = String(Math.floor(ms / 3600000)).padStart(2, '0');
    const mm = String(Math.floor((ms % 3600000) / 60000)).padStart(2, '0');
    const ss = String(Math.floor((ms % 60000) / 1000)).padStart(2, '0');
    const mmm = String(ms % 1000).padStart(3, '0');
    return `${hh}:${mm}:${ss}.${mmm}`;
}

function chunks(text) {
    const sentences = text.match(/[^.!?;:]+[.!?;:]?/g).map((s) => s.trim()).filter(Boolean);
    const out = [];
    for (const s of sentences) {
        if (s.length <= 95) { out.push(s); continue; }
        let cur = '';
        for (const w of s.split(' ')) {
            if ((cur + ' ' + w).trim().length > 80) { out.push(cur.trim()); cur = ''; }
            cur += ' ' + w;
        }
        if (cur.trim()) out.push(cur.trim());
    }
    return out;
}

/** Reproductor HTML con capítulos (se abre en cualquier navegador junto al MP4). */
function playerHtml(video, chapters, seconds) {
    const items = chapters.map((c) => `<li><button data-t="${c.start.toFixed(1)}"><span class="t">${vttTime(c.start).slice(3, 8)}</span><span>${c.title}</span></button></li>`).join('\n');
    return `<!doctype html>
<html lang="es"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>${video.title} · BTT Writer</title>
<style>
:root{color-scheme:light dark;--bg:#0b1220;--panel:#111a2e;--text:#e5edff;--muted:#8fa3c7;--accent:#3b82f6}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font-family:Inter,system-ui,sans-serif}
.wrap{max-width:1180px;margin:0 auto;padding:24px 16px;display:grid;gap:18px;grid-template-columns:minmax(0,1fr) 300px}
@media(max-width:900px){.wrap{grid-template-columns:1fr}}
h1{font-size:22px;margin:0 0 4px}p.sub{color:var(--muted);margin:0 0 14px}
video{width:100%;border-radius:14px;background:#000;box-shadow:0 20px 60px rgba(0,0,0,.5)}
aside{background:var(--panel);border-radius:14px;padding:14px}aside h2{font-size:14px;letter-spacing:.15em;text-transform:uppercase;color:var(--muted);margin:0 0 10px}
ul{list-style:none;margin:0;padding:0;display:grid;gap:6px}li button{width:100%;display:flex;gap:10px;align-items:center;background:none;border:1px solid transparent;color:var(--text);padding:9px 10px;border-radius:10px;cursor:pointer;text-align:left;font:inherit}
li button:hover{background:rgba(59,130,246,.12)}li button.active{border-color:var(--accent);background:rgba(59,130,246,.18)}.t{font-variant-numeric:tabular-nums;color:var(--accent);min-width:42px}
.actions{display:flex;gap:8px;margin-top:12px;flex-wrap:wrap}.actions a{color:var(--text);text-decoration:none;border:1px solid #2a3a5c;padding:8px 12px;border-radius:10px;font-size:14px}
</style></head><body><div class="wrap">
<main><h1>${video.title}</h1><p class="sub">${video.summary}</p>
<video id="v" controls playsinline poster="${video.key}.jpg"><source src="${video.key}.mp4" type="video/mp4"><track kind="subtitles" srclang="es" label="Español" src="${video.key}.vtt" default></video>
<div class="actions"><a href="${video.key}.mp4" download>Descargar MP4</a><a href="${video.key}.vtt" download>Subtítulos</a></div></main>
<aside><h2>Capítulos · ${Math.round(seconds / 60)} min</h2><ul id="ch">
${items}
</ul></aside></div>
<script>
const v=document.getElementById('v'),b=[...document.querySelectorAll('#ch button')];
b.forEach(x=>x.onclick=()=>{v.currentTime=+x.dataset.t;v.play();});
v.addEventListener('timeupdate',()=>{let cur=null;for(const x of b){if(v.currentTime>=+x.dataset.t-0.2)cur=x;}b.forEach(x=>x.classList.toggle('active',x===cur));});
</script></body></html>`;
}

async function record(video) {
    console.log(`\n=== ${video.key}: ${video.title}`);
    const audio = [];
    for (const scene of video.scenes) audio.push(await tts(scene.say));
    const total = audio.reduce((a, b) => a + b.seconds, 0);
    console.log(`  narración: ${total.toFixed(1)} s, caracteres nuevos: ${charsUsed}`);

    const browser = await chromium.launch();
    const tmpDir = path.join(OUT_DIR, 'tmp-' + video.key);
    fs.rmSync(tmpDir, { recursive: true, force: true });
    const ctx = await browser.newContext({
        viewport: { width: W, height: H },
        recordVideo: { dir: tmpDir, size: { width: W, height: H } },
        locale: 'es-419',
        colorScheme: video.colorScheme || 'light',
    });
    await ctx.addInitScript(OVERLAY);
    const page = await ctx.newPage();
    page.on('dialog', (d) => d.accept());
    const h = helpers(page);
    const tCtx = Date.now();

    await page.goto(BASE + (video.start || '/'), { waitUntil: 'networkidle' });
    // Preparación fuera del video (se recorta al montar): alta de usuario, etc.
    if (video.setup) await video.setup(h);
    const skip = (Date.now() - tCtx) / 1000;
    const t0 = Date.now();

    const starts = [];
    for (let i = 0; i < video.scenes.length; i++) {
        const scene = video.scenes[i];
        const start = (Date.now() - t0) / 1000;
        starts.push(start);
        process.stdout.write(`  escena ${i + 1}/${video.scenes.length} (${audio[i].seconds.toFixed(1)} s)\n`);
        if (scene.do) {
            try {
                await scene.do(h);
            } catch (e) {
                await page.screenshot({ path: path.join(OUT_DIR, `${video.key}-error-escena${i + 1}.png`) }).catch(() => {});
                throw new Error(`escena ${i + 1} («${scene.say.slice(0, 40)}…»): ${e.message.split(String.fromCharCode(10))[0]}`);
            }
        }
        const elapsed = (Date.now() - t0) / 1000 - start;
        const wait = audio[i].seconds + (scene.pause ?? 0.6) - elapsed;
        if (wait > 0) await sleep(wait * 1000);
    }
    await sleep(800);
    const end = (Date.now() - t0) / 1000;

    await page.close();
    const webm = await page.video().path();
    await ctx.close();
    await browser.close();

    const chapters = video.scenes.map((s, i) => (s.chapter ? { title: s.chapter, start: starts[i] } : null)).filter(Boolean);

    if (DRY) {
        const sheet = path.join(OUT_DIR, video.key + '-ensayo.jpg');
        execFileSync('ffmpeg', ['-y', '-v', 'error', '-ss', skip.toFixed(2), '-i', webm, '-vf', `fps=1/${Math.max(3, Math.round(end / 20))},scale=320:-1,tile=5x4`, '-frames:v', '1', sheet]);
        fs.rmSync(tmpDir, { recursive: true, force: true });
        console.log(`  ensayo listo (${end.toFixed(1)} s): ${sheet}`);
        console.log('  capítulos:', chapters.map((c) => `${vttTime(c.start).slice(3, 8)} ${c.title}`).join(' | '));
        return { key: video.key, seconds: Math.round(end) };
    }

    const outMp4 = path.join(OUT_DIR, video.key + '.mp4');
    const args = ['-y', '-ss', skip.toFixed(2), '-i', webm];
    audio.forEach((a) => args.push('-i', a.file));
    const delays = audio.map((a, i) => `[${i + 1}]adelay=${Math.round(starts[i] * 1000)}|${Math.round(starts[i] * 1000)}[a${i}]`).join(';');
    const mix = audio.map((_, i) => `[a${i}]`).join('') + `amix=inputs=${audio.length}:normalize=0[a]`;
    args.push(
        '-filter_complex', `${delays};${mix}`,
        '-map', '0:v', '-map', '[a]',
        '-t', end.toFixed(2),
        '-c:v', 'libx264', '-preset', 'medium', '-crf', '22', '-pix_fmt', 'yuv420p', '-r', '25',
        '-c:a', 'aac', '-b:a', '160k', '-movflags', '+faststart',
        outMp4,
    );
    execFileSync('ffmpeg', args, { stdio: ['ignore', 'ignore', 'pipe'] });

    let vtt = 'WEBVTT\n\n';
    video.scenes.forEach((scene, i) => {
        const parts = chunks(scene.say);
        const len = parts.reduce((a, p) => a + p.length, 0);
        let t = starts[i];
        parts.forEach((p) => {
            const d = audio[i].seconds * (p.length / len);
            vtt += `${vttTime(t)} --> ${vttTime(t + d)}\n${p}\n\n`;
            t += d;
        });
    });
    fs.writeFileSync(path.join(OUT_DIR, video.key + '.vtt'), vtt);
    execFileSync('ffmpeg', ['-y', '-ss', '1.5', '-i', outMp4, '-frames:v', '1', '-q:v', '3', path.join(OUT_DIR, video.key + '.jpg')], { stdio: 'ignore' });
    fs.writeFileSync(path.join(OUT_DIR, video.key + '.html'), playerHtml(video, chapters, end));
    fs.rmSync(tmpDir, { recursive: true, force: true });
    console.log(`  listo: ${outMp4} (${end.toFixed(1)} s)`);
    return { key: video.key, seconds: Math.round(end) };
}

module.exports = { record, tts, get charsUsed() { return charsUsed; } };
