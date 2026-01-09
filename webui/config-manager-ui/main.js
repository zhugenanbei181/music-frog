const profilesContainer = document.getElementById('profiles');
const statusEl = document.getElementById('status');
const profilesStatusEl = document.getElementById('profiles-status');
const refreshBtn = document.getElementById('refresh-btn');
const importForm = document.getElementById('import-form');
const importSubmitBtn = importForm.querySelector('button[type="submit"]');
const localImportForm = document.getElementById('local-import-form');
const localFileInput = document.getElementById('local-file');
const localNameInput = document.getElementById('local-name');
const localActivateInput = document.getElementById('local-activate');
const localImportSubmitBtn = localImportForm.querySelector('button[type="submit"]');

const editorForm = document.getElementById('editor-form');
const editorNameInput = document.getElementById('editor-name');
const editorContentInput = document.getElementById('editor-content');
const editorActivateInput = document.getElementById('editor-activate');
const editorStatusEl = document.getElementById('editor-status');
const editorSaveBtn = document.getElementById('editor-save-btn');
const resetEditorBtn = document.getElementById('reset-editor-btn');
const newProfileBtn = document.getElementById('new-profile-btn');
const openExternalBtn = document.getElementById('open-external-btn');

const editorConfigForm = document.getElementById('editor-config-form');
const editorPathInput = document.getElementById('editor-path');
const editorConfigStatusEl = document.getElementById('editor-config-status');
const editorConfigSaveBtn = document.getElementById('editor-config-save');
const editorConfigResetBtn = document.getElementById('editor-config-reset');

const coreRefreshBtn = document.getElementById('core-refresh-btn');
const coreCurrentEl = document.getElementById('core-current');
const coreVersionsEl = document.getElementById('core-versions');
const coreStatusEl = document.getElementById('core-status');

const basePath = window.location.pathname.endsWith('/')
  ? window.location.pathname.slice(0, -1)
  : window.location.pathname;
const API_BASE = `${basePath}/api`;

function setStatus(message, type = '') {
  statusEl.textContent = message;
  statusEl.className = `status ${type}`;
}

function setProfilesStatus(message, type = '') {
  profilesStatusEl.textContent = message;
  profilesStatusEl.className = `status ${type}`;
}

function setBusy(isBusy) {
  refreshBtn.disabled = isBusy;
  importSubmitBtn.disabled = isBusy;
  localImportSubmitBtn.disabled = isBusy;
  localFileInput.disabled = isBusy;
  localNameInput.disabled = isBusy;
  localActivateInput.disabled = isBusy;
}

function setEditorBusy(isBusy) {
  editorSaveBtn.disabled = isBusy;
  resetEditorBtn.disabled = isBusy;
  newProfileBtn.disabled = isBusy;
  editorNameInput.disabled = isBusy;
  editorContentInput.disabled = isBusy;
  editorActivateInput.disabled = isBusy;
  openExternalBtn.disabled = isBusy;
}

function setEditorStatus(message, type = '') {
  editorStatusEl.textContent = message;
  editorStatusEl.className = `status ${type}`;
}

function setEditorConfigStatus(message, type = '') {
  editorConfigStatusEl.textContent = message;
  editorConfigStatusEl.className = `status ${type}`;
}

function setCoreStatus(message, type = '') {
  coreStatusEl.textContent = message;
  coreStatusEl.className = `status ${type}`;
}

function setCoreBusy(isBusy) {
  coreRefreshBtn.disabled = isBusy;
}

async function request(path, { method = 'GET', body } = {}) {
  const options = { method, headers: {} };
  if (body !== undefined) {
    options.body = JSON.stringify(body);
    options.headers['Content-Type'] = 'application/json';
  }
  const response = await fetch(`${API_BASE}/${path}`, options);
  let payload = null;
  const contentType = response.headers.get('content-type') || '';
  if (contentType.includes('application/json')) {
    payload = await response.json();
  } else if (!response.ok) {
    payload = await response.text();
  }
  if (!response.ok) {
    const message = payload?.error || payload || response.statusText;
    throw new Error(message);
  }
  return payload;
}

function createButton(label, onClick, { disabled = false, classes = '' } = {}) {
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.textContent = label;
  if (classes) btn.className = classes;
  btn.disabled = disabled;
  btn.addEventListener('click', onClick);
  return btn;
}

function createProfileCard(profile) {
  const card = document.createElement('div');
  card.className = 'profile-card';

  const header = document.createElement('div');
  header.className = 'profile-header';

  const name = document.createElement('span');
  name.className = 'profile-name';
  name.textContent = profile.name;
  header.append(name);

  const badge = document.createElement('span');
  badge.className = `badge ${profile.active ? 'active' : ''}`;
  badge.textContent = profile.active ? '当前' : '可用';
  header.append(badge);

  const body = document.createElement('div');
  body.className = 'path';
  body.textContent = profile.path;

  const actions = document.createElement('div');
  actions.className = 'profile-actions';

  const editBtn = createButton('编辑', () => loadProfileIntoEditor(profile.name));
  const externalBtn = createButton('外部编辑', () => openProfileInExternalEditor(profile.name), {
    classes: 'ghost',
  });
  const activateBtn = createButton(
    profile.active ? '已启用' : '设为当前',
    () => switchProfile(profile.name),
    { disabled: profile.active },
  );
  const deleteBtn = createButton(
    '删除',
    () => deleteProfile(profile.name),
    { disabled: profile.active, classes: 'danger' },
  );

  actions.append(editBtn, externalBtn, activateBtn, deleteBtn);

  card.append(header, body, actions);
  return card;
}

function createCoreVersionCard(version, current) {
  const card = document.createElement('div');
  card.className = 'profile-card';

  const header = document.createElement('div');
  header.className = 'profile-header';

  const name = document.createElement('span');
  name.className = 'profile-name';
  name.textContent = version;
  header.append(name);

  const badge = document.createElement('span');
  badge.className = `badge ${current ? 'active' : ''}`;
  badge.textContent = current ? '当前' : '可用';
  header.append(badge);

  const actions = document.createElement('div');
  actions.className = 'profile-actions';

  const activateBtn = createButton(
    current ? '已启用' : '设为当前',
    () => activateCoreVersion(version),
    { disabled: current },
  );
  actions.append(activateBtn);

  card.append(header, actions);
  return card;
}

async function refreshProfiles() {
  setBusy(true);
  setProfilesStatus('加载配置列表...', '');
  try {
    const profiles = await request('profiles');
    profilesContainer.innerHTML = '';
    if (!profiles || profiles.length === 0) {
      const empty = document.createElement('p');
      empty.textContent = '暂无配置，请先导入或新建。';
      profilesContainer.append(empty);
    } else {
      profiles
        .sort((a, b) => a.name.localeCompare(b.name))
        .forEach((profile) => profilesContainer.append(createProfileCard(profile)));
    }
    setProfilesStatus(`共 ${profiles.length || 0} 个配置`, 'success');
  } catch (err) {
    console.error(err);
    setProfilesStatus(err.message || String(err), 'error');
  } finally {
    setBusy(false);
  }
}

async function refreshCoreVersions() {
  setCoreBusy(true);
  setCoreStatus('加载内核版本...', '');
  try {
    const data = await request('core/versions');
    coreCurrentEl.textContent = data.current || '未设置';
    coreVersionsEl.innerHTML = '';
    if (!data.versions || data.versions.length === 0) {
      const empty = document.createElement('p');
      empty.textContent = '尚未安装版本，将使用内置内核。';
      coreVersionsEl.append(empty);
    } else {
      data.versions.forEach((version) => {
        const isCurrent = data.current === version;
        coreVersionsEl.append(createCoreVersionCard(version, isCurrent));
      });
    }
    setCoreStatus('内核版本已更新', 'success');
  } catch (err) {
    console.error(err);
    setCoreStatus(err.message || String(err), 'error');
  } finally {
    setCoreBusy(false);
  }
}

async function activateCoreVersion(version) {
  setCoreBusy(true);
  setCoreStatus(`正在切换到 ${version}...`, '');
  try {
    await request('core/activate', { method: 'POST', body: { version } });
    setCoreStatus(`已切换到 ${version}`, 'success');
    await refreshCoreVersions();
  } catch (err) {
    console.error(err);
    setCoreStatus(err.message || String(err), 'error');
    setCoreBusy(false);
  }
}

async function switchProfile(name) {
  setBusy(true);
  setStatus(`正在切换到 ${name}...`, '');
  try {
    await request('profiles/switch', { method: 'POST', body: { name } });
    setStatus(`已切换到 ${name}`, 'success');
  } catch (err) {
    console.error(err);
    setStatus(err.message || String(err), 'error');
  } finally {
    await refreshProfiles();
  }
}

async function openProfileInExternalEditor(name) {
  if (!name) {
    setEditorStatus('请先选择一个配置', 'error');
    return;
  }
  setEditorStatus(`正在用外部编辑器打开 ${name}...`, '');
  try {
    await request('profiles/open', { method: 'POST', body: { name } });
    setEditorStatus(`已在外部编辑器打开 ${name}`, 'success');
  } catch (err) {
    console.error(err);
    setEditorStatus(err.message || String(err), 'error');
  }
}

async function loadProfileIntoEditor(name) {
  setEditorBusy(true);
  setEditorStatus(`正在加载 ${name} ...`, '');
  try {
    const detail = await request(`profiles/${encodeURIComponent(name)}`);
    editorNameInput.value = detail.name;
    editorContentInput.value = detail.content;
    editorActivateInput.checked = detail.active;
    setEditorStatus(`正在编辑 ${detail.name}`, '');
    const panel = document.getElementById('editor-panel');
    if (panel) {
      panel.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
    editorContentInput.focus();
  } catch (err) {
    console.error(err);
    setEditorStatus(err.message || String(err), 'error');
  } finally {
    setEditorBusy(false);
  }
}

async function deleteProfile(name) {
  const confirmation = window.prompt(
    `危险操作：删除配置 ${name}。\n请输入配置名以确认删除：`,
  );
  if (confirmation !== name) {
    return;
  }
  setBusy(true);
  setStatus(`正在删除 ${name}...`, '');
  try {
    await request(`profiles/${encodeURIComponent(name)}`, { method: 'DELETE' });
    setStatus(`配置 ${name} 已删除`, 'success');
    if (editorNameInput.value.trim() === name) {
      resetEditorForm();
    }
    await refreshProfiles();
  } catch (err) {
    console.error(err);
    setStatus(err.message || String(err), 'error');
  } finally {
    setBusy(false);
  }
}

function resetEditorForm() {
  editorForm.reset();
  editorContentInput.value = '';
  editorActivateInput.checked = false;
  setEditorStatus('已准备新配置，输入名称并粘贴 YAML 后保存。', '');
}

async function refreshEditorConfig() {
  setEditorConfigStatus('正在读取编辑器设置...', '');
  try {
    const data = await request('editor');
    editorPathInput.value = data.editor || '';
    setEditorConfigStatus('编辑器设置已读取', 'success');
  } catch (err) {
    console.error(err);
    setEditorConfigStatus(err.message || String(err), 'error');
  }
}

importForm.addEventListener('submit', async (event) => {
  event.preventDefault();
  const formData = new FormData(importForm);
  const name = formData.get('name')?.toString() ?? '';
  const url = formData.get('url')?.toString() ?? '';
  const activate = formData.get('activate') === 'on';

  setBusy(true);
  setStatus(`正在从 ${url} 导入...`, '');
  try {
    const profile = await request('profiles/import', {
      method: 'POST',
      body: { name, url, activate },
    });
    setStatus(
      activate
        ? `配置 ${profile.name} 已导入并激活`
        : `配置 ${profile.name} 已导入，可在列表中启用`,
      'success',
    );
    importForm.reset();
  } catch (err) {
    console.error(err);
    setStatus(err.message || String(err), 'error');
  } finally {
    await refreshProfiles();
  }
});

localFileInput.addEventListener('change', () => {
  const file = localFileInput.files?.[0];
  if (!file) {
    return;
  }
  if (!localNameInput.value.trim()) {
    localNameInput.value = file.name.replace(/\.[^/.]+$/, '');
  }
});

localImportForm.addEventListener('submit', async (event) => {
  event.preventDefault();
  const file = localFileInput.files?.[0];
  if (!file) {
    setStatus('请选择本地文件', 'error');
    return;
  }
  const name = localNameInput.value.trim() || file.name.replace(/\.[^/.]+$/, '');
  const activate = localActivateInput.checked;

  setBusy(true);
  setStatus(`正在从本地文件导入 ${file.name}...`, '');
  try {
    const content = await file.text();
    if (!content.trim()) {
      throw new Error('文件内容为空');
    }
    const profile = await request('profiles/save', {
      method: 'POST',
      body: { name, content, activate },
    });
    setStatus(
      activate
        ? `配置 ${profile.name} 已导入并激活`
        : `配置 ${profile.name} 已导入，可在列表中启用`,
      'success',
    );
    localImportForm.reset();
  } catch (err) {
    console.error(err);
    setStatus(err.message || String(err), 'error');
  } finally {
    await refreshProfiles();
  }
});

editorForm.addEventListener('submit', async (event) => {
  event.preventDefault();
  const name = editorNameInput.value.trim();
  const content = editorContentInput.value;
  const activate = editorActivateInput.checked;

  if (!name) {
    setEditorStatus('配置名称不能为空', 'error');
    return;
  }
  if (!content.trim()) {
    setEditorStatus('配置内容不能为空', 'error');
    return;
  }

  setEditorBusy(true);
  setEditorStatus(`正在保存 ${name}...`, '');
  try {
    const response = await request('profiles/save', {
      method: 'POST',
      body: { name, content, activate },
    });
    let message = activate ? `配置 ${name} 已保存并设为当前` : `配置 ${name} 已保存`;
    if (response?.controller_url) {
      const suffix = response.controller_changed ? '（已更新）' : '';
      message += `，控制接口：${response.controller_url}${suffix}`;
    }
    setEditorStatus(message, 'success');
    await refreshProfiles();
  } catch (err) {
    console.error(err);
    setEditorStatus(err.message || String(err), 'error');
  } finally {
    setEditorBusy(false);
  }
});

refreshBtn.addEventListener('click', refreshProfiles);
resetEditorBtn.addEventListener('click', resetEditorForm);
newProfileBtn.addEventListener('click', () => {
  resetEditorForm();
  editorNameInput.focus();
});
openExternalBtn.addEventListener('click', () => {
  const name = editorNameInput.value.trim();
  openProfileInExternalEditor(name);
});

editorConfigForm.addEventListener('submit', async (event) => {
  event.preventDefault();
  const editor = editorPathInput.value.trim();
  setEditorConfigStatus('正在保存编辑器设置...', '');
  try {
    await request('editor', { method: 'POST', body: { editor } });
    setEditorConfigStatus('编辑器设置已保存', 'success');
  } catch (err) {
    console.error(err);
    setEditorConfigStatus(err.message || String(err), 'error');
  }
});

editorConfigResetBtn.addEventListener('click', async () => {
  editorPathInput.value = '';
  setEditorConfigStatus('正在恢复默认...', '');
  try {
    await request('editor', { method: 'POST', body: { editor: '' } });
    setEditorConfigStatus('已恢复默认编辑器', 'success');
  } catch (err) {
    console.error(err);
    setEditorConfigStatus(err.message || String(err), 'error');
  }
});

coreRefreshBtn.addEventListener('click', refreshCoreVersions);

window.addEventListener('DOMContentLoaded', () => {
  refreshProfiles();
  resetEditorForm();
  refreshEditorConfig();
  refreshCoreVersions();
});
