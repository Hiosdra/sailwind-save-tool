export type ArtifactKind = 'primary' | 'metadata' | 'thumbnail' | 'gameBackup' | 'sidecar';

export interface ArtifactSummary {
  relativePath: string;
  kind: ArtifactKind;
  size: number;
}

export interface SlotSummary {
  slot: number;
  complete: boolean;
  totalSize: number;
  artifacts: ArtifactSummary[];
}

export interface SnapshotSummary {
  id: string;
  slot: number;
  createdAtUtc: string;
  fileCount: number;
  totalSize: number;
  label: string | null;
  note: string | null;
  protected: boolean;
}

export interface RestoreResult {
  restoredSnapshotId: string;
  safetySnapshotId: string | null;
  slot: number;
}

export interface AppSettings {
  schemaVersion: number;
  activeSaveDirectory: string | null;
  updateChecksEnabled: boolean;
  theme: 'dark';
}

export interface AppError {
  code: string;
  message: string;
  context?: Record<string, string>;
}
