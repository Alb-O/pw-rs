let domains = new Set();
let connected = false;
let authenticated = false;
let currentTabDomain = null;

const statusDot = document.getElementById('statusDot');
const statusText = document.getElementById('statusText');
const serverInput = document.getElementById('serverInput');
const tokenInput = document.getElementById('tokenInput');
const connectBtn = document.getElementById('connectBtn');
const domainList = document.getElementById('domainList');
const newDomainInput = document.getElementById('newDomain');
const addDomainBtn = document.getElementById('addDomainBtn');
const exportBtn = document.getElementById('exportBtn');
const messageArea = document.getElementById('messageArea');

async function init() {
  const stored = await chrome.storage.local.get(['pw_export_domains', 'pw_export_server']);
  if (stored.pw_export_domains) {
    domains = new Set(stored.pw_export_domains);
  }
  if (stored.pw_export_server) {
    serverInput.value = stored.pw_export_server;
  }

  // Query from popup context to get the actual active tab, not the background's view
  try {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (tab?.url) {
      currentTabDomain = extractDomain(tab.url);
      if (currentTabDomain) {
        newDomainInput.placeholder = currentTabDomain;
      }
    }
  } catch (e) {
    console.error('Failed to get current tab:', e);
  }

  chrome.runtime.sendMessage({ type: 'get_status' }, (response) => {
    if (response?.type === 'status') {
      updateStatus(response.connected, response.authenticated, response.server);
    }
  });

  renderDomains();
}

function extractDomain(url) {
  try {
    return new URL(url).hostname;
  } catch {
    return null;
  }
}

chrome.runtime.onMessage.addListener((message) => {
  if (message.type === 'status') {
    updateStatus(message.connected, message.authenticated, message.server);
  } else if (message.type === 'export_result') {
    if (message.success) {
      showMessage('success', `Saved cookies for ${message.domains_saved} domain(s)`, message.paths);
    } else {
      showMessage('error', message.error || 'Export failed');
    }
  } else if (message.type === 'error') {
    showMessage('error', message.message);
  }
});

function updateStatus(conn, auth, server) {
  connected = conn;
  authenticated = auth;

  statusDot.className = 'dot';
  if (authenticated) {
    statusDot.classList.add('authenticated');
    statusText.textContent = 'Connected & authenticated';
    connectBtn.textContent = 'Disconnect';
  } else if (connected) {
    statusDot.classList.add('connected');
    statusText.textContent = 'Connected (awaiting auth)';
    connectBtn.textContent = 'Disconnect';
  } else {
    statusText.textContent = 'Not connected';
    connectBtn.textContent = 'Connect';
  }

  exportBtn.disabled = !authenticated || domains.size === 0;
}

function renderDomains() {
  domainList.innerHTML = '';

  if (domains.size === 0) {
    domainList.innerHTML = '<div style="color: #666; font-size: 12px; padding: 8px 0;">No domains added</div>';
    exportBtn.disabled = true;
    return;
  }

  for (const domain of domains) {
    const item = document.createElement('div');
    item.className = 'domain-item';

    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.checked = true;
    checkbox.id = `domain-${domain}`;

    const label = document.createElement('label');
    label.htmlFor = checkbox.id;
    label.textContent = domain;

    const removeBtn = document.createElement('button');
    removeBtn.className = 'secondary';
    removeBtn.textContent = '×';
    removeBtn.style.padding = '2px 8px';
    removeBtn.style.marginLeft = 'auto';
    removeBtn.onclick = () => {
      domains.delete(domain);
      renderDomains();
      saveDomains();
    };

    item.appendChild(checkbox);
    item.appendChild(label);
    item.appendChild(removeBtn);
    domainList.appendChild(item);
  }

  exportBtn.disabled = !authenticated || domains.size === 0;
}

function saveDomains() {
  chrome.storage.local.set({ pw_export_domains: [...domains] });
}

function saveServer() {
  chrome.storage.local.set({ pw_export_server: serverInput.value });
}

function showMessage(type, text, paths) {
  messageArea.innerHTML = '';
  const msg = document.createElement('div');
  msg.className = `message ${type}`;
  msg.textContent = text;

  if (paths?.length > 0) {
    const pathsDiv = document.createElement('div');
    pathsDiv.className = 'paths';
    pathsDiv.textContent = paths.join('\n');
    msg.appendChild(pathsDiv);
  }

  messageArea.appendChild(msg);

  if (type === 'success') {
    setTimeout(() => msg.remove(), 5000);
  }
}

connectBtn.onclick = () => {
  if (connected) {
    // Empty credentials trigger disconnect on the server side
    chrome.runtime.sendMessage({ type: 'connect', server: '', token: '' });
    updateStatus(false, false, null);
    return;
  }

  const server = serverInput.value.trim();
  const token = tokenInput.value.trim();

  if (!server) {
    showMessage('error', 'Please enter server URL');
    return;
  }
  if (!token) {
    showMessage('error', 'Please enter authentication token');
    return;
  }

  saveServer();
  showMessage('info', 'Connecting...');
  chrome.runtime.sendMessage({ type: 'connect', server, token }, (response) => {
    if (response?.type === 'error') {
      showMessage('error', response.message);
    }
  });
};

addDomainBtn.onclick = () => {
  // Falls back to current tab domain if input is empty
  let domain = newDomainInput.value.trim().toLowerCase() || currentTabDomain;
  if (!domain) return;

  if (!domain.includes('.') || domain.includes(' ')) {
    showMessage('error', 'Invalid domain format');
    return;
  }

  domains.add(domain);
  newDomainInput.value = '';
  renderDomains();
  saveDomains();
};

newDomainInput.onkeydown = (e) => {
  if (e.key === 'Enter') addDomainBtn.click();
};

exportBtn.onclick = () => {
  const selectedDomains = [...domains].filter((domain) => {
    const checkbox = document.getElementById(`domain-${domain}`);
    return checkbox?.checked;
  });

  if (selectedDomains.length === 0) {
    showMessage('error', 'No domains selected');
    return;
  }

  showMessage('info', `Exporting cookies for ${selectedDomains.length} domain(s)...`);
  chrome.runtime.sendMessage({ type: 'export', domains: selectedDomains });
};

init();
