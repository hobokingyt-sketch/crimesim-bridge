(() => {
  const invoke = window.__TAURI__?.core?.invoke;
  const $ = (id) => document.getElementById(id);
  const buttons = [...document.querySelectorAll('button, select, textarea')];
  let contextReady = false;
  let busy = false;

  function setBusy(value) {
    busy = value;
    buttons.forEach((button) => { button.disabled = value; });
    $('createPack').disabled = value || !contextReady;
  }

  function basename(path) {
    if (!path) return 'None';
    return path.replace(/\\/g, '/').split('/').pop();
  }

  function renderAction(result) {
    $('activityTitle').textContent = result?.title || 'Action complete';
    $('activityDetail').textContent = result?.detail || '';
    $('activityPath').textContent = result?.path || '';
  }

  function renderValidation(report) {
    const target = $('validationList');
    target.innerHTML = '';
    if (!report?.steps?.length) {
      target.innerHTML = '<p class="empty">No validation has run yet.</p>';
      return;
    }
    for (const step of report.steps) {
      const row = document.createElement('div');
      row.className = `validation-row ${step.status}`;
      const mark = step.status === 'passed' ? '✓' : step.status === 'failed' ? '×' : '–';
      row.innerHTML = `<span class="mark">${mark}</span><span class="name"></span><span class="detail"></span>`;
      row.querySelector('.name').textContent = step.name;
      row.querySelector('.detail').textContent = step.detail;
      target.appendChild(row);
    }
    const errors = $('validationErrors');
    const summaries = report.errors || [];
    errors.hidden = summaries.length === 0;
    errors.textContent = summaries.length ? `Failure summary: ${summaries.join(' · ')}` : '';
  }

  async function refresh() {
    if (!invoke) {
      renderAction({ title: 'Tauri runtime unavailable', detail: 'Open this interface through the compiled CrimeSim Bridge application.' });
      return;
    }
    const status = await invoke('get_status');
    $('sourceRevision').textContent = status.source_revision ?? 'Not initialized';
    $('playableRevision').textContent = status.playable_revision ?? 'None';
    $('godotStatus').textContent = status.godot_runtime_present ? `Ready · ${status.engine_version || ''}` : 'Runtime missing';
    $('runtimeIntegrity').textContent = status.godot_runtime_integrity || 'Unknown';
    $('latestUpdate').textContent = status.latest_update ? basename(status.latest_update) : 'None found';
    $('recoveryStatus').textContent = status.recovery_error || (status.recovery_required ? 'Recovery pending; project controls blocked' : status.last_recovery || 'None');
    const healthy = Boolean(status.pipeline_ready);
    $('healthPill').textContent = status.recovery_required || status.recovery_error ? 'RECOVERY REQUIRED' : healthy ? 'READY' : status.project_present ? 'SETUP' : 'EMPTY';
    const needsRecovery = Boolean(status.recovery_required || status.recovery_error);
    $('healthPill').className = `pill ${needsRecovery ? 'bad' : healthy ? 'good' : 'muted'}`;
    $('recoveryStatus').title = $('recoveryStatus').textContent;
    if (needsRecovery) {
      renderAction({
        title: 'Recovery required',
        detail: status.recovery_error || 'An interrupted operation needs recovery. Project changes and launch requests remain blocked until recovery succeeds.'
      });
    }
    renderValidation(status.last_validation);
    contextReady = false;
    if (status.project_present && !needsRecovery) {
      try {
        const options = await invoke('chat_context_options');
        const previous = $('contextArea').value;
        $('contextArea').replaceChildren();
        for (const area of options.modules) {
          const option = document.createElement('option');
          option.value = area.id;
          option.textContent = area.title;
          $('contextArea').appendChild(option);
        }
        if (options.modules.some((m) => m.id === previous)) $('contextArea').value = previous;
        contextReady = options.modules.length > 0;
        $('contextHint').textContent = options.memory_origin === 'read_only_legacy_routes'
          ? 'Older project: temporary reading routes. Export will not rewrite its source.'
          : 'Game memory and exact source hashes travel with the selected area.';
      } catch (error) {
        $('contextHint').textContent = 'Context unavailable: ' + String(error);
      }
    } else {
      $('contextHint').textContent = needsRecovery ? 'Finish recovery before exporting.' : 'Initialize the project to prepare a handoff.';
    }
    setBusy(busy);
  }

  async function run(command, args) {
    if (!invoke) return;
    setBusy(true);
    renderAction({ title: 'Working', detail: 'The Bridge is checking the project. Godot workers have enforced time limits.' });
    try {
      const result = await invoke(command, args);
      renderAction(result);
    } catch (error) {
      renderAction({ title: 'Action failed', detail: String(error) });
    } finally {
      try { await refresh(); }
      catch (error) { renderAction({ title: 'Status check failed', detail: String(error) }); }
      setBusy(false);
    }
  }

  $('createPack').addEventListener('click', () => {
    if (!contextReady || busy) return;
    try {
      const raw = $('contextRequests').value.trim();
      const fileRequests = raw ? JSON.parse(raw).file_requests : [];
      if (!Array.isArray(fileRequests)) throw new Error('Paste the file_requests object provided by chat.');
      const request = { module_id: $('contextArea').value, task: $('contextTask').value.trim(), file_requests: fileRequests };
      run('create_chat_pack', { request });
    } catch (error) {
      renderAction({ title: 'File request needs attention', detail: String(error) });
    }
  });
  $('applyUpdate').addEventListener('click', () => run('apply_latest_update_and_play'));
  $('playCurrent').addEventListener('click', () => run('play_current'));
  $('scanUpdate').addEventListener('click', () => run('scan_latest_update'));
  $('rollback').addEventListener('click', () => run('rollback'));
  $('bootstrap').addEventListener('click', () => run('initialize_pipeline'));
  $('refreshStatus').addEventListener('click', () => refresh().catch((error) => renderAction({ title: 'Status check failed', detail: String(error) })));
  refresh().then(() => {
    if (!invoke) return;
    requestAnimationFrame(() => requestAnimationFrame(() => {
      const controlsReady = ['createPack', 'applyUpdate', 'playCurrent', 'bootstrap'].every((id) => {
        const element = $(id);
        return element && element.getBoundingClientRect().width > 0 && getComputedStyle(element).visibility !== 'hidden';
      });
      // A no-op during normal use; the packaged acceptance mode verifies real WebView IPC.
      invoke('frontend_ready', { controlsReady }).catch((error) => renderAction({ title: 'Startup check failed', detail: String(error) }));
    }));
  }).catch((e) => renderAction({ title: 'Status error', detail: String(e) }));
})();
