const engine = require('./engine.cjs');
(async () => {
    const keys = process.argv.slice(2);
    if (!keys.length) {
        console.error('uso: node run.cjs <guion> [...]   (guiones en scripts/)');
        process.exit(2);
    }
    for (const k of keys) {
        const video = require('./scripts/' + k + '.cjs');
        await engine.record(video);
    }
    console.log('caracteres usados en esta corrida:', engine.charsUsed);
})().catch((e) => { console.error('ERROR', e.message); process.exit(1); });
