export function memoryChangeIds(changes: ReadonlyArray<{ id: string }>): string[] {
  return changes.map((change) => change.id)
}

export function toggleMemoryChange(selectedIds: readonly string[], changeId: string): string[] {
  return selectedIds.includes(changeId)
    ? selectedIds.filter((id) => id !== changeId)
    : [...selectedIds, changeId]
}

export function areAllMemoryChangesSelected(
  changes: ReadonlyArray<{ id: string }>,
  selectedIds: readonly string[],
): boolean {
  if (changes.length === 0) return false
  const selected = new Set(selectedIds)
  return changes.every((change) => selected.has(change.id))
}

export function newestRestorableMemoryRun<T extends { status: string; started_at?: string }>(
  runs: readonly T[],
): T | null {
  return [...runs]
    .filter((run) => run.status === 'running' || run.status === 'awaiting_review')
    .sort((left, right) => (right.started_at ?? '').localeCompare(left.started_at ?? ''))[0] ?? null
}
