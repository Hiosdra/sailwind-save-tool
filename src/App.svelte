<script lang="ts">
  import { native } from './lib/api/native';
  import type { AppError, DiagnosticReport, SlotSummary, SnapshotSummary } from './lib/api/contracts';
  import { formatKilobytes } from './lib/format';
  import { onMount } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';

  let saveDirectory = $state('');
  let slots = $state<SlotSummary[]>([]);
  let snapshots = $state<SnapshotSummary[]>([]);
  let selectedSlot = $state<number | null>(null);
  let busy = $state(false);
  let error = $state('');
  let diagnostics = $state<DiagnosticReport | null>(null);
  let diagnosticsOpen = $state(false);
  let diagnosticCopied = $state(false);

  function describeError(value: unknown): string {
    if (typeof value === 'object' && value !== null && 'message' in value) {
      return String((value as AppError).message);
    }
    return String(value);
  }

  async function detect(): Promise<void> {
    busy = true;
    error = '';
    try {
      const directories = await native.detectSaveDirectories();
      if (directories[0]) {
        saveDirectory = directories[0];
        await refresh();
      } else {
        error = 'No Sailwind save directory was detected. Enter one manually.';
      }
    } catch (cause) {
      error = describeError(cause);
    } finally {
      busy = false;
    }
  }

  async function chooseDirectory(): Promise<void> {
    const selected = await open({ directory: true, multiple: false, title: 'Choose Sailwind save directory' });
    if (selected) {
      saveDirectory = selected;
      await refresh();
    }
  }

  async function refresh(): Promise<void> {
    if (!saveDirectory.trim()) return;
    busy = true;
    error = '';
    try {
      slots = await native.discoverSlots(saveDirectory.trim());
      snapshots = await native.listSnapshots();
      await native.setActiveSaveDirectory(saveDirectory.trim());
      if (selectedSlot === null && slots[0]) selectedSlot = slots[0].slot;
    } catch (cause) {
      error = describeError(cause);
    } finally {
      busy = false;
    }
  }

  async function createSnapshot(): Promise<void> {
    if (selectedSlot === null) return;
    busy = true;
    error = '';
    try {
      await native.createSnapshot(saveDirectory.trim(), selectedSlot);
      snapshots = await native.listSnapshots();
    } catch (cause) {
      const appError = cause as Partial<AppError>;
      if (appError?.code === 'retention_confirmation_required') {
        const confirmed = window.confirm(
          'This slot has reached 50 snapshots. Create the snapshot and remove the oldest unprotected version?'
        );
        if (confirmed) {
          try {
            await native.createSnapshot(saveDirectory.trim(), selectedSlot, true);
            snapshots = await native.listSnapshots();
          } catch (retryCause) {
            error = describeError(retryCause);
          }
        }
      } else {
        error = describeError(cause);
      }
    } finally {
      busy = false;
    }
  }

  async function restore(snapshot: SnapshotSummary): Promise<void> {
    const confirmed = window.confirm(
      `Restore snapshot ${snapshot.id.slice(0, 10)}? A safety snapshot of the current slot will be created first.`
    );
    if (!confirmed) return;
    busy = true;
    error = '';
    try {
      await native.restoreSnapshot(saveDirectory.trim(), snapshot.id);
      await refresh();
    } catch (cause) {
      error = describeError(cause);
    } finally {
      busy = false;
    }
  }

  async function editLabel(snapshot: SnapshotSummary): Promise<void> {
    const label = window.prompt('Snapshot label (leave empty to remove)', snapshot.label ?? '');
    if (label === null) return;
    busy = true;
    error = '';
    try {
      await native.updateSnapshotAnnotation(snapshot.id, label, snapshot.note, snapshot.protected);
      snapshots = await native.listSnapshots();
    } catch (cause) {
      error = describeError(cause);
    } finally {
      busy = false;
    }
  }

  async function toggleProtection(snapshot: SnapshotSummary): Promise<void> {
    busy = true;
    error = '';
    try {
      await native.updateSnapshotAnnotation(snapshot.id, snapshot.label, snapshot.note, !snapshot.protected);
      snapshots = await native.listSnapshots();
    } catch (cause) {
      error = describeError(cause);
    } finally {
      busy = false;
    }
  }

  async function removeSnapshot(snapshot: SnapshotSummary): Promise<void> {
    if (!window.confirm(`Permanently delete snapshot ${snapshot.id.slice(0, 10)}?`)) return;
    busy = true;
    error = '';
    try {
      await native.deleteSnapshot(snapshot.id);
      snapshots = await native.listSnapshots();
    } catch (cause) {
      error = describeError(cause);
    } finally {
      busy = false;
    }
  }

  async function exportBackup(snapshot: SnapshotSummary): Promise<void> {
    const destination = await save({
      defaultPath: `${snapshot.id}.swbackup`,
      filters: [{ name: 'Sailwind backup', extensions: ['swbackup'] }]
    });
    if (!destination) return;
    busy = true;
    error = '';
    try {
      await native.exportSnapshot(snapshot.id, destination);
    } catch (cause) {
      error = describeError(cause);
    } finally {
      busy = false;
    }
  }

  async function importBackup(): Promise<void> {
    const source = await open({
      multiple: false,
      filters: [{ name: 'Sailwind backup', extensions: ['swbackup'] }]
    });
    if (!source) return;
    busy = true;
    error = '';
    try {
      const imported = await native.importSnapshot(source);
      snapshots = await native.listSnapshots();
      selectedSlot = imported.slot;
    } catch (cause) {
      error = describeError(cause);
    } finally {
      busy = false;
    }
  }

  async function refreshDiagnostics(): Promise<void> {
    diagnostics = await native.loadDiagnostics();
  }

  async function toggleDiagnostics(): Promise<void> {
    try {
      diagnosticsOpen = !diagnosticsOpen;
      if (diagnosticsOpen) await refreshDiagnostics();
    } catch (cause) {
      diagnosticsOpen = false;
      error = describeError(cause);
    }
  }

  async function dismissRecovery(): Promise<void> {
    try {
      await native.acknowledgeRecovery();
      await refreshDiagnostics();
    } catch (cause) {
      error = describeError(cause);
    }
  }

  async function copyDiagnosticReport(): Promise<void> {
    try {
      await refreshDiagnostics();
      await copyText(JSON.stringify(diagnostics, null, 2));
      diagnosticCopied = true;
      window.setTimeout(() => (diagnosticCopied = false), 2000);
    } catch (cause) {
      error = describeError(cause);
    }
  }

  async function copyText(value: string): Promise<void> {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(value);
      return;
    }
    const textArea = document.createElement('textarea');
    textArea.value = value;
    textArea.setAttribute('readonly', '');
    textArea.style.position = 'fixed';
    textArea.style.opacity = '0';
    document.body.appendChild(textArea);
    textArea.select();
    const copied = document.execCommand('copy');
    textArea.remove();
    if (!copied) throw new Error('The diagnostic report could not be copied to the clipboard.');
  }

  const selected = $derived(slots.find((slot) => slot.slot === selectedSlot));
  const selectedSnapshots = $derived(snapshots.filter((item) => item.slot === selectedSlot));

  onMount(async () => {
    try {
      await refreshDiagnostics();
    } catch (cause) {
      error = `Diagnostics unavailable: ${describeError(cause)}`;
    }
    try {
      const settings = await native.loadSettings();
      if (settings.activeSaveDirectory) {
        saveDirectory = settings.activeSaveDirectory;
        await refresh();
      }
    } catch (cause) {
      error = describeError(cause);
    }
  });
</script>

<svelte:head><title>Sailwind Save Manager</title></svelte:head>

<main>
  <header>
    <div>
      <p class="eyebrow">LOCAL SAVE ARCHIVE</p>
      <h1>Sailwind Save Manager</h1>
      <p class="subtitle">Immutable, verified snapshots for every voyage.</p>
    </div>
    <div class="header-actions"><button class="secondary" onclick={toggleDiagnostics}>Diagnostics</button><button class="secondary" onclick={importBackup} disabled={busy}>Import .swbackup</button><span class="status">0.1.0-alpha.1</span></div>
  </header>

  {#if diagnostics?.recoveryNotice}
    <section class:failed={diagnostics.recoveryNotice.outcome === 'failed'} class="recovery-banner" role="status">
      <div><strong>Restore recovery</strong><p>{diagnostics.recoveryNotice.message}</p></div>
      <button class="secondary compact" onclick={dismissRecovery}>Acknowledge</button>
    </section>
  {/if}

  {#if diagnosticsOpen && diagnostics}
    <section class="diagnostics card" aria-label="Diagnostics">
      <div class="section-title">
        <div><p class="eyebrow">PRIVACY-SAFE REPORT</p><h2>Diagnostics</h2></div>
        <button class="secondary" onclick={copyDiagnosticReport}>{diagnosticCopied ? 'Copied' : 'Copy report'}</button>
      </div>
      <p class="privacy-note">{diagnostics.privacyNote}</p>
      <dl><div><dt>Version</dt><dd>{diagnostics.appVersion}</dd></div><div><dt>Platform</dt><dd>{diagnostics.operatingSystem} · {diagnostics.architecture}</dd></div></dl>
      <h3>Recent operations</h3>
      {#if diagnostics.recentOperations.length === 0}
        <p class="empty">No operations have been recorded yet.</p>
      {:else}
        <ol class="operation-log">
          {#each diagnostics.recentOperations.slice(-8).reverse() as operation}
            <li><span class:failed={operation.outcome === 'failed'}>{operation.outcome}</span><code>{operation.operation}</code><small>{new Date(operation.occurredAtUtc).toLocaleString()}{operation.errorCode ? ` · ${operation.errorCode}` : ''}</small></li>
          {/each}
        </ol>
      {/if}
    </section>
  {/if}

  <section class="location card">
    <label for="save-directory">Active save directory</label>
    <div class="location-row">
      <input id="save-directory" bind:value={saveDirectory} placeholder="C:\\Users\\…\\Sailwind" />
      <button class="secondary" onclick={chooseDirectory} disabled={busy}>Browse</button>
      <button class="secondary" onclick={detect} disabled={busy}>Detect</button>
      <button onclick={refresh} disabled={busy || !saveDirectory.trim()}>Open</button>
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </section>

  <div class="workspace">
    <aside class="card">
      <div class="section-title"><h2>Save slots</h2><span>{slots.length}</span></div>
      {#if slots.length === 0}
        <p class="empty">Choose a directory to discover save bundles.</p>
      {:else}
        <nav aria-label="Save slots">
          {#each slots as slot}
            <button
              class:active={slot.slot === selectedSlot}
              class="slot"
              onclick={() => (selectedSlot = slot.slot)}
            >
              <span class="slot-number">{slot.slot}</span>
              <span><strong>Slot {slot.slot}</strong><small>{slot.artifacts.length} artifacts · {formatKilobytes(slot.totalSize)}</small></span>
              <span class:warning={!slot.complete} class="dot" title={slot.complete ? 'Complete' : 'Missing primary save'}></span>
            </button>
          {/each}
        </nav>
      {/if}
    </aside>

    <section class="card history">
      <div class="section-title">
        <div><p class="eyebrow">{selected ? `SLOT ${selected.slot}` : 'NO SLOT SELECTED'}</p><h2>Snapshot history</h2></div>
        <button onclick={createSnapshot} disabled={busy || !selected?.complete}>Create snapshot</button>
      </div>
      {#if selected && !selected.complete}
        <p class="notice">The primary <code>slot{selected.slot}.save</code> file is missing. This bundle cannot be snapshotted.</p>
      {/if}
      {#if selectedSnapshots.length === 0}
        <div class="empty-state"><span>◇</span><h3>No snapshots yet</h3><p>Create the first verified recovery point for this slot.</p></div>
      {:else}
        <div class="snapshot-list">
          {#each selectedSnapshots as snapshot}
            <article>
              <span class="snapshot-mark"></span>
              <div><strong>{snapshot.label ?? new Date(snapshot.createdAtUtc).toLocaleString()}</strong><small>{snapshot.label ? `${new Date(snapshot.createdAtUtc).toLocaleString()} · ` : ''}{snapshot.fileCount} files · {formatKilobytes(snapshot.totalSize)}</small></div>
              <div class="snapshot-actions">
                <code>{snapshot.id.slice(0, 10)}</code>
                <button class="secondary compact" onclick={() => editLabel(snapshot)} disabled={busy}>Label</button>
                <button class="secondary compact" onclick={() => toggleProtection(snapshot)} disabled={busy}>{snapshot.protected ? 'Unprotect' : 'Protect'}</button>
                <button class="secondary compact" onclick={() => restore(snapshot)} disabled={busy}>Restore</button>
                <button class="secondary compact" onclick={() => exportBackup(snapshot)} disabled={busy}>Export</button>
                <button class="danger compact" onclick={() => removeSnapshot(snapshot)} disabled={busy || snapshot.protected}>Delete</button>
              </div>
            </article>
          {/each}
        </div>
      {/if}
    </section>
  </div>
</main>
