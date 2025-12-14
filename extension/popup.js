const RELAY = 'ws://127.0.0.1:19988/extension';
const dot = document.getElementById('dot');
const stateEl = document.getElementById('state');
const addrEl = document.getElementById('addr');
const logEl = document.getElementById('log');

addrEl.textContent = RELAY;

function setState(text, color) {
  stateEl.textContent = text;
  dot.style.background = color;
}

chrome.storage.local.get(['pw_bridge_log', 'pw_bridge_state'], (data) => {
  const state = data.pw_bridge_state || { status: 'unknown', message: 'No data yet' };
  setState(state.message, colorFor(state.status));
  logEl.textContent = (data.pw_bridge_log || []).join('\n');
});

chrome.runtime.onMessage.addListener((msg) => {
  if (msg.type === 'pw-bridge-state') {
    setState(msg.message, colorFor(msg.status));
  }
  if (msg.type === 'pw-bridge-log') {
    logEl.textContent = msg.lines.join('\n');
  }
});

function colorFor(status) {
  switch (status) {
    case 'connected': return '#1aaa50';
    case 'error': return '#d22';
    case 'disconnected': return '#888';
    default: return '#999';
  }
}
