(() => {
  const invoke = window.__TAURI__?.core?.invoke;
  const $ = (id) => document.getElementById(id);
  const buttons = [...document.querySelectorAll('button')];

  function setBusy(value) {
    buttons.forEach((button) => { button.disabled = value; });
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
    $('healthPill').className = `pill ${healthy ? 'good' : 'muted'}`;
    renderValidation(status.last_validation);
  }

  async function run(command) {
    if (!invoke) return;
    setBusy(true);
    try {
      const result = await invoke(command);
      renderAction(result);
    } catch (error) {
      renderAction({ title: 'Action failed', detail: String(error) });
    } finally {
      setBusy(false);
      await refresh();
    }
  }

  $('createPack').addEventListener('click', () => run('create_chat_pack'));
  $('applyUpdate').addEventListener('click', () => run('apply_latest_update_and_play'));
  $('playCurrent').addEventListener('click', () => run('play_current'));
  $('scanUpdate').addEventListener('click', () => run('scan_latest_update'));
  $('rollback').addEventListener('click', () => run('rollback'));
  $('bootstrap').addEventListener('click', () => run('initialize_pipeline'));
  $('refreshStatus').addEventListener('click', refresh);
  refresh().catch((e) => renderAction({ title: 'Status error', detail: String(e) }));
})();
