// The few W3C WebDriver calls the end-to-end check makes, over fetch
// (https://www.w3.org/TR/webdriver2/).

export class WebDriver {
  constructor(base) {
    this.base = base;
    this.session = null;
  }

  async call(method, path, body) {
    const res = await fetch(this.base + path, {
      method,
      headers: { 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const json = await res.json();
    if (json.value && json.value.error) throw new Error(`${json.value.error}: ${json.value.message}`);
    return json.value;
  }

  async status() {
    return (await this.call('GET', '/status')).ready !== false;
  }

  async newSession(alwaysMatch) {
    const value = await this.call('POST', '/session', { capabilities: { alwaysMatch } });
    this.session = `/session/${value.sessionId}`;
    return value;
  }

  deleteSession() {
    return this.call('DELETE', this.session);
  }

  run(script, args = []) {
    return this.call('POST', `${this.session}/execute/sync`, { script, args });
  }

  runAsync(script, args = []) {
    return this.call('POST', `${this.session}/execute/async`, { script, args });
  }

  rect() {
    return this.call('GET', `${this.session}/window/rect`);
  }

  setRect(rect) {
    return this.call('POST', `${this.session}/window/rect`, rect);
  }

  // Presses the keys together, then lets them go in reverse order
  // ('' is Control).
  keys(keys) {
    const down = keys.map((value) => ({ type: 'keyDown', value }));
    const up = [...keys].reverse().map((value) => ({ type: 'keyUp', value }));
    return this.call('POST', `${this.session}/actions`, {
      actions: [{ type: 'key', id: 'keyboard', actions: [...down, ...up] }],
    });
  }
}
