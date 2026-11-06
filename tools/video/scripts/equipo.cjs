// Presentación del trabajo en equipo (repositorio compartido, sincronización
// automática y reservas de chunk). Se graba contra la app en el navegador con
// el backend simulado (`npm run dev` en apps/desktop), que ya incluye a «ana»
// como compañera con el chunk 1:1-4 reservado y un cambio remoto en 1:11-12.

const card = (title) => `.chunk-card:has(.pane-head .title:text-is("${title}"))`;

module.exports = {
    key: 'equipo',
    title: 'Trabajo en equipo en BTT Writer',
    summary: 'Varias personas traducen el mismo libro a la vez, cada una desde su dispositivo, con sincronización automática y sin perder trabajo.',
    start: '/',
    // Alta con cuenta del servidor (fuera del video).
    async setup(h) {
        await h.page.locator('text=Continuar').first().click();
        await h.page.locator('input:not([type=password])').first().fill('maria');
        await h.page.locator('input[type=password]').first().fill('demo');
        await h.page.locator('button:has-text("Continuar")').first().click();
        // Avisos que siguen (privacidad, términos): se aceptan los que aparezcan.
        for (let i = 0; i < 8; i++) {
            if (await h.page.locator('.project-row').count()) break;
            const btn = h.page.locator('button:has-text("Acepto"), button:has-text("Continuar")').first();
            if (await btn.count() && await btn.isEnabled()) {
                await btn.click();
            }
            await h.sleep(700);
        }
        await h.waitFor('.project-row');
        await h.sleep(600);
    },
    scenes: [
        {
            chapter: 'Introducción',
            say: 'BTT Writer ahora permite trabajar en equipo. Varias personas pueden traducir el mismo libro al mismo tiempo, cada una desde su propio dispositivo, y la aplicación se encarga de unir el trabajo de todos sin que nadie pierda nada.',
            do: async (h) => { await h.card('Trabajo en equipo', 'Traduzcan el mismo libro a la vez, cada uno desde su dispositivo'); },
        },
        {
            say: 'Todo empieza en la pantalla de inicio. Abra el proyecto que quiere compartir con su equipo.',
            do: async (h) => {
                await h.card('');
                await h.sleep(900);
                await h.highlight('.project-row:has-text("Chayahuita")', 1800);
                await h.click('.project-row:has-text("Chayahuita")', { settle: 1800 });
            },
        },
        {
            chapter: 'Activar el equipo',
            say: 'En la barra superior aparece el botón Equipo. Al abrirlo, la aplicación explica cómo funciona: todos comparten un mismo repositorio en WACS, cada chunk es independiente, y antes de cada fusión se guarda una copia de respaldo.',
            do: async (h) => {
                await h.highlight('.topbar button:has-text("Equipo")', 1600);
                await h.click('.topbar button:has-text("Equipo")', { settle: 1200 });
                await h.highlight('.collab-how', 4500);
            },
        },
        {
            say: 'Añada a sus compañeros por su nombre de usuario de WACS. Basta con escribir las primeras letras y elegirlo de la lista. Cada compañero recibe permiso para escribir en el proyecto.',
            do: async (h) => {
                await h.type('input[placeholder="usuario de WACS"]', 'be', { delay: 120 });
                await h.sleep(900);
                await h.click('.menu.static button:has-text("beto")', { settle: 900 });
                await h.highlight('.member-row', 2000);
            },
        },
        {
            say: 'Al pulsar Activar equipo, se crea el repositorio compartido en el servidor y comienza la sincronización automática. Desde ese momento, mientras el proyecto esté abierto, la aplicación guarda, baja los cambios de los demás y sube los suyos cada pocos segundos.',
            do: async (h) => {
                await h.click('button:has-text("Activar equipo")', { settle: 2500 });
                await h.highlight('.collab-pill', 3500);
            },
        },
        {
            chapter: 'Estado y reservas',
            say: 'El indicador de la barra muestra el estado en todo momento: equipo al día, sincronizando, o sin conexión. Sin internet puede seguir traduciendo con normalidad; todo se sincroniza al volver la conexión.',
            do: async (h) => {
                await h.highlight('.collab-pill', 3000);
                await h.banner('Sin conexión también se puede trabajar: se sincroniza al volver');
                await h.sleep(3500);
                await h.banner('');
            },
        },
        {
            say: 'Pasemos al modo de revisión. Cuando un compañero está traduciendo un chunk, usted lo ve reservado con su nombre en la tarjeta. Aquí, Ana está trabajando en los versículos cinco al ocho.',
            do: async (h) => {
                await h.click('button[title="Modo revisión"]', { settle: 1500 });
                await h.scrollTo(card('1:5-8'));
                await h.highlight(`${card('1:5-8')} .claim-badge`, 3500);
            },
        },
        {
            say: 'Si intenta editar ese mismo chunk, la aplicación le avisa para evitar que dos personas trabajen en lo mismo. Puede entrar de todos modos, pero lo normal es dejárselo a quien ya lo tiene y seguir con el suyo.',
            do: async (h) => {
                await h.click(`${card('1:5-8')} .pane.target .pane-head button[title="Modo chunk"]`, { settle: 1200 });
                await h.highlight('.dialog', 3500);
                await h.click('.dialog button:has-text("Cancelar")', { settle: 800 });
            },
        },
        {
            chapter: 'Traducir en paralelo',
            say: 'Cada chunk es independiente. Usted traduce sus versículos con tranquilidad mientras los demás trabajan en los suyos; como son archivos distintos, nunca se estorban. Al terminar, pulse Listo y su trabajo se sube en la siguiente sincronización.',
            do: async (h) => {
                await h.scrollTo(card('1:13-14'));
                await h.click(`${card('1:13-14')} .pane.target .pane-head button[title="Modo chunk"]`, { settle: 1200 });
                await h.click(`${card('1:13-14')} .verse-editor`, { settle: 300 });
                await h.page.keyboard.press('Control+End');
                // Los marcadores escritos a mano se vuelven fichas al instante.
                await h.page.keyboard.type('\\v 13 Tenía muchas cosas que escribirte. \\v 14 Espero verte pronto y hablar cara a cara.', { delay: 45 });
                await h.sleep(900);
                await h.click(`${card('1:13-14')} button:has-text("Listo")`, { settle: 1500 });
            },
        },
        {
            chapter: 'Cambios del equipo',
            say: 'Los cambios de los demás llegan solos. Un aviso le indica qué chunks cambiaron y esas tarjetas se actualizan sin interrumpir lo que usted está escribiendo. Aquí acaba de llegar la traducción de Ana de los versículos once y doce.',
            do: async (h) => {
                // Hasta que el backend simulado entregue el cambio remoto de Ana.
                for (let i = 0; i < 6; i++) {
                    const txt = await h.page.locator(`${card('1:11-12')} .pane.target`).innerText().catch(() => '');
                    if (txt.includes('cambio de Ana')) break;
                    await h.click('.collab-pill', { settle: 900 });
                    await h.click('.dialog button:has-text("Sincronizar ahora")', { settle: 1300 });
                    await h.click('.dialog button:has-text("Cerrar")', { settle: 500 });
                }
                await h.scrollTo(card('1:11-12'));
                await h.highlight(`${card('1:11-12')} .pane.target`, 4000);
            },
        },
        {
            say: 'Y si dos personas editaran el mismo chunk, la aplicación mostraría las dos versiones para que el equipo elija cuál conservar, como ya hace con los conflictos. Ese dispositivo no sube nada hasta resolverlo, y siempre queda la copia de respaldo anterior.',
            do: async (h) => {
                await h.scrollTo('.chunk-card:has(.pane.target.conflict)');
                await h.highlight('.chunk-card:has(.pane.target.conflict) .pane.target', 5000);
            },
        },
        {
            chapter: 'Unirse al proyecto',
            say: 'Sus compañeros se unen desde Inicio: en Más opciones, Unirse a un proyecto compartido. Verán los proyectos que usted compartió con ellos y, con un clic, tendrán su propia copia lista para traducir y sincronizada con el equipo.',
            do: async (h) => {
                await h.click('button[title="Inicio"]', { settle: 1500 });
                await h.click('button[title="Más opciones"]', { settle: 800 });
                await h.highlight('.menu button:has-text("Unirse")', 1500);
                await h.click('.menu button:has-text("Unirse")', { settle: 1500 });
                await h.highlight('.dialog .list-item', 3500);
            },
        },
        {
            say: 'Trabajo en equipo en BTT Writer: más rápido, sin perder nada y con todo el equipo viendo el mismo libro. Disponible en la próxima versión.',
            do: async (h) => {
                await h.click('.dialog button:has-text("Cerrar")', { settle: 400 });
                await h.card('Trabajo en equipo', 'Más rápido · Sin perder nada · Todo el equipo en el mismo libro');
            },
            pause: 1.5,
        },
    ],
};
