import { invoke } from '@tauri-apps/api/core';
import type { AppSettings, DiagnosticReport, RestoreResult, SlotSummary, SnapshotSummary } from './contracts';

export const native = {
  loadSettings: () => invoke<AppSettings>('load_settings'),
  setActiveSaveDirectory: (saveDirectory: string | null) =>
    invoke<AppSettings>('set_active_save_directory', { saveDirectory }),
  detectSaveDirectories: () => invoke<string[]>('detect_save_directories'),
  discoverSlots: (saveDirectory: string) =>
    invoke<SlotSummary[]>('discover_slots', { saveDirectory }),
  createSnapshot: (saveDirectory: string, slot: number, pruneOldest = false) =>
    invoke<SnapshotSummary>('create_snapshot', { saveDirectory, slot, pruneOldest }),
  listSnapshots: (slot?: number) => invoke<SnapshotSummary[]>('list_snapshots', { slot }),
  restoreSnapshot: (saveDirectory: string, snapshotId: string) =>
    invoke<RestoreResult>('restore_snapshot', { saveDirectory, snapshotId }),
  updateSnapshotAnnotation: (
    snapshotId: string,
    label: string | null,
    note: string | null,
    protectedSnapshot: boolean
  ) =>
    invoke<SnapshotSummary>('update_snapshot_annotation', {
      snapshotId,
      label,
      note,
      protected: protectedSnapshot
    }),
  deleteSnapshot: (snapshotId: string) => invoke<void>('delete_snapshot', { snapshotId }),
  exportSnapshot: (snapshotId: string, destination: string) =>
    invoke<void>('export_snapshot', { snapshotId, destination }),
  importSnapshot: (source: string) => invoke<SnapshotSummary>('import_snapshot', { source }),
  loadDiagnostics: () => invoke<DiagnosticReport>('load_diagnostics'),
  acknowledgeRecovery: () => invoke<void>('acknowledge_recovery')
};
