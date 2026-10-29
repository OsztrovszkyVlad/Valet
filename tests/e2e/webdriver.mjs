// Minimal W3C WebDriver client over Node's built-in fetch (Node 22+).
// Covers what the smoke test needs: session, find (CSS or XPath), click,
// send keys, text, attributes, window size, screenshot.

const ELEMENT_KEY = 'element-6066-11e4-a52e-4f735466cecf';

export class WebDriverError extends Error {
    constructor(method, path, status, value) {
        const detail = value && typeof value === 'object' ? `${value.error}: ${value.message}` : String(value);
        super(`${method} ${path} -> HTTP ${status}: ${detail}`);
        this.status = status;
        this.code = value && typeof value === 'object' ? value.error : undefined;
    }
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export class Session {
    constructor(base, id) {
        this.base = base;
        this.id = id;
    }

    /** POST /session with `tauri:options.application` (tauri-driver). */
    static async create(base, application, args = []) {
        const body = {
            capabilities: {
                alwaysMatch: {
                    'tauri:options': { application, args },
                },
            },
        };
        const value = await request(base, 'POST', '/session', body);
        return new Session(base, value.sessionId);
    }

    cmd(method, path, body) {
        return request(this.base, method, `/session/${this.id}${path}`, body);
    }

    async delete() {
        await request(this.base, 'DELETE', `/session/${this.id}`);
    }

    async setWindowSize(width, height) {
        return this.cmd('POST', '/window/rect', { width, height });
    }

    async getWindowRect() {
        return this.cmd('GET', '/window/rect');
    }

    /** POST /execute/sync: runs `script` (a function body) in the page. */
    async execute(script, args = []) {
        return this.cmd('POST', '/execute/sync', { script, args });
    }

    /** Base64 PNG of the viewport. */
    async screenshot() {
        return this.cmd('GET', '/screenshot');
    }

    async findAll(using, value) {
        const list = await this.cmd('POST', '/elements', { using, value });
        return list.map((e) => new Element(this, e[ELEMENT_KEY]));
    }

    async find(using, value) {
        const e = await this.cmd('POST', '/element', { using, value });
        return new Element(this, e[ELEMENT_KEY]);
    }

    css(selector) {
        return this.find('css selector', selector);
    }

    xpath(expr) {
        return this.find('xpath', expr);
    }

    xpathAll(expr) {
        return this.findAll('xpath', expr);
    }

    /**
     * Polls until `xpath` matches a displayed element (or any element when
     * `displayed` is false). Throws with `what` in the message on timeout.
     */
    async waitFor(expr, { timeout = 15000, displayed = true, what = expr } = {}) {
        const end = Date.now() + timeout;
        let last;
        while (Date.now() < end) {
            try {
                const els = await this.xpathAll(expr);
                for (const el of els) {
                    if (!displayed || (await el.displayed())) return el;
                }
            } catch (err) {
                last = err;
            }
            await sleep(200);
        }
        throw new Error(`timeout after ${timeout} ms waiting for ${what}${last ? ` (${last.message})` : ''}`);
    }

    /** Polls until no displayed element matches `expr`. */
    async waitGone(expr, { timeout = 15000, what = expr } = {}) {
        const end = Date.now() + timeout;
        while (Date.now() < end) {
            const els = await this.xpathAll(expr);
            let visible = false;
            for (const el of els) {
                try {
                    if (await el.displayed()) visible = true;
                } catch {
                    // stale: gone
                }
            }
            if (!visible) return;
            await sleep(200);
        }
        throw new Error(`timeout after ${timeout} ms waiting for ${what} to disappear`);
    }

    /** Visible text of the page body, for failure messages. */
    async bodyText() {
        return (await this.find('css selector', 'body')).text();
    }
}

export class Element {
    constructor(session, id) {
        this.session = session;
        this.id = id;
    }

    cmd(method, path, body) {
        return this.session.cmd(method, `/element/${this.id}${path}`, body);
    }

    click() {
        return this.cmd('POST', '/click', {});
    }

    sendKeys(text) {
        return this.cmd('POST', '/value', { text });
    }

    text() {
        return this.cmd('GET', '/text');
    }

    attribute(name) {
        return this.cmd('GET', `/attribute/${encodeURIComponent(name)}`);
    }

    property(name) {
        return this.cmd('GET', `/property/${encodeURIComponent(name)}`);
    }

    selected() {
        return this.cmd('GET', '/selected');
    }

    enabled() {
        return this.cmd('GET', '/enabled');
    }

    displayed() {
        return this.cmd('GET', '/displayed');
    }

    async findAll(using, value) {
        const list = await this.cmd('POST', '/elements', { using, value });
        return list.map((e) => new Element(this.session, e[ELEMENT_KEY]));
    }

    xpathAll(expr) {
        return this.findAll('xpath', expr);
    }
}

async function request(base, method, path, body) {
    const res = await fetch(base + path, {
        method,
        headers: body === undefined ? {} : { 'content-type': 'application/json; charset=utf-8' },
        body: body === undefined ? undefined : JSON.stringify(body),
    });
    const raw = await res.text();
    let json;
    try {
        json = raw === '' ? {} : JSON.parse(raw);
    } catch {
        throw new WebDriverError(method, path, res.status, raw.slice(0, 500));
    }
    if (!res.ok) throw new WebDriverError(method, path, res.status, json.value ?? json);
    return json.value ?? null;
}

/** Polls GET /status until the server answers. */
export async function waitForServer(base, timeout = 20000) {
    const end = Date.now() + timeout;
    let last;
    while (Date.now() < end) {
        try {
            const res = await fetch(`${base}/status`);
            if (res.ok || res.status === 404) return;
        } catch (err) {
            last = err;
        }
        await sleep(200);
    }
    throw new Error(`WebDriver server at ${base} did not answer within ${timeout} ms (${last?.message ?? 'no response'})`);
}
